//! One-way mirror (plan 7, RFD §5.8): what a run copies, updates and removes.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

use crate::error::IoFailure;
use crate::filter::ExtensionFilter;
use crate::job::{JobControl, JobReport};
use crate::plan::{Action, DiffersPolicy, Plan};
use crate::preflight::{Blocker, preflight};
use crate::scan::{DriveRoot, ScanOptions, ScanProblem, scan};
use crate::source::{DirMode, Source};
use crate::system::is_system_file;
use crate::verify::hash_from_device;

pub const ARCHIVE_DIR: &str = ".secopy-archive";

/// What happens to files deleted in the origin (FR-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deleted {
    /// Moved to the destination's archive (kept for the preset's days: `clean_archives`).
    Archive,
    Delete,
}

#[derive(Debug, Clone)]
pub struct MirrorOptions {
    pub deleted: Deleted,
    /// Also compare the contents of files whose size and date match (FR-46).
    pub deep_check: bool,
}

/// Why a file is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    New,
    /// Its size or modification date differs.
    Changed,
    /// Same size and date, different contents (the deep check).
    ContentsDiffer,
}

/// A destination file as the preview saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    pub len: u64,
    pub modified: Option<std::time::SystemTime>,
    /// The file itself (device and inode): a file saved in its place is another one, even
    /// with the same size and time.
    pub file: (u64, u64),
}

impl Seen {
    fn of(meta: &fs::Metadata) -> Seen {
        use std::os::unix::fs::MetadataExt;
        Seen {
            len: meta.len(),
            modified: meta.modified().ok(),
            file: (meta.dev(), meta.ino()),
        }
    }
}

/// What a mirror run does, worked out before anything is touched.
#[derive(Debug, Clone)]
pub struct MirrorPlan {
    pub origin: PathBuf,
    /// The origin as a job source: its contents go straight into the destination.
    pub source: Source,
    /// The copy phase: `Copy`, `Overwrite`, `SkipIdentical` or `Fail` for each origin file.
    pub copy: Plan,
    /// Each file the run writes, by index into `copy.files`.
    pub changes: Vec<(usize, Change)>,
    /// Destination files no longer in the origin, relative to the destination.
    pub removals: Vec<PathBuf>,
    /// How each removal looked in the preview: a file that changed since isn't removed.
    pub seen: HashMap<PathBuf, Seen>,
    /// Unchanged files the deep check read on both sides and found equal, with their hash.
    pub same: HashMap<PathBuf, u64>,
    /// Destination directories no longer in the origin, deepest first.
    pub remove_dirs: Vec<PathBuf>,
    /// The same file spelled otherwise (letter case, Unicode form): (destination name, origin name).
    pub renames: Vec<(PathBuf, PathBuf)>,
    /// Files in the destination (system files and the archive not counted).
    pub destination_files: u64,
    /// Why the run looks wrong (FR-50), if it does.
    pub guard: Option<Guard>,
    pub options: MirrorOptions,
}

/// Why a mirror can't be planned. `Display` is the English text reports and the CLI show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    OriginMissing(PathBuf),
    Same,
    DestinationInOrigin,
    OriginInDestination,
    /// The destination's archive is a link or a file.
    ArchiveNotDir,
    /// The origin couldn't be scanned; `drive_root` for "the folder itself" of a drive root.
    Scan {
        io: IoFailure,
        drive_root: bool,
    },
    Blocked(Blocker),
    Cancelled,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::OriginMissing(origin) => {
                write!(f, "The origin isn't there: {}", origin.display())
            }
            PlanError::Same => {
                f.write_str("The origin and the destination are the same directory.")
            }
            PlanError::DestinationInOrigin => {
                f.write_str("The destination can't be inside the origin.")
            }
            PlanError::OriginInDestination => {
                f.write_str("The origin can't be inside the destination.")
            }
            PlanError::ArchiveNotDir => write!(
                f,
                "The destination's {ARCHIVE_DIR} isn't a directory (it's a link or a file). Move it \
                 away, or choose to delete removed files."
            ),
            PlanError::Scan { io, .. } => write!(f, "{io}"),
            PlanError::Blocked(b) => write!(f, "{b}"),
            PlanError::Cancelled => f.write_str("Cancelled."),
        }
    }
}

impl From<PlanError> for String {
    fn from(e: PlanError) -> String {
        e.to_string()
    }
}

