//! Checks a selection against the destination before a job starts (FR-16, FR-17, FR-17a).

use std::collections::HashSet;
use std::fmt;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::copy::partial_path;
use crate::error::{FileError, IoFailure};
use crate::fsinfo::{self, FsInfo};
use crate::names::{self, NameProblem};
use crate::scan::{ScanEntry, Selection};
use crate::source::Source;

/// Why the job can't start at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocker {
    DestMissing,
    DestNotWritable(IoFailure),
    DestInsideSource,
    NotEnoughSpace { needed: u64, free: u64 },
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Blocker::DestMissing => write!(f, "the destination is not an existing directory"),
            Blocker::DestNotWritable(e) => write!(f, "can't write to the destination: {e}"),
            Blocker::DestInsideSource => {
                write!(f, "the destination is the source directory or inside it")
            }
            Blocker::NotEnoughSpace { needed, free } => write!(
                f,
                "not enough free space: {needed} bytes needed, {free} bytes free"
            ),
        }
    }
}

/// A file that will fail if the job starts anyway (FR-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileProblem {
    /// Index in `Selection::files`.
    pub id: usize,
    pub kind: ProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemKind {
    InvalidName(NameProblem),
    /// Larger than the destination file system allows (FAT: 4 GiB).
    TooLarge {
        limit: u64,
    },
    /// Another file in this job has the same destination path (FR-17a).
    NameClash,
    /// A folder is where the file goes, or a file is where one of its folders goes.
    InTheWay {
        path: PathBuf,
    },
}

impl ProblemKind {
    /// How the file fails if the job starts anyway.
    pub fn to_error(&self) -> FileError {
        match self {
            ProblemKind::InvalidName(p) => FileError::InvalidName(p.clone()),
            ProblemKind::TooLarge { limit } => FileError::TooLarge { limit: *limit },
            ProblemKind::NameClash => FileError::NameClash,
            ProblemKind::InTheWay { path } => FileError::InTheWay { path: path.clone() },
        }
    }
}

/// A file that already exists at the destination (FR-17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub id: usize,
    pub kind: ConflictKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// Same size, and modified less than [`SAME_MTIME`] apart: skipped without reading it.
    Identical,
    /// The existing file's size and modification time.
    Differs {
        size: u64,
        mtime: Option<SystemTime>,
    },
}

/// A source folder and the volume it is on, re-checked after I/O errors (FR-21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRoot {
    pub path: PathBuf,
    pub device: u64,
}

#[derive(Debug, Clone)]
pub struct Preflight {
    pub dest: PathBuf,
    pub fs: FsInfo,
    pub file_problems: Vec<FileProblem>,
    pub conflicts: Vec<Conflict>,
    /// Partial files left by an interrupted job where this job will write (FR-18).
    pub stale_partials: Vec<PathBuf>,
    /// Files whose names aren't valid UTF-8, so the checksum file can't list them (FR-31).
    pub checksum_omissions: Vec<usize>,
    pub source_roots: Vec<SourceRoot>,
}

/// Modification times closer than this count as equal: FAT stores times in 2 s steps,
/// and exFAT and SMB round them.
const SAME_MTIME: Duration = Duration::from_secs(2);

/// Checks `sel` against `dest`. Problems that stop the whole job are returned as the
/// error; everything else is listed so the user can decide.
pub fn preflight(source: &Source, sel: &Selection, dest: &Path) -> Result<Preflight, Blocker> {
    if !fs::metadata(dest).is_ok_and(|m| m.is_dir()) {
        return Err(Blocker::DestMissing);
    }
    if let Source::Directory { path, .. } = source
        && is_inside(dest, path)
    {
        return Err(Blocker::DestInsideSource);
    }
    let fs = fsinfo::fs_info(dest).map_err(|e| Blocker::DestNotWritable(e.into()))?;
    let checked = check_files(sel, dest, &fs);
    Ok(Preflight {
        dest: dest.to_path_buf(),
        fs,
        file_problems: checked.problems,
        conflicts: checked.conflicts,
        stale_partials: checked.stale_partials,
        checksum_omissions: checked.checksum_omissions,
        source_roots: source_roots(source),
    })
}

/// True if `dest` is `source` or inside it. Symlinks are resolved, so a link that
/// points into the source is caught too.
fn is_inside(dest: &Path, source: &Path) -> bool {
    let resolve = |p: &Path| fs::canonicalize(p).or_else(|_| std::path::absolute(p));
    match (resolve(dest), resolve(source)) {
        (Ok(dest), Ok(source)) => dest.starts_with(source),
        _ => false,
    }
}

fn source_roots(source: &Source) -> Vec<SourceRoot> {
    let paths: Vec<PathBuf> = match source {
        Source::Directory { path, .. } => vec![path.clone()],
        Source::Files(files) => {
            let mut seen = HashSet::new();
            files
                .iter()
                .filter_map(|f| f.parent().map(Path::to_path_buf))
                .filter(|p| seen.insert(p.clone()))
                .collect()
        }
    };
    paths
        .into_iter()
        .filter_map(|path| {
            // Absolute, like the scanned file paths it is compared with.
            let path = std::path::absolute(&path).ok()?;
            let device = fsinfo::device_id(&path).ok()?;
            Some(SourceRoot { path, device })
        })
        .collect()
}

#[derive(Default)]
struct Checked {
    problems: Vec<FileProblem>,
    conflicts: Vec<Conflict>,
    stale_partials: Vec<PathBuf>,
    checksum_omissions: Vec<usize>,
}

