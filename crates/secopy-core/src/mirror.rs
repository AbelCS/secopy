//! One-way mirror (plan 7, RFD §5.8): what a run copies, updates and removes.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::filter::ExtensionFilter;
use crate::job::JobControl;
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
    /// The same file spelled with other letter case: (destination name, origin name).
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
    let Extras {
        removals,
        remove_dirs,
        renames,
        destination_files,
    } = extras(origin, destination, &copy);
    let guard = guard(
        sel.files.len() as u64,
        removals.len() as u64,
        destination_files,
    );
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

/// What's in the destination but not in the origin: files to remove, directories to remove
/// What the destination has that the origin doesn't.
struct Extras {
    removals: Vec<PathBuf>,
    remove_dirs: Vec<PathBuf>,
    renames: Vec<(PathBuf, PathBuf)>,
    destination_files: u64,
}

/// (deepest first), case-only renames; and the destination's file count.
fn extras(origin: &Path, destination: &Path, copy: &Plan) -> Extras {
    let planned: HashSet<&Path> = copy.files.iter().map(|f| f.entry.rel.as_path()).collect();
    let mut removals = Vec::new();
    let mut dirs = Vec::new();
    let mut renames = Vec::new();
    let mut count = 0;
    let walk = WalkDir::new(destination)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !(e.depth() == 1 && e.file_name() == ARCHIVE_DIR) && !is_system_file(e.file_name())
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
            if fs::symlink_metadata(origin.join(&rel)).is_err() {
                dirs.push(rel);
            }
            continue;
        }
        count += 1;
        if planned.contains(rel.as_path()) {
            continue;
        }
        // The origin's file system resolves the name (case- or form-insensitively): the same file.
        if fs::symlink_metadata(origin.join(&rel)).is_ok() {
            if let Some(spelled) = origin_spelling(origin, &rel)
                && spelled != rel
                && spelled.to_string_lossy().to_lowercase() == rel.to_string_lossy().to_lowercase()
            {
                renames.push((rel, spelled));
            }
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

/// How the origin spells `rel` (its directory listing's names), when it resolves.
fn origin_spelling(origin: &Path, rel: &Path) -> Option<PathBuf> {
    let mut at = origin.to_path_buf();
    let mut out = PathBuf::new();
    for part in rel.components() {
        let want = part.as_os_str().to_string_lossy().to_lowercase();
        let found = fs::read_dir(&at)
            .ok()?
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().to_lowercase() == want)?;
        out.push(found.file_name());
        at = found.path();
    }
    Some(out)
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
