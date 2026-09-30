//! Walks the source and builds the list of files to copy (FR-1..FR-14).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walkdir::WalkDir;

use crate::error::IoFailure;
use crate::filter::{ExtKey, ExtensionFilter, ext_key};
use crate::source::{DirMode, Source};
use crate::system::is_system_file;

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Include system files too (FR-14): `.DS_Store`, `Thumbs.db` and the like. Hidden
    /// files are always included.
    pub include_system_files: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    /// Path of the source file.
    pub source: PathBuf,
    /// Path relative to the destination directory.
    pub rel: PathBuf,
    pub size: u64,
    pub ext: ExtKey,
    /// Source modification time, kept on the copy (FR-19) and used to spot identical files (FR-17).
    pub mtime: Option<SystemTime>,
}

/// A directory the job creates, relative to the destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub rel: PathBuf,
    /// Source directory's modification time, restored after its contents are written (FR-19).
    pub mtime: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExtStat {
    pub files: u64,
    pub bytes: u64,
}

/// Something under the source that could not be read during the scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProblem {
    pub path: PathBuf,
    /// In English, for reports and the CLI.
    pub message: String,
    /// What it is, for the app to say in the user's language (#84).
    pub kind: ScanProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanProblemKind {
    /// Picked, but not a regular file (a directory among picked files, a device…).
    NotAFile,
    Io(IoFailure),
    /// A directory that links back to one of its parents.
    Loop,
}

/// `scan`'s error for "copy the folder itself" of a drive root, which has no name: inside the
/// `io::Error`, so callers can tell it apart (`get_ref()` + `downcast_ref`).
#[derive(Debug)]
pub struct DriveRoot;

impl std::fmt::Display for DriveRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a drive root has no directory name; copy only its contents instead")
    }
}

impl std::error::Error for DriveRoot {}

#[derive(Debug, Clone, Default)]
pub struct Scan {
    pub files: Vec<ScanEntry>,
    /// Folder created for "copy the folder itself" (FR-4a), relative to the destination.
    pub root_dir: Option<PathBuf>,
    /// Source directories with no visible children, relative to the destination (FR-6).
    pub empty_dirs: Vec<PathBuf>,
    /// File count and bytes per extension, for the filter chips (FR-7).
    pub ext_stats: BTreeMap<ExtKey, ExtStat>,
    /// System files skipped; a skipped directory counts once (FR-13).
    pub skipped_system: u64,
    /// Symlinks are never followed or copied (FR-24).
    pub skipped_symlinks: Vec<PathBuf>,
    pub problems: Vec<ScanProblem>,
    /// Modification time of every source directory, by path relative to the destination.
    pub dir_mtimes: HashMap<PathBuf, SystemTime>,
}

/// The files and directories a job will create.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub files: Vec<ScanEntry>,
    /// Directories to create, relative to the destination, parents first.
    pub dirs: Vec<DirEntry>,
    pub total_bytes: u64,
    /// What the scan couldn't read: it isn't copied, and the job says so (#58).
    pub unread: Vec<ScanProblem>,
}

impl Selection {
    /// Only the files at `ids` (indexes into `files`), e.g. to retry failed files.
    /// Keeps the directories that contain them; empty directories are dropped.
    pub fn subset(&self, ids: &[usize]) -> Selection {
        let files: Vec<ScanEntry> = ids.iter().map(|&i| self.files[i].clone()).collect();
        let dirs = self
            .dirs
            .iter()
            .filter(|d| files.iter().any(|f| f.rel.starts_with(&d.rel)))
            .cloned()
            .collect();
        // A retry copies files that were read; what couldn't be read was reported before.
        Selection {
            total_bytes: files.iter().map(|f| f.size).sum(),
            files,
            dirs,
            unread: Vec::new(),
        }
    }
}

pub fn scan(source: &Source, opts: &ScanOptions) -> io::Result<Scan> {
    match source {
        Source::Directory { path, mode } => scan_dir(path, *mode, opts),
        Source::Files(paths) => Ok(scan_files(paths)),
    }
}

impl Scan {
    /// Applies the extension filter and lists the directories to create (FR-8..FR-10).
    pub fn select(&self, filter: &ExtensionFilter) -> Selection {
        let files: Vec<ScanEntry> = self
            .files
            .iter()
            .filter(|f| filter.matches(&f.ext))
            .cloned()
            .collect();
        let mut dirs: BTreeSet<PathBuf> = self.root_dir.iter().cloned().collect();
        if !filter.is_active() {
            dirs.extend(self.empty_dirs.iter().cloned());
        }
        // Every folder on the way, so each gets its date back and is made durable (#115).
        for f in &files {
            for dir in f.rel.ancestors().skip(1) {
                if dir.as_os_str().is_empty() || !dirs.insert(dir.to_path_buf()) {
                    break;
                }
            }
        }
        let total_bytes = files.iter().map(|f| f.size).sum();
        let dirs = dirs
            .into_iter()
            .map(|rel| DirEntry {
                mtime: self.dir_mtimes.get(&rel).copied(),
                rel,
            })
            .collect();
        Selection {
            files,
            dirs,
            total_bytes,
            unread: self.problems.clone(),
        }
    }