/// Why a mirror run looks wrong (FR-50). `Display` is the English text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Guard {
    /// Items in the origin couldn't be read: they would look deleted, so nothing is removed.
    Unread {
        count: usize,
        first: ScanProblem,
    },
    /// Directories in the destination couldn't be read: what they hold is unknown, so nothing
    /// is removed (#114).
    DestinationUnread {
        count: usize,
        first: PathBuf,
    },
    EmptyOrigin,
    TooMany {
        removals: u64,
        files: u64,
    },
}

impl std::fmt::Display for Guard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Guard::Unread { count, first } => write!(
                f,
                "{} in the origin couldn't be read ({}: {}). Nothing is removed from the destination this run.",
                match count {
                    1 => "1 item".to_string(),
                    n => format!("{n} items"),
                },
                first.path.display(),
                first.message
            ),
            Guard::DestinationUnread { count, first } => write!(
                f,
                "{} in the destination couldn't be read ({}). Nothing is removed from the destination this run.",
                match count {
                    1 => "1 directory".to_string(),
                    n => format!("{n} directories"),
                },
                first.display()
            ),
            Guard::EmptyOrigin => f.write_str(
                "The origin has no files: every file in the destination would be removed.",
            ),
            Guard::TooMany { removals, files } => {
                write!(
                    f,
                    "{removals} of the destination's {files} files would be removed."
                )
            }
        }
    }
}

/// Why `finish` removed nothing. `Display` is the English text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotRemoved {
    Cancelled,
    Stopped,
    Failed(usize),
    NotClean,
}

impl std::fmt::Display for NotRemoved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Files deleted in the origin were left in the destination: ")?;
        match self {
            NotRemoved::Cancelled => f.write_str("the mirror was cancelled."),
            NotRemoved::Stopped => f.write_str("the mirror stopped."),
            NotRemoved::Failed(1) => f.write_str("1 file failed."),
            NotRemoved::Failed(n) => write!(f, "{n} files failed."),
            NotRemoved::NotClean => f.write_str("the copy didn't end cleanly."),
        }
    }
}

/// Why one file wasn't removed. `Display` is the English text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemovalError {
    ChangedAfterPreview,
    Io(IoFailure),
}

impl std::fmt::Display for RemovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemovalError::ChangedAfterPreview => {
                f.write_str("It changed after the preview, so it was kept.")
            }
            RemovalError::Io(io) => write!(f, "{io}"),
        }
    }
}

pub fn plan(
    origin: &Path,
    destination: &Path,
    options: &MirrorOptions,
) -> Result<MirrorPlan, PlanError> {
    plan_watched(origin, destination, options, &JobControl::new(), &|_, _| {})
}

