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

/// What a mirror run does, worked out before anything is touched.
#[derive(Debug, Clone)]
pub struct MirrorPlan {
    pub origin: PathBuf,
    /// The origin as a job source: its contents go straight into the destination.
    pub source: Source,
    /// The copy phase: `Copy`, `Overwrite` or `SkipIdentical` for each origin file.
    pub copy: Plan,
    /// Each file the run writes, by index into `copy.files`.
    pub changes: Vec<(usize, Change)>,
    /// Destination files no longer in the origin, relative to the destination.
    pub removals: Vec<PathBuf>,
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
    if !origin.is_dir() {
        return Err(format!("The origin isn't there: {}", origin.display()));
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
    for (i, f) in copy.files.iter_mut().enumerate() {
        match f.action {
            Action::Copy => changes.push((i, Change::New)),
            Action::Overwrite => changes.push((i, Change::Changed)),
            Action::SkipIdentical
                if options.deep_check
                    && differs(&f.entry.source, &destination.join(&f.entry.rel)) =>
            {
                f.action = Action::Overwrite;
                changes.push((i, Change::ContentsDiffer));
            }
            _ => {}
        }
    }
    let planned: Vec<&Path> = copy.files.iter().map(|f| f.entry.rel.as_path()).collect();
    let Extras {
        mut removals,
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
        remove_dirs,
        renames,
        destination_files,
        guard,
        options: options.clone(),
    })
}

/// Contents differ (the deep check): either side unreadable counts as different.
fn differs(a: &Path, b: &Path) -> bool {
    let control = JobControl::new();
    let hash = |p: &Path| {
        hash_from_device(p, 4 << 20, &|_| {}, &control)
            .map(|(h, _)| h)
            .ok()
    };
    match (hash(a), hash(b)) {
        (Some(x), Some(y)) => x != y,
        _ => true,
    }
}

/// What the destination has that the origin doesn't.
struct Extras {
    removals: Vec<PathBuf>,
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
        removals.push(rel);
    }
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    Extras {
        removals,
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

const STAMP: &str = "%Y-%m-%d %H.%M.%S";

pub fn archive_dir(destination: &Path, now: chrono::DateTime<chrono::Local>) -> PathBuf {
    destination
        .join(ARCHIVE_DIR)
        .join(now.format(STAMP).to_string())
}

/// The removals, once the copy phase is clean (FR-49); `archive` is `Some` in archive mode.
pub fn finish(
    plan: &MirrorPlan,
    report: &JobReport,
    archive: Option<&Path>,
) -> Result<Vec<Removal>, String> {
    let failed = report.failed().count();
    if report.cancelled {
        return Err("Nothing was removed: the mirror was cancelled.".into());
    }
    if report.fatal.is_some() {
        return Err("Nothing was removed: the mirror stopped.".into());
    }
    if failed > 0 {
        return Err(format!(
            "Nothing was removed: {failed} {} failed.",
            if failed == 1 { "file" } else { "files" }
        ));
    }
    let dest = &plan.copy.dest;
    let mut done = Vec::new();
    for rel in &plan.removals {
        // Back in the origin since the plan: it stays.
        if fs::symlink_metadata(plan.origin.join(rel)).is_ok() {
            continue;
        }
        let from = dest.join(rel);
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
    for (from, to) in &plan.renames {
        let (from, to) = (dest.join(from), dest.join(to));
        // Only the spelling of one file; never onto another one.
        if same_file::is_same_file(&from, &to).unwrap_or(false) {
            let _ = fs::rename(from, to);
        }
    }
    Ok(done)
}

/// Removes archive run directories older than `days` (named by `archive_dir`).
pub fn clean_archives(destination: &Path, days: u32, now: chrono::DateTime<chrono::Local>) -> u32 {
    let limit = now - chrono::Duration::days(i64::from(days));
    let Ok(entries) = fs::read_dir(destination.join(ARCHIVE_DIR)) else {
        return 0;
    };
    let mut removed = 0;
    for e in entries.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        let old = chrono::NaiveDateTime::parse_from_str(&name, STAMP)
            .ok()
            .and_then(|t| t.and_local_timezone(chrono::Local).single())
            .is_some_and(|t| t < limit);
        if old && fs::remove_dir_all(e.path()).is_ok() {
            removed += 1;
        }
    }
    removed
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