fn check_files(sel: &Selection, dest: &Path, fs: &FsInfo) -> Checked {
    let mut out = Checked::default();
    let mut seen = HashSet::new();
    for (id, entry) in sel.files.iter().enumerate() {
        if entry.rel.to_str().is_none() {
            out.checksum_omissions.push(id);
        }
        let clash = !seen.insert(clash_key(&entry.rel, fs.case_sensitive));
        let final_path = dest.join(&entry.rel);
        let problem = if clash {
            Some(ProblemKind::NameClash)
        } else if let Err(e) = names::check_path(&entry.rel, fs) {
            Some(ProblemKind::InvalidName(e))
        } else if let Some(limit) = fs.max_file_size.filter(|&l| entry.size > l) {
            Some(ProblemKind::TooLarge { limit })
        } else {
            match fs::symlink_metadata(&final_path) {
                Ok(meta) if meta.is_file() => {
                    out.conflicts.push(Conflict {
                        id,
                        kind: conflict_kind(entry, &meta),
                    });
                    None
                }
                Ok(_) => Some(ProblemKind::InTheWay {
                    path: entry.rel.clone(),
                }),
                Err(e) if e.kind() == io::ErrorKind::NotADirectory => Some(ProblemKind::InTheWay {
                    path: entry.rel.parent().unwrap_or(&entry.rel).to_path_buf(),
                }),
                Err(_) => None,
            }
        };
        if let Some(kind) = problem {
            out.problems.push(FileProblem { id, kind });
        } else if fs::symlink_metadata(partial_path(&final_path)).is_ok() {
            out.stale_partials.push(partial_path(&final_path));
        }
    }
    out
}

fn conflict_kind(entry: &ScanEntry, existing: &Metadata) -> ConflictKind {
    let mtime = existing.modified().ok();
    let same_mtime = match (entry.mtime, mtime) {
        (Some(a), Some(b)) => a.duration_since(b).unwrap_or_else(|e| e.duration()) < SAME_MTIME,
        _ => false,
    };
    if entry.size == existing.len() && same_mtime {
        ConflictKind::Identical
    } else {
        ConflictKind::Differs {
            size: existing.len(),
            mtime,
        }
    }
}

/// Paths that land on the same destination file. Compared on raw bytes, not lossy UTF-8,
/// and ignoring case only where the file system does. Unicode normalization (NFC vs NFD
/// on APFS) isn't predicted; the no-replace commit catches it.
pub(crate) fn clash_key(rel: &Path, case_sensitive: bool) -> Vec<u8> {
    let bytes = rel.as_os_str().as_encoded_bytes();
    if case_sensitive {
        return bytes.to_vec();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_lowercase().into_bytes(),
        Err(_) => bytes.to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsinfo::{FAT_MAX_FILE_SIZE, FsKind, NameLimit};

    fn entry(rel: &str, size: u64) -> ScanEntry {
        ScanEntry {
            source: PathBuf::from("/src").join(rel),
            rel: PathBuf::from(rel),
            size,
            ext: None,
            mtime: None,
        }
    }

    fn fat(case_sensitive: bool) -> FsInfo {
        FsInfo {
            kind: FsKind::Fat,
            case_sensitive,
            free_bytes: u64::MAX,
            max_file_size: Some(FAT_MAX_FILE_SIZE),
            name_limit: NameLimit::Utf16Units(255),
            device: 0,
        }
    }

    fn problems(files: Vec<ScanEntry>, fs: &FsInfo) -> Vec<(usize, ProblemKind)> {
        let dir = tempfile::tempdir().unwrap();
        let sel = Selection {
            files,
            ..Selection::default()
        };
        check_files(&sel, dir.path(), fs)
            .problems
            .into_iter()
            .map(|p| (p.id, p.kind))
            .collect()
    }

    #[test]
    fn per_file_problems_on_fat() {
        let files = vec![
            entry("ok.mov", 10),
            entry("a?b.mov", 10),
            entry("huge.mov", FAT_MAX_FILE_SIZE + 1),
            entry("OK.MOV", 10),
        ];
        assert_eq!(
            problems(files, &fat(false)),
            vec![
                (1, ProblemKind::InvalidName(NameProblem::InvalidChar('?'))),
                (
                    2,
                    ProblemKind::TooLarge {
                        limit: FAT_MAX_FILE_SIZE
                    }
                ),
                (3, ProblemKind::NameClash),
            ]
        );
    }

    #[test]
    fn names_differing_in_case_only_clash_on_case_insensitive_drives() {
        let files = || vec![entry("a.txt", 1), entry("A.TXT", 1)];
        assert_eq!(
            problems(files(), &fat(false)),
            vec![(1, ProblemKind::NameClash)]
        );
        assert_eq!(problems(files(), &fat(true)), vec![]);
    }

    #[test]
    fn identical_means_same_size_and_mtime_within_two_seconds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mov");
        fs::write(&path, b"12345").unwrap();
        let meta = fs::metadata(&path).unwrap();
        let t = meta.modified().unwrap();
        let at = |mtime: SystemTime, size| ScanEntry {
            mtime: Some(mtime),
            ..entry("a.mov", size)
        };
        assert_eq!(conflict_kind(&at(t, 5), &meta), ConflictKind::Identical);
        let later = t + Duration::from_millis(1500);
        assert_eq!(conflict_kind(&at(later, 5), &meta), ConflictKind::Identical);
        let differs = ConflictKind::Differs {
            size: 5,
            mtime: Some(t),
        };
        assert_eq!(
            conflict_kind(&at(t + Duration::from_secs(3), 5), &meta),
            differs
        );
        assert_eq!(conflict_kind(&at(t, 6), &meta), differs);
    }
}