/// `plan`, telling how far the deep check is (files compared, of how many) and stopping with
/// "Cancelled." when `control` is cancelled.
pub fn plan_watched(
    origin: &Path,
    destination: &Path,
    options: &MirrorOptions,
    control: &JobControl,
    on_compared: &(dyn Fn(u64, u64) + Sync),
) -> Result<MirrorPlan, PlanError> {
    if !origin.is_dir() {
        return Err(PlanError::OriginMissing(origin.to_path_buf()));
    }
    nested(origin, destination)?;
    // Archiving into a link could put (and later clean up) files anywhere.
    if let Deleted::Archive = options.deleted
        && fs::symlink_metadata(destination.join(ARCHIVE_DIR)).is_ok_and(|m| !m.is_dir())
    {
        return Err(PlanError::ArchiveNotDir);
    }
    let source = Source::Directory {
        path: origin.to_path_buf(),
        mode: DirMode::ContentsOnly,
    };
    let scanned = scan(&source, &ScanOptions::default()).map_err(|e| PlanError::Scan {
        drive_root: e.get_ref().is_some_and(|r| r.is::<DriveRoot>()),
        io: e.into(),
    })?;
    let mut sel = scanned.select(&ExtensionFilter::All);
    // An archive inside the origin (a mirror of a mirror) isn't mirrored.
    sel.files.retain(|f| !f.rel.starts_with(ARCHIVE_DIR));
    sel.dirs.retain(|d| !d.rel.starts_with(ARCHIVE_DIR));
    sel.total_bytes = sel.files.iter().map(|f| f.size).sum();
    let pf = preflight(&source, &sel, destination).map_err(PlanError::Blocked)?;
    let mut copy = Plan::resolve(&sel, &pf, DiffersPolicy::Overwrite);
    let mut changes = Vec::new();
    let to_compare = if options.deep_check {
        copy.files
            .iter()
            .filter(|f| f.action == Action::SkipIdentical)
            .count() as u64
    } else {
        0
    };
    let mut same = HashMap::new();
    let mut compared = 0;
    if options.deep_check {
        on_compared(0, to_compare);
    }
    for (i, f) in copy.files.iter_mut().enumerate() {
        match f.action {
            Action::Copy => changes.push((i, Change::New)),
            Action::Overwrite => changes.push((i, Change::Changed)),
            Action::SkipIdentical if options.deep_check => {
                let equal = compare(&f.entry.source, &destination.join(&f.entry.rel), control);
                if control.is_stopped() {
                    return Err(PlanError::Cancelled);
                }
                match equal {
                    Some(hash) => {
                        same.insert(f.entry.rel.clone(), hash);
                    }
                    None => {
                        f.action = Action::Overwrite;
                        changes.push((i, Change::ContentsDiffer));
                    }
                }
                compared += 1;
                on_compared(compared, to_compare);
            }
            _ => {}
        }
    }
    // Not enough space for what it copies, the deep check's too: refused before it starts, as
    // a copy is (#135).
    if let Some(blocker) = copy.blockers().into_iter().next() {
        return Err(PlanError::Blocked(blocker));
    }
    let planned: Vec<&Path> = copy.files.iter().map(|f| f.entry.rel.as_path()).collect();
    let Extras {
        mut removals,
        seen,
        mut remove_dirs,
        mut renames,
        destination_files,
        unread,
    } = extras(destination, &planned, &|rel| in_origin(origin, rel));
    let guard = if let Some(first) = scanned.problems.first() {
        // What couldn't be read would look deleted in the origin.
        removals.clear();
        remove_dirs.clear();
        renames.clear();
        Some(Guard::Unread {
            count: scanned.problems.len(),
            first: first.clone(),
        })
    } else if let Some(first) = unread.first() {
        // What the destination couldn't show can't be weighed against the origin (#114).
        removals.clear();
        remove_dirs.clear();
        renames.clear();
        Some(Guard::DestinationUnread {
            count: unread.len(),
            first: first.clone(),
        })
    } else {
        guard(
            sel.files.len() as u64,
            removals.len() as u64,
            destination_files,
        )
    };
    Ok(MirrorPlan {
        origin: origin.to_path_buf(),
        source,
        copy,
        changes,
        removals,
        seen,
        same,
        remove_dirs,
        renames,
        destination_files,
        guard,
        options: options.clone(),
    })
}

/// One of the two directories holds the other, or they are the same one, however the paths
/// are written (letter case, symlinks): mirroring would copy or remove its own files.
fn nested(origin: &Path, destination: &Path) -> Result<(), PlanError> {
    let (origin, destination) = (&resolved(origin), &resolved(destination));
    let same = |a: &Path, b: &Path| same_file::is_same_file(a, b).unwrap_or(false);
    if same(origin, destination) {
        return Err(PlanError::Same);
    }
    if destination.ancestors().skip(1).any(|a| same(a, origin)) {
        return Err(PlanError::DestinationInOrigin);
    }
    if origin.ancestors().skip(1).any(|a| same(a, destination)) {
        return Err(PlanError::OriginInDestination);
    }
    Ok(())
}

/// `p` made absolute, with symlinks and `..` resolved as far as it exists (a destination may
/// not exist yet).
fn resolved(p: &Path) -> PathBuf {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut rest = Vec::new();
    let mut at = abs.as_path();
    loop {
        if let Ok(real) = fs::canonicalize(at) {
            return rest.iter().rev().fold(real, |acc, name| acc.join(name));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                at = parent;
            }
            _ => return abs,
        }
    }
}

/// The deep check: `Some(hash)` when both sides read in full and are equal; either side
/// unreadable counts as different.
fn compare(a: &Path, b: &Path, control: &JobControl) -> Option<u64> {
    let hash = |p: &Path| {
        hash_from_device(p, 4 << 20, &|_| {}, control)
            .map(|(h, _)| h)
            .ok()
    };
    match (hash(a), hash(b)) {
        (Some(x), Some(y)) if x == y => Some(x),
        _ => None,
    }
}

