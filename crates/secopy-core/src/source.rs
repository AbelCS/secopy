//! What the user picked as source (RFD §6.1).

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// One directory, copied recursively (FR-1, FR-6).
    Directory { path: PathBuf, mode: DirMode },
    /// One or more files, copied flat into the destination (FR-2, FR-5).
    Files(Vec<PathBuf>),
}

/// How a directory source lands in the destination (FR-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirMode {
    /// `DEST/<SOURCE_NAME>/…`
    FolderItself,
    /// `DEST/…`
    ContentsOnly,
}
