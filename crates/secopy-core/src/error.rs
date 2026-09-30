//! Per-file errors (FR-21).

use std::path::PathBuf;
use std::{fmt, io};

use crate::names::NameProblem;

/// An I/O error reduced to plain data, so outcomes can be cloned, compared and sent to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoFailure {
    pub kind: io::ErrorKind,
    pub message: String,
}

impl From<io::Error> for IoFailure {
    fn from(e: io::Error) -> Self {
        Self {
            kind: e.kind(),
            message: e.to_string(),
        }
    }
}

impl fmt::Display for IoFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Why one file was not copied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("cannot read source: {0}")]
    ReadSource(IoFailure),
    #[error("cannot write destination: {0}")]
    WriteDest(IoFailure),
    #[error("cannot read back the copy: {0}")]
    ReadBack(IoFailure),
    /// Verify: the file's size changed while it was read, so what was hashed is no version of it.
    #[error("cannot read back the copy: it changed while it was read")]
    ChangedWhileRead,
    #[error("hash mismatch (source {expected}, copy {actual})")]
    HashMismatch { expected: String, actual: String },
    #[error("a file with this name already exists at the destination")]
    AlreadyExists,
    #[error("another file in this copy has the same name (names are compared ignoring case)")]
    NameClash,
    #[error("another copy is writing this file")]
    PartialInUse,
    #[error("{0}")]
    InvalidName(NameProblem),
    #[error("the file is larger than the destination drive allows ({limit} bytes)")]
    TooLarge { limit: u64 },
    #[error("something is in the way at {}", path.display())]
    InTheWay { path: PathBuf },
    #[error("it would land on a source file")]
    InSource,
    #[error("the source file changed while it was copied")]
    SourceChanged,
    #[error("changed since it was copied (expected {expected}, found {actual})")]
    Changed { expected: String, actual: String },
    #[error("missing")]
    Missing,
    /// Verify found a link where a file was listed; links are never followed (FR-24).
    #[error("is a link, not checked (links aren't followed)")]
    IsLink,
    #[error("is a directory, not a file")]
    IsDirectory,
    /// Something else that isn't a regular file (a pipe, a device…).
    #[error("isn't a regular file, not checked")]
    NotAFile,
    #[error("cancelled")]
    Cancelled,
}

impl FileError {
    pub fn read_source(e: io::Error) -> Self {
        FileError::ReadSource(e.into())
    }

    pub fn write_dest(e: io::Error) -> Self {
        FileError::WriteDest(e.into())
    }

    pub fn read_back(e: io::Error) -> Self {
        FileError::ReadBack(e.into())
    }

    pub fn is_disk_full(&self) -> bool {
        matches!(self, FileError::WriteDest(f) if f.kind == io::ErrorKind::StorageFull)
    }
}

/// Errors that stop the whole job instead of just one file (FR-21).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FatalError {
    #[error("the destination drive is full")]
    DiskFull,
    #[error("the destination is no longer available; was it disconnected?")]
    DestinationGone,
    #[error("the source is no longer available; was it disconnected?")]
    SourceGone,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_full_is_recognised() {
        let e = FileError::write_dest(io::Error::from(io::ErrorKind::StorageFull));
        assert!(e.is_disk_full());
    }

    #[test]
    fn other_errors_are_not_disk_full() {
        let e = FileError::read_source(io::Error::from(io::ErrorKind::StorageFull));
        assert!(!e.is_disk_full(), "only writes can fill the destination");
        assert!(!FileError::AlreadyExists.is_disk_full());
    }
}
