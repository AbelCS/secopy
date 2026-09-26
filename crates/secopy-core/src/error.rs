//! Per-file errors (FR-21).

use std::{fmt, io};

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
    #[error("hash mismatch (source {expected}, copy {actual})")]
    HashMismatch { expected: String, actual: String },
    #[error("a file with this name already exists at the destination")]
    AlreadyExists,
    #[error("another file in this copy has the same name (names are compared ignoring case)")]
    NameClash,
    #[error("another copy is writing this file")]
    PartialInUse,
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

    /// Errors that stop the whole job instead of just this file (FR-21).
    pub fn is_fatal(&self) -> bool {
        matches!(self, FileError::WriteDest(f) if f.kind == io::ErrorKind::StorageFull)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_full_is_fatal() {
        let e = FileError::write_dest(io::Error::from(io::ErrorKind::StorageFull));
        assert!(e.is_fatal());
    }

    #[test]
    fn other_errors_are_not_fatal() {
        let e = FileError::read_source(io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(!e.is_fatal());
        assert!(!FileError::AlreadyExists.is_fatal());
    }
}
