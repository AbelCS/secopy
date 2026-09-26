//! Walks the source and builds the list of files to copy (FR-1..FR-14).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::filter::{ExtKey, ExtensionFilter, ext_key};
use crate::hidden::is_hidden;
use crate::source::{DirMode, Source};

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Include hidden files and folders (FR-14). Not exposed in the v1 UI.
    pub include_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    /// Path of the source file.
    pub source: PathBuf,
    /// Path relative to the destination directory.
    pub rel: PathBuf,
    pub size: u64,
    pub ext: ExtKey,
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
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Scan {
    pub files: Vec<ScanEntry>,
    /// Folder created for "copy the folder itself" (FR-4a), relative to the destination.
    pub root_dir: Option<PathBuf>,
    /// Source directories with no visible children, relative to the destination (FR-6).
    pub empty_dirs: Vec<PathBuf>,
    /// File count and bytes per extension, for the filter chips (FR-7).
    pub ext_stats: BTreeMap<ExtKey, ExtStat>,
    /// Hidden files and folders skipped; a skipped folder counts once (FR-13).
    pub skipped_hidden: u64,
    /// Symlinks are never followed or copied (FR-24).
    pub skipped_symlinks: Vec<PathBuf>,
    pub problems: Vec<ScanProblem>,
}

/// The files and directories a job will create.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub files: Vec<ScanEntry>,
    /// Directories to create, relative to the destination, parents first.
    pub dirs: Vec<PathBuf>,
    pub total_bytes: u64,
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
        for f in &files {
            if let Some(parent) = f.rel.parent().filter(|p| !p.as_os_str().is_empty()) {
                dirs.insert(parent.to_path_buf());
            }
        }
        let total_bytes = files.iter().map(|f| f.size).sum();
        Selection {
            files,
            dirs: dirs.into_iter().collect(),
            total_bytes,
        }
    }

    fn push_file(&mut self, source: PathBuf, rel: PathBuf, size: u64) {
        let ext = ext_key(&rel);
        let stat = self.ext_stats.entry(ext.clone()).or_default();
        stat.files += 1;
        stat.bytes += size;
        self.files.push(ScanEntry {
            source,
            rel,
            size,
            ext,
        });
    }

    fn problem(&mut self, path: &Path, message: impl ToString) {
        self.problems.push(ScanProblem {
            path: path.to_path_buf(),
            message: message.to_string(),
        });
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
                scan.push_file(path.clone(), PathBuf::from(name), meta.len());
            }
            Ok(_) => scan.problem(path, "not a regular file"),
            Err(e) => scan.problem(path, e),
        }
    }
    scan
}

fn scan_dir(root: &Path, mode: DirMode, opts: &ScanOptions) -> io::Result<Scan> {
    let root = fs::canonicalize(root)?;
    let prefix = match mode {
        DirMode::ContentsOnly => PathBuf::new(),
        DirMode::FolderItself => PathBuf::from(root.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a drive root has no folder name; copy only its contents instead",
            )
        })?),
    };
    let mut scan = Scan {
        root_dir: (!prefix.as_os_str().is_empty()).then(|| prefix.clone()),
        ..Scan::default()
    };
    let mut dirs = BTreeSet::new();
    let mut non_empty = HashSet::new();
    let mut skipped_hidden = 0u64;

    let walker = WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            // The root is passed to the predicate too; picking a hidden folder is allowed.
            if opts.include_hidden || e.depth() == 0 {
                return true;
            }
            let hidden = e.metadata().is_ok_and(|m| is_hidden(e.path(), &m));
            if hidden {
                skipped_hidden += 1;
            }
            !hidden
        });

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let path = e
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.clone());
                scan.problem(&path, e);
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
            dirs.insert(rel);
            non_empty.insert(parent);
        } else if file_type.is_file() {
            match entry.metadata() {
                Ok(meta) => {
                    non_empty.insert(parent);
                    scan.push_file(entry.into_path(), rel, meta.len());
                }
                Err(e) => scan.problem(entry.path(), e),
            }
        }
        // Sockets, FIFOs and devices are ignored.
    }

    scan.skipped_hidden = skipped_hidden;
    scan.empty_dirs = dirs
        .into_iter()
        .filter(|d| !non_empty.contains(d))
        .collect();
    Ok(scan)
}