/// What the destination has that the origin doesn't.
struct Extras {
    removals: Vec<PathBuf>,
    seen: HashMap<PathBuf, Seen>,
    remove_dirs: Vec<PathBuf>,
    renames: Vec<(PathBuf, PathBuf)>,
    destination_files: u64,
    /// Directories that couldn't be read, relative to the destination.
    unread: Vec<PathBuf>,
}

/// Files to remove, directories to remove (deepest first), names spelled otherwise, and the
/// destination's file count. `planned` are the origin's files; `origin_has` says whether the
/// origin resolves a destination path.
fn extras(destination: &Path, planned: &[&Path], origin_has: &dyn Fn(&Path) -> bool) -> Extras {
    let exact: HashSet<&Path> = planned.iter().copied().collect();
    // The same name in another letter case or Unicode form.
    let mut alike: HashMap<String, Vec<&Path>> = HashMap::new();
    for p in planned {
        alike.entry(name_key(p)).or_default().push(p);
    }
    let mut removals = Vec::new();
    let mut seen = HashMap::new();
    let mut dirs = Vec::new();
    let mut renames = Vec::new();
    let mut count = 0;
    let walk = WalkDir::new(destination)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !(e.depth() == 1 && e.file_name() == ARCHIVE_DIR)
                && !is_system_file(e.file_name())
                && !is_nas_file(e.file_name())
        });
    let mut unread = Vec::new();
    for entry in walk {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let at = e.path().unwrap_or(destination);
                unread.push(at.strip_prefix(destination).unwrap_or(at).to_path_buf());
                continue;
            }
        };
        let Ok(rel) = entry.path().strip_prefix(destination) else {
            continue;
        };
        let rel = rel.to_path_buf();
        if entry.file_type().is_symlink() {
            continue;
        }
        if entry.file_type().is_dir() {
            if !origin_has(&rel) {
                dirs.push(rel);
            }
            continue;
        }
        count += 1;
        if exact.contains(rel.as_path()) {
            continue;
        }
        // The copy writes the origin's spelling; if the destination resolves it to this very
        // file, it is the same file (only a rename away), never one to remove.
        let same = alike.get(&name_key(&rel)).and_then(|names| {
            names.iter().find(|p| {
                same_file::is_same_file(destination.join(p), entry.path()).unwrap_or(false)
            })
        });
        if let Some(spelled) = same {
            renames.push((rel, spelled.to_path_buf()));
            continue;
        }
        if origin_has(&rel) {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            seen.insert(rel.clone(), Seen::of(&meta));
        }
        removals.push(rel);
    }
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    Extras {
        removals,
        seen,
        remove_dirs: dirs,
        renames,
        destination_files: count,
        unread,
    }
}

/// A name compared without letter case or Unicode form (NFC).
fn name_key(p: &Path) -> String {
    p.to_string_lossy().nfc().collect::<String>().to_lowercase()
}

/// What a NAS keeps in a share (thumbnails, recycle bins, snapshots, AFP bookkeeping):
/// never the origin's, so never removed.
const NAS_NAMES: &[&str] = &[
    "@eaDir",
    "#recycle",
    "#snapshot",
    "@Recycle",
    "@Recently-Snapshot",
    ".snapshot",
    ".@__thumb",
    ".@__qini",
    ".AppleDB",
    ".AppleDouble",
    ".AppleDesktop",
    "Network Trash Folder",
    "Temporary Items",
];

fn is_nas_file(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    NAS_NAMES.iter().any(|n| n.eq_ignore_ascii_case(&name))
}

fn guard(origin_files: u64, removals: u64, destination_files: u64) -> Option<Guard> {
    if origin_files == 0 && destination_files > 0 {
        return Some(Guard::EmptyOrigin);
    }
    (removals * 2 > destination_files).then_some(Guard::TooMany {
        removals,
        files: destination_files,
    })
}

/// One file the mirror archived or deleted, or why it couldn't.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub rel: PathBuf,
    pub result: Result<(), RemovalError>,
}

/// What `finish` did: the removals, and the names changed to the origin's spelling.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Finished {
    pub removals: Vec<Removal>,
    /// (destination name before, the origin's name).
    pub renamed: Vec<(PathBuf, PathBuf)>,
}

