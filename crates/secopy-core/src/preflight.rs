//! Checks a selection against the destination before a job starts (FR-16, FR-17, FR-17a).

use std::collections::HashSet;
use std::fmt;
use std::fs::{self, Metadata};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use unicode_normalization::UnicodeNormalization;

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
    /// `available` counts purgeable space (`FsInfo::available_bytes`).
    NotEnoughSpace {
        needed: u64,
        available: u64,
    },
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Blocker::DestMissing => write!(f, "the destination is not an existing directory"),
            Blocker::DestNotWritable(e) => write!(f, "can't write to the destination: {e}"),
            Blocker::DestInsideSource => {
                write!(f, "the destination is the source directory or inside it")
            }
            Blocker::NotEnoughSpace { needed, available } => write!(
                f,
                "not enough space: {needed} bytes needed, {available} bytes available"
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
    /// It would land on a source file: never written, so the source can't be replaced (#112).
    InSource,
}

impl ProblemKind {
    /// How the file fails if the job starts anyway.
    pub fn to_error(&self) -> FileError {
        match self {
            ProblemKind::InvalidName(p) => FileError::InvalidName(p.clone()),
            ProblemKind::TooLarge { limit } => FileError::TooLarge { limit: *limit },
            ProblemKind::NameClash => FileError::NameClash,
            ProblemKind::InTheWay { path } => FileError::InTheWay { path: path.clone() },
            ProblemKind::InSource => FileError::InSource,
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
    let checked = check_files(sel, dest, &fs, &Sources::of(source));
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

/// The source, by device and inode, so a destination file can be recognised as one of its
/// files whatever its path's case, Unicode form or links.
struct Sources {
    /// A directory source: every file under it.
    dir: Option<(u64, u64)>,
    /// Picked files.
    files: HashSet<(u64, u64)>,
}

impl Sources {
    fn of(source: &Source) -> Self {
        let id = |p: &Path| fs::metadata(p).ok().map(|m| (m.dev(), m.ino()));
        match source {
            Source::Directory { path, .. } => Sources {
                dir: id(path),
                files: HashSet::new(),
            },
            Source::Files(files) => Sources {
                dir: None,
                files: files.iter().filter_map(|f| id(f)).collect(),
            },
        }
    }

    /// `existing` (at `path`, under `dest`) is a source file.
    fn has(&self, path: &Path, existing: &Metadata, dest: &Path) -> bool {
        if self.files.contains(&(existing.dev(), existing.ino())) {
            return true;
        }
        let Some(dir) = self.dir else { return false };
        // Is one of its directories under the destination the source directory? (Above it,
        // the source would hold the destination: blocked already.)
        path.ancestors()
            .skip(1)
            .take_while(|a| a.starts_with(dest))
            .any(|a| fs::metadata(a).is_ok_and(|m| (m.dev(), m.ino()) == dir))
    }
}

fn check_files(sel: &Selection, dest: &Path, fs: &FsInfo, sources: &Sources) -> Checked {
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
                Ok(meta) if meta.is_file() && sources.has(&final_path, &meta, dest) => {
                    Some(ProblemKind::InSource)
                }
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

/// Paths that land on the same destination file: the same in Unicode's composed form (NFC),
/// as APFS and HFS+ treat them on every volume (#112), ignoring case only where the file
/// system does. Names that aren't UTF-8 are compared on raw bytes.
pub(crate) fn clash_key(rel: &Path, case_sensitive: bool) -> Vec<u8> {
    let bytes = rel.as_os_str().as_encoded_bytes();
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            let composed: String = text.nfc().collect();
            if case_sensitive {
                composed.into_bytes()
            } else {
                composed.to_lowercase().into_bytes()
            }
        }
        Err(_) if case_sensitive => bytes.to_vec(),
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
            available_bytes: u64::MAX,
            block_size: 1,
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
        check_files(&sel, dir.path(), fs, &Sources::of(&Source::Files(vec![])))
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
    fn names_differing_in_unicode_form_only_clash_on_every_drive() {
        // "café" composed (NFC) and decomposed (NFD): one name on APFS and HFS+ (#112).
        let files = || vec![entry("caf\u{e9}.mov", 1), entry("cafe\u{301}.mov", 1)];
        for case_sensitive in [false, true] {
            assert_eq!(
                problems(files(), &fat(case_sensitive)),
                vec![(1, ProblemKind::NameClash)]
            );
        }
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
