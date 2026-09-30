//! Keeps file and folder metadata on the copies (FR-19).

use std::fs::{File, FileTimes, Metadata};
use std::io;
use std::path::Path;
use std::time::SystemTime;

/// Gives `dst` the source's modification, access and creation times, and its permission
/// bits. Call before the final `fsync`, so the metadata is flushed with the data.
pub fn copy_to(src: &Metadata, dst: &File) -> io::Result<()> {
    let mut times = FileTimes::new();
    if let Ok(t) = src.modified() {
        times = times.set_modified(t);
    }
    let full = with_extra_times(times, src);
    // Some file systems reject part of it (e.g. creation times over SMB): then keep at
    // least the modification time, which later identical-file checks rely on (FR-17).
    if dst.set_times(full).is_err() {
        dst.set_times(times)?;
    }
    use std::os::unix::fs::PermissionsExt;
    // Always readable and writable by its owner (#118): verify reads it back, and a source
    // readable only through its group or others (a share) would give a copy nobody can open.
    let mode = (src.permissions().mode() & 0o777) | 0o600;
    dst.set_permissions(std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

fn with_extra_times(times: FileTimes, src: &Metadata) -> FileTimes {
    let times = match src.accessed() {
        Ok(t) => times.set_accessed(t),
        Err(_) => times,
    };
    use std::os::macos::fs::FileTimesExt;
    if let Ok(t) = src.created() {
        return times.set_created(t);
    }
    times
}

/// Sets a folder's modification time. Called after everything inside it is written,
/// deepest folders first, since writing into a folder changes its time.
pub fn set_dir_mtime(dir: &Path, mtime: SystemTime) -> io::Result<()> {
    File::open(dir)?.set_modified(mtime)
}

#[cfg(test)]
mod owner_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// QA review (#118): a source readable only through its group or others (a share) gives a
    /// copy its owner can still read and write: verify reads it back, and it can be opened.
    #[test]
    fn the_copy_stays_readable_and_writable_by_its_owner() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::write(&src, b"x").unwrap();
        std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o044)).unwrap();
        let meta = std::fs::metadata(&src).unwrap();
        let out = File::create(&dst).unwrap();
        copy_to(&meta, &out).unwrap();
        let mode = std::fs::metadata(&dst).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }
}