/// `finish`'s result for the report.
pub fn report_part(
    finished: &Result<Finished, NotRemoved>,
    archived: bool,
) -> crate::report::MirrorPart {
    let show = |p: &Path| p.display().to_string();
    let mut part = crate::report::MirrorPart {
        archived,
        removed: Vec::new(),
        not_removed: Vec::new(),
        renamed: Vec::new(),
        nothing_removed: None,
        archive_problem: None,
    };
    match finished {
        Ok(f) => {
            for r in &f.removals {
                match &r.result {
                    Ok(()) => part.removed.push(show(&r.rel)),
                    Err(why) => part.not_removed.push(crate::report::Unread {
                        path: show(&r.rel),
                        reason: why.to_string(),
                    }),
                }
            }
            part.renamed = f.renamed.iter().map(|(a, b)| (show(a), show(b))).collect();
        }
        Err(why) => part.nothing_removed = Some(why.to_string()),
    }
    part
}

const STAMP: &str = "%Y-%m-%d %H.%M.%S";
/// A run's name since #136: with its UTC offset, so its age is right after a time zone change.
const STAMP_ZONED: &str = "%Y-%m-%d %H.%M.%S %z";

/// This run's archive directory: named by `now`, and never one another run already has
/// (two runs in the same second get "… (2)"), so nothing archived is ever overwritten.
pub fn archive_dir(destination: &Path, now: chrono::DateTime<chrono::Local>) -> PathBuf {
    let root = destination.join(ARCHIVE_DIR);
    let stamp = now.format(STAMP_ZONED).to_string();
    (1u32..)
        .map(|n| match n {
            1 => root.join(&stamp),
            n => root.join(format!("{stamp} ({n})")),
        })
        .find(|p| fs::symlink_metadata(p).is_err())
        .expect("a free archive name")
}

/// The removals, once the copy phase is clean (FR-49); `archive` is `Some` in archive mode.
pub fn finish(
    plan: &MirrorPlan,
    report: &JobReport,
    archive: Option<&Path>,
) -> Result<Finished, NotRemoved> {
    let failed = report.failed().count();
    if report.cancelled {
        return Err(NotRemoved::Cancelled);
    }
    if report.fatal.is_some() {
        return Err(NotRemoved::Stopped);
    }
    if failed > 0 {
        return Err(NotRemoved::Failed(failed));
    }
    // Anything else that went wrong (unread items, directories, the device): no removals.
    if !report.is_success() {
        return Err(NotRemoved::NotClean);
    }
    let dest = &plan.copy.dest;
    let mut done = Vec::new();
    for rel in &plan.removals {
        // Back in the origin since the plan: it stays.
        if in_origin(&plan.origin, rel) {
            continue;
        }
        let from = dest.join(rel);
        let now = match fs::symlink_metadata(&from) {
            Ok(meta) => Seen::of(&meta),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue, // already gone
            // It can't be looked at: it stays, and says so (#114).
            Err(e) => {
                done.push(Removal {
                    rel: rel.clone(),
                    result: Err(RemovalError::Io(e.into())),
                });
                continue;
            }
        };
        // Written since the preview (another app): not the file the preview listed.
        if plan.seen.get(rel) != Some(&now) {
            done.push(Removal {
                rel: rel.clone(),
                result: Err(RemovalError::ChangedAfterPreview),
            });
            continue;
        }
        let result = match archive {
            Some(root) => {
                let to = root.join(rel);
                to.parent()
                    .map_or(Ok(()), fs::create_dir_all)
                    .and_then(|()| move_new(&from, &to))
            }
            None => fs::remove_file(&from),
        };
        done.push(Removal {
            rel: rel.clone(),
            result: result.map_err(|e| RemovalError::Io(e.into())),
        });
    }
    for dir in &plan.remove_dirs {
        let _ = fs::remove_dir(dest.join(dir)); // only if it is empty now
    }
    let mut renamed = Vec::new();
    for (from, to) in &plan.renames {
        if respell(dest, from, to) {
            renamed.push((from.clone(), to.clone()));
        }
    }
    Ok(Finished {
        removals: done,
        renamed,
    })
}

/// What a destination's archive holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveSummary {
    pub files: u64,
    pub bytes: u64,
    /// When its oldest run was archived (from the run directory's name).
    pub oldest: Option<chrono::DateTime<chrono::Local>>,
}