    fn push_file(&mut self, source: PathBuf, rel: PathBuf, meta: &fs::Metadata) {
        let (size, mtime) = (meta.len(), meta.modified().ok());
        let ext = ext_key(&rel);
        let stat = self.ext_stats.entry(ext.clone()).or_default();
        stat.files += 1;
        stat.bytes += size;
        self.files.push(ScanEntry {
            source,
            rel,
            size,
            ext,
            mtime,
        });
    }

    fn problem(&mut self, path: &Path, message: impl ToString, kind: ScanProblemKind) {
        self.problems.push(ScanProblem {
            path: path.to_path_buf(),
            message: message.to_string(),
            kind,
        });
    }

    fn io_problem(&mut self, path: &Path, e: io::Error) {
        let message = e.to_string();
        self.problem(path, message, ScanProblemKind::Io(e.into()));
    }
}

/// Files picked one by one are copied flat, and copied even if hidden:
/// the user chose them explicitly.
fn scan_files(paths: &[PathBuf]) -> Scan {
    let mut scan = Scan::default();
    for path in paths {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => scan.skipped_symlinks.push(path.clone()),
            Ok(meta) if meta.is_file() => {
                let name = path.file_name().expect("a regular file has a file name");
                scan.push_file(path.clone(), PathBuf::from(name), &meta);
            }
            Ok(_) => scan.problem(path, "not a regular file", ScanProblemKind::NotAFile),
            Err(e) => scan.io_problem(path, e),
        }
    }
    scan
}

fn scan_dir(root: &Path, mode: DirMode, opts: &ScanOptions) -> io::Result<Scan> {
    // Not `canonicalize`: a symlinked source folder keeps its own name.
    let root = std::path::absolute(root)?;
    let root_meta = fs::metadata(&root)?;
    let prefix = match mode {
        DirMode::ContentsOnly => PathBuf::new(),
        DirMode::FolderItself => PathBuf::from(folder_name(&root)?),
    };
    let mut scan = Scan {
        root_dir: (!prefix.as_os_str().is_empty()).then(|| prefix.clone()),
        ..Scan::default()
    };
    if let Ok(mtime) = root_meta.modified() {
        scan.dir_mtimes.insert(prefix.clone(), mtime);
    }
    let mut dirs = BTreeSet::new();
    let mut non_empty = HashSet::new();
    let mut skipped_system = 0u64;

    let walker = WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            // The root is passed to the predicate too; whatever was picked is scanned.
            if opts.include_system_files || e.depth() == 0 {
                return true;
            }
            let system = is_system_file(e.file_name());
            if system {
                skipped_system += 1;
            }
            !system
        });

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let path = e
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.clone());
                let message = e.to_string();
                let kind = match e.into_io_error() {
                    Some(io) => ScanProblemKind::Io(io.into()),
                    None => ScanProblemKind::Loop,
                };
                scan.problem(&path, message, kind);
                continue;
            }
        };
        let rel = prefix.join(
            entry
                .path()
                .strip_prefix(&root)
                .expect("walkdir yields paths under the root"),
        );
        let parent = rel.parent().map(Path::to_path_buf).unwrap_or_default();
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            scan.skipped_symlinks.push(entry.into_path());
        } else if file_type.is_dir() {
            if let Some(mtime) = entry.metadata().ok().and_then(|m| m.modified().ok()) {
                scan.dir_mtimes.insert(rel.clone(), mtime);
            }
            dirs.insert(rel);
            non_empty.insert(parent);
        } else if file_type.is_file() {
            match entry.metadata() {
                Ok(meta) => {
                    non_empty.insert(parent);
                    scan.push_file(entry.into_path(), rel, &meta);
                }
                Err(e) => {
                    let message = e.to_string();
                    let kind = match e.into_io_error() {
                        Some(io) => ScanProblemKind::Io(io.into()),
                        None => ScanProblemKind::Loop,
                    };
                    scan.problem(entry.path(), message, kind);
                }
            }
        }
        // Sockets, FIFOs and devices are ignored.
    }

    scan.skipped_system = skipped_system;
    scan.empty_dirs = dirs
        .into_iter()
        .filter(|d| !non_empty.contains(d))
        .collect();
    Ok(scan)
}

/// The folder's own name for "copy the folder itself" (FR-4a). A path ending in `..`
/// has no name of its own, so it is resolved first.
fn folder_name(root: &Path) -> io::Result<std::ffi::OsString> {
    let resolved;
    let named = if root.file_name().is_some() {
        root
    } else {
        resolved = fs::canonicalize(root)?;
        &resolved
    };
    named
        .file_name()
        .map(ToOwned::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, DriveRoot))
}
