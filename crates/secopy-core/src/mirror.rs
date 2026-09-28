//! One-way mirror (plan 7, RFD §5.8): what a run copies, updates and removes.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

use crate::filter::ExtensionFilter;
use crate::job::{JobControl, JobReport};
use crate::plan::{Action, DiffersPolicy, Plan};
use crate::preflight::preflight;
use crate::scan::{ScanOptions, scan};
use crate::source::{DirMode, Source};
use crate::system::is_system_file;
use crate::verify::hash_from_device;

pub const ARCHIVE_DIR: &str = ".secopy-archive";

/// What happens to files deleted in the origin (FR-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deleted {
    /// Moved to the destination's archive, kept `days` days.
    Archive {
        days: u32,
    },
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
    pub guard: Option<String>,
    pub options: MirrorOptions,
}

pub fn plan(
    origin: &Path,
    destination: &Path,
    options: &MirrorOptions,
) -> Result<MirrorPlan, String> {
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
) -> Result<MirrorPlan, String> {
    if !origin.is_dir() {
        return Err(format!("The origin isn't there: {}", origin.display()));
    }
    nested(origin, destination)?;
    // Archiving into a link could put (and later clean up) files anywhere.
    if let Deleted::Archive { .. } = options.deleted
        && fs::symlink_metadata(destination.join(ARCHIVE_DIR)).is_ok_and(|m| !m.is_dir())
    {
        return Err(format!(
            "The destination's {ARCHIVE_DIR} isn't a directory (it's a link or a file). Move it \
             away, or choose to delete removed files."
        ));
    }
    let source = Source::Directory {
        path: origin.to_path_buf(),
        mode: DirMode::ContentsOnly,
    };
    let scanned = scan(&source, &ScanOptions::default()).map_err(|e| e.to_string())?;
    let mut sel = scanned.select(&ExtensionFilter::All);
    // An archive inside the origin (a mirror of a mirror) isn't mirrored.
    sel.files.retain(|f| !f.rel.starts_with(ARCHIVE_DIR));
    sel.dirs.retain(|d| !d.rel.starts_with(ARCHIVE_DIR));
    sel.total_bytes = sel.files.iter().map(|f| f.size).sum();
    let pf = preflight(&source, &sel, destination).map_err(|b| b.to_string())?;
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
                    return Err("Cancelled.".into());
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
    let planned: Vec<&Path> = copy.files.iter().map(|f| f.entry.rel.as_path()).collect();
    let Extras {
        mut removals,
        seen,
        mut remove_dirs,
        mut renames,
        destination_files,
    } = extras(destination, &planned, &|rel| {
        fs::symlink_metadata(origin.join(rel)).is_ok()
    });
    let guard = if let Some(first) = scanned.problems.first() {
        // What couldn't be read would look deleted in the origin.
        removals.clear();
        remove_dirs.clear();
        renames.clear();
        Some(format!(
            "{} in the origin couldn't be read ({}: {}). Nothing is removed from the destination this run.",
            match scanned.problems.len() {
                1 => "1 item".to_string(),
                n => format!("{n} items"),
            },
            first.path.display(),
            first.message
        ))
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
fn nested(origin: &Path, destination: &Path) -> Result<(), String> {
    let (origin, destination) = (&resolved(origin), &resolved(destination));
    let same = |a: &Path, b: &Path| same_file::is_same_file(a, b).unwrap_or(false);
    if same(origin, destination) {
        return Err("The origin and the destination are the same directory.".into());
    }
    if destination.ancestors().skip(1).any(|a| same(a, origin)) {
        return Err("The destination can't be inside the origin.".into());
    }
    if origin.ancestors().skip(1).any(|a| same(a, destination)) {
        return Err("The origin can't be inside the destination.".into());
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
    for entry in walk.filter_map(Result::ok) {
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

fn guard(origin_files: u64, removals: u64, destination_files: u64) -> Option<String> {
    if origin_files == 0 && destination_files > 0 {
        return Some(
            "The origin has no files: every file in the destination would be removed.".into(),
        );
    }
    (removals * 2 > destination_files).then(|| {
        format!("{removals} of the destination's {destination_files} files would be removed.")
    })
}

/// One file the mirror archived or deleted, or why it couldn't.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub rel: PathBuf,
    pub result: Result<(), String>,
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
    finished: &Result<Finished, String>,
    archived: bool,
) -> crate::report::MirrorPart {
    let show = |p: &Path| p.display().to_string();
    let mut part = crate::report::MirrorPart {
        archived,
        removed: Vec::new(),
        not_removed: Vec::new(),
        renamed: Vec::new(),
        nothing_removed: None,
    };
    match finished {
        Ok(f) => {
            for r in &f.removals {
                match &r.result {
                    Ok(()) => part.removed.push(show(&r.rel)),
                    Err(why) => part.not_removed.push(crate::report::Unread {
                        path: show(&r.rel),
                        reason: why.clone(),
                    }),
                }
            }
            part.renamed = f.renamed.iter().map(|(a, b)| (show(a), show(b))).collect();
        }
        Err(why) => part.nothing_removed = Some(why.clone()),
    }
    part
}

const STAMP: &str = "%Y-%m-%d %H.%M.%S";

/// This run's archive directory: named by `now`, and never one another run already has
/// (two runs in the same second get "… (2)"), so nothing archived is ever overwritten.
pub fn archive_dir(destination: &Path, now: chrono::DateTime<chrono::Local>) -> PathBuf {
    let root = destination.join(ARCHIVE_DIR);
    let stamp = now.format(STAMP).to_string();
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
) -> Result<Finished, String> {
    let failed = report.failed().count();
    if report.cancelled {
        return Err(
            "Files deleted in the origin were left in the destination: the mirror was cancelled."
                .into(),
        );
    }
    if report.fatal.is_some() {
        return Err(
            "Files deleted in the origin were left in the destination: the mirror stopped.".into(),
        );
    }
    if failed > 0 {
        return Err(format!(
            "Files deleted in the origin were left in the destination: {failed} {} failed.",
            if failed == 1 { "file" } else { "files" }
        ));
    }
    // Anything else that went wrong (unread items, directories, the device): no removals.
    if !report.is_success() {
        return Err(
            "Files deleted in the origin were left in the destination: the copy didn't end cleanly."
                .into(),
        );
    }
    let dest = &plan.copy.dest;
    let mut done = Vec::new();
    for rel in &plan.removals {
        // Back in the origin since the plan: it stays.
        if fs::symlink_metadata(plan.origin.join(rel)).is_ok() {
            continue;
        }
        let from = dest.join(rel);
        let now = match fs::symlink_metadata(&from) {
            Ok(meta) => Seen::of(&meta),
            Err(_) => continue, // already gone
        };
        // Written since the preview (another app): not the file the preview listed.
        if plan.seen.get(rel) != Some(&now) {
            done.push(Removal {
                rel: rel.clone(),
                result: Err("It changed after the preview, so it was kept.".into()),
            });
            continue;
        }
        let result = match archive {
            Some(root) => {
                let to = root.join(rel);
                to.parent()
                    .map_or(Ok(()), fs::create_dir_all)
                    .and_then(|()| fs::rename(&from, &to))
            }
            None => fs::remove_file(&from),
        };
        done.push(Removal {
            rel: rel.clone(),
            result: result.map_err(|e| e.to_string()),
        });
    }
    for dir in &plan.remove_dirs {
        let _ = fs::remove_dir(dest.join(dir)); // only if it is empty now
    }
    let mut renamed = Vec::new();
    for (from, to) in &plan.renames {
        let (a, b) = (dest.join(from), dest.join(to));
        // Only the spelling of one file; never onto another one.
        if same_file::is_same_file(&a, &b).unwrap_or(false) && fs::rename(&a, &b).is_ok() {
            renamed.push((from.clone(), to.clone()));
        }
    }
    Ok(Finished {
        removals: done,
        renamed,
    })
}

/// Removes archive run directories older than `days` (named by `archive_dir`).
pub fn clean_archives(destination: &Path, days: u32, now: chrono::DateTime<chrono::Local>) -> u32 {
    let limit = now - chrono::Duration::days(i64::from(days));
    let root = destination.join(ARCHIVE_DIR);
    // Only a real directory: a link could lead anywhere outside the destination.
    if !fs::symlink_metadata(&root).is_ok_and(|m| m.is_dir()) {
        return 0;
    }
    let Ok(entries) = fs::read_dir(&root) else {
        return 0;
    };
    let mut removed = 0;
    for e in entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
    {
        let name = e.file_name().to_string_lossy().into_owned();
        // "… (2)": a second run in the same second.
        let stamp = name.split(" (").next().unwrap_or_default();
        let old = chrono::NaiveDateTime::parse_from_str(stamp, STAMP)
            .ok()
            .and_then(|t| t.and_local_timezone(chrono::Local).single())
            .is_some_and(|t| t < limit);
        if old && fs::remove_dir_all(e.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// The mirror's checksum file, after a clean run (plan 8): the previous one, with this run's
/// copied and updated files, the deep check's equal files, removals dropped and renames moved.
pub fn write_checksums(
    plan: &MirrorPlan,
    report: &JobReport,
    finished: &Finished,
) -> std::io::Result<()> {
    use crate::job::FileStatus;
    let dest = &plan.copy.dest;
    let path = dest.join(crate::check::MIRROR_CHECKSUMS);
    // The previous file; one that is there but can't be read is kept, never replaced.
    let mut sums: std::collections::BTreeMap<PathBuf, u64> = match fs::read_to_string(&path) {
        Ok(text) => crate::check::parse(&text).0.into_iter().collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e),
    };
    // Renames first: a renamed file this run also updated keeps the new hash below.
    for (from, to) in &finished.renamed {
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
    for r in finished.removals.iter().filter(|r| r.result.is_ok()) {
        sums.remove(&r.rel);
    }
    // Files no longer in the destination (removed by hand, or gone) leave it too.
    sums.retain(|rel, _| fs::symlink_metadata(dest.join(rel)).is_ok_and(|m| m.is_file()));
    let entries: Vec<(PathBuf, u64)> = sums.into_iter().collect();
    crate::checksum_file::write_replacing(&path, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;

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