/// What `destination`'s archive holds (#101): `None` when it has none, it's empty, or it's a
/// link (never followed). An error when the destination itself can't be read.
pub fn archive_summary(destination: &Path) -> std::io::Result<Option<ArchiveSummary>> {
    fs::read_dir(destination)?;
    let root = destination.join(ARCHIVE_DIR);
    if !fs::symlink_metadata(&root).is_ok_and(|m| m.is_dir()) {
        return Ok(None);
    }
    let mut summary = ArchiveSummary {
        files: 0,
        bytes: 0,
        oldest: fs::read_dir(&root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| archived_at(&e.file_name().to_string_lossy()))
            .min(),
    };
    // Counted as `delete_archive` counts what it removes: everything but directories.
    for entry in WalkDir::new(&root).follow_links(false) {
        let entry = entry.map_err(std::io::Error::from)?;
        if !entry.file_type().is_dir() {
            summary.files += 1;
            summary.bytes += entry.metadata().map_err(std::io::Error::from)?.len();
        }
    }
    Ok((summary.files > 0).then_some(summary))
}

/// What deleting an archive did: files removed, files still there, and why the first that
/// couldn't be deleted wasn't.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArchiveDeleted {
    pub removed: u64,
    pub remaining: u64,
    pub error: Option<(PathBuf, IoFailure)>,
}

/// Files under `dir`, links not followed.
fn files_under(dir: &Path) -> u64 {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
        .count() as u64
}

impl ArchiveDeleted {
    /// For a report, in English: what `what` (a clean-up, a deletion) left in the archive, and
    /// why; `None` when it all went (#136).
    pub fn left_in_english(&self, what: &str) -> Option<String> {
        if self.remaining == 0 && self.error.is_none() {
            return None;
        }
        let why = self.error.as_ref().map_or_else(String::new, |(path, e)| {
            format!(": {}: {e}", path.display())
        });
        Some(format!(
            "Archive {what} NOT complete: {} file(s) left{why}",
            self.remaining
        ))
    }
}

/// Deletes `destination`'s whole archive (#101), when the user asks: only inside a real
/// `.secopy-archive` directory, never through a link. Each run directory goes with
/// `remove_dir_all`, which never follows a link, even one swapped in while it works. What
/// can't be deleted stays, counted.
pub fn delete_archive(destination: &Path) -> ArchiveDeleted {
    let mut done = ArchiveDeleted::default();
    let root = destination.join(ARCHIVE_DIR);
    if !fs::symlink_metadata(&root).is_ok_and(|m| m.is_dir()) {
        return done;
    }
    let before = files_under(&root);
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(e) => {
            done.remaining = before;
            done.error = Some((root, e.into()));
            return done;
        }
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let removed = match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => fs::remove_dir_all(&path),
            Ok(_) => fs::remove_file(&path),
            Err(e) => Err(e),
        };
        if let Err(e) = removed
            && done.error.is_none()
        {
            done.error = Some((path, e.into()));
        }
    }
    let _ = fs::remove_dir(&root); // only once it's empty
    done.remaining = if root.exists() { files_under(&root) } else { 0 };
    done.removed = before.saturating_sub(done.remaining);
    done
}

/// When an archive run directory was made, from its name (`archive_dir`'s, "… (2)" too).
fn archived_at(name: &str) -> Option<chrono::DateTime<chrono::Local>> {
    let stamp = name.split(" (").next().unwrap_or_default();
    if let Ok(zoned) = chrono::DateTime::parse_from_str(stamp, STAMP_ZONED) {
        return Some(zoned.with_timezone(&chrono::Local));
    }
    // A name from before (#136): local time; in the repeated hour when clocks go back, the
    // later of the two, so it's never removed early (it used to be never removed at all).
    chrono::NaiveDateTime::parse_from_str(stamp, STAMP)
        .ok()?
        .and_local_timezone(chrono::Local)
        .latest()
}

/// Moves `from` to `to`, never over something there: another run in the same second may have
/// archived a file under that name (#136).
fn move_new(from: &Path, to: &Path) -> std::io::Result<()> {
    match crate::os::rename_noreplace(from, to) {
        // No exclusive rename here (exFAT): the name is taken first, atomically, by creating
        // it; only then is the file moved over that empty placeholder of its own.
        Err(e) if e.kind() == std::io::ErrorKind::Unsupported => {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(to)?;
            fs::rename(from, to).inspect_err(|_| {
                let _ = fs::remove_file(to);
            })
        }
        other => other,
    }
}

/// Gives `from` the origin's spelling `to`, one name at a time from the top, so a directory
/// spelled otherwise is renamed too (#114): renaming the file alone left it as it was, and
/// every run reported it again. Only the spelling of the same file, never onto another one.
/// True if a name changed.
fn respell(dest: &Path, from: &Path, to: &Path) -> bool {
    let (from, to): (Vec<_>, Vec<_>) = (from.components().collect(), to.components().collect());
    if from.len() != to.len() {
        return false;
    }
    let mut changed = false;
    let mut parent = dest.to_path_buf();
    for (a, b) in from.iter().zip(&to) {
        let (x, y) = (parent.join(a), parent.join(b));
        if a != b && same_file::is_same_file(&x, &y).unwrap_or(false) {
            // Part of the way isn't renamed: it's reported only once it's spelled as asked.
            if fs::rename(&x, &y).is_err() {
                return false;
            }
            changed = true;
        }
        parent = y;
    }
    changed
}

/// `rel` is in the origin, or that can't be told, so it isn't removed from the backup: a link
/// on its way (never followed: what's under it may be on a disk that's unplugged, #114) or a
/// directory that can't be read. Checked one name at a time, so no link is followed.
fn in_origin(origin: &Path, rel: &Path) -> bool {
    let names: Vec<_> = rel.components().collect();
    let mut at = origin.to_path_buf();
    for (i, name) in names.iter().enumerate() {
        at.push(name);
        match fs::symlink_metadata(&at) {
            Ok(meta) if i + 1 < names.len() && meta.file_type().is_symlink() => return true,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return false,
            Err(_) => return true,
        }
    }
    true
}

/// Removes archive run directories older than `days` (named by `archive_dir`): the files
/// removed, the files left in runs that couldn't be removed, and why the first wasn't (#136).
pub fn clean_archives(
    destination: &Path,
    days: u32,
    now: chrono::DateTime<chrono::Local>,
) -> ArchiveDeleted {
    let mut done = ArchiveDeleted::default();
    // 0 isn't a limit: presets need at least a day, and an old one saved with 0 would empty
    // the archive without anyone asking (#113).
    if days == 0 {
        return done;
    }
    let limit = now - chrono::Duration::days(i64::from(days));
    let root = destination.join(ARCHIVE_DIR);
    // Only a real directory: a link could lead anywhere outside the destination. None is no
    // error; one that can't be looked at is (#136).
    match fs::symlink_metadata(&root) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return done,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return done,
        Err(e) => {
            done.error = Some((root, e.into()));
            return done;
        }
    }
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(e) => {
            done.error = Some((root, e.into()));
            return done;
        }
    };
    for e in entries {
        let e = match e.and_then(|e| e.file_type().map(|t| (e, t))) {
            Ok((e, t)) if t.is_dir() => e,
            Ok(_) => continue,
            Err(err) => {
                done.error.get_or_insert((root.clone(), err.into()));
                continue;
            }
        };
        if !archived_at(&e.file_name().to_string_lossy()).is_some_and(|t| t < limit) {
            continue;
        }
        let run = e.path();
        let before = files_under(&run);
        if let Err(err) = fs::remove_dir_all(&run) {
            let left = files_under(&run);
            done.removed += before.saturating_sub(left);
            done.remaining += left;
            done.error.get_or_insert((run, err.into()));
        } else {
            done.removed += before;
        }
    }
    done
}

/// The mirror's checksum file (plan 8): the previous one, with this run's copied and updated
/// files and the deep check's equal files; after a clean run (`finished`), removals dropped and
/// renames moved. A run that isn't clean still records what it verified (#114), so Verify
/// doesn't call those files changed. Not after an undo: the destination is back as it was.
pub fn write_checksums(
    plan: &MirrorPlan,
    report: &JobReport,
    finished: Option<&Finished>,
) -> std::io::Result<()> {
    use crate::job::FileStatus;
    let dest = &plan.copy.dest;
    let path = dest.join(crate::check::MIRROR_CHECKSUMS);
    // The previous file; one that is there but can't be read is kept, never replaced.
    let mut sums: std::collections::BTreeMap<PathBuf, u64> = match fs::read_to_string(&path) {
        Ok(text) => {
            let (entries, bad) = crate::check::parse(&text);
            // Lines that can't be read aren't dropped silently: that file is kept aside.
            if !bad.is_empty() {
                let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M.%S");
                let aside = (1..)
                    .map(|n| {
                        let name = crate::check::MIRROR_CHECKSUMS;
                        dest.join(match n {
                            1 => format!("{name}.damaged-{stamp}"),
                            n => format!("{name}.damaged-{stamp} ({n})"),
                        })
                    })
                    .find(|p| fs::symlink_metadata(p).is_err())
                    .expect("a free name");
                fs::rename(&path, aside)?;
            }
            entries.into_iter().collect()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e),
    };
    let (renamed, removals) = match finished {
        Some(f) => (f.renamed.as_slice(), f.removals.as_slice()),
        None => (&[][..], &[][..]),
    };
    // Renames first: a renamed file this run also updated keeps the new hash below.
    for (from, to) in renamed {
        if let Some(hash) = sums.remove(from) {
            sums.insert(to.clone(), hash);
        }
    }
    sums.extend(plan.same.iter().map(|(p, h)| (p.clone(), *h)));
    for o in &report.outcomes {
        if matches!(o.status, FileStatus::Copied | FileStatus::Verified)
            && let Some(hash) = o.hash
        {
            sums.insert(o.final_rel.clone(), hash);
        }
    }
    for r in removals.iter().filter(|r| r.result.is_ok()) {
        sums.remove(&r.rel);
    }
    // Files no longer in the destination (removed by hand, or gone) leave it too.
    // One that can't be looked at stays: gone isn't known (#114).
    sums.retain(|rel, _| match fs::symlink_metadata(dest.join(rel)) {
        Ok(meta) => meta.is_file(),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    });
    let entries: Vec<(PathBuf, u64)> = sums.into_iter().collect();
    crate::checksum_file::write_replacing(&path, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// QA review (#136): a run's name says its time zone, so its age is right after the Mac's
    /// changes; names from before (local time) still read.
    #[test]
    fn a_runs_name_holds_its_time_zone() {
        let dir = tempfile::tempdir().unwrap();
        let now = chrono::Local::now();
        let run = archive_dir(dir.path(), now);
        let name = run.file_name().unwrap().to_string_lossy().into_owned();
        let offset = name.rsplit(' ').next().unwrap();
        assert!(
            offset.len() == 5 && (offset.starts_with('+') || offset.starts_with('-')),
            "{name}"
        );
        let read = archived_at(&name).unwrap();
        assert_eq!(read.timestamp(), now.timestamp());
        let tokyo = archived_at("2026-01-01 09.00.00 +0900").unwrap();
        let utc = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap();
        assert_eq!(tokyo.timestamp(), utc.timestamp());
        assert!(
            archived_at("2026-01-01 09.00.00").is_some(),
            "an older name"
        );
        assert!(archived_at("2026-01-01 09.00.00 +0900 (2)").is_some());
    }

    /// Review of #114: a rename that gets only part of the way isn't reported as done.
    #[test]
    fn a_rename_that_fails_part_way_is_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("Clips")).unwrap();
        let file = dir.path().join("Clips/A.mov");
        fs::write(&file, b"a").unwrap();
        if !dir.path().join("clips/a.mov").exists() {
            return; // a case-sensitive volume
        }
        let c = crate::os::c_path(&file).unwrap();
        // SAFETY: `c` is a NUL-terminated path; UF_IMMUTABLE makes the rename of the file fail.
        unsafe { libc::chflags(c.as_ptr(), libc::UF_IMMUTABLE as _) };
        let done = respell(
            dir.path(),
            Path::new("Clips/A.mov"),
            Path::new("clips/a.mov"),
        );
        let c = crate::os::c_path(&dir.path().join("clips/A.mov")).unwrap();
        // SAFETY: as above; the flag is cleared so the directory can be removed.
        unsafe { libc::chflags(c.as_ptr(), 0) };
        assert!(!done);
    }

    /// Final review 3: an origin that doesn't resolve other Unicode forms (an SMB share) still
    /// has the file the destination spells in NFD.
    #[test]
    fn a_name_in_another_unicode_form_is_the_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let nfd = "Cafe\u{301}.mov";
        let nfc = "Caf\u{e9}.mov";
        fs::write(dir.path().join(nfd), b"x").unwrap();
        let planned = [Path::new(nfc)];
        let e = extras(dir.path(), &planned, &|_| false);
        assert!(e.removals.is_empty(), "{:?}", e.removals);
        assert_eq!(e.destination_files, 1);
    }
}
