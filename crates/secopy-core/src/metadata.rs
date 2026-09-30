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
    // Always readable by its owner (#118): verify reads it back, and a source readable only
    // through its group or others (a share) would give a copy its owner can't open. The rest
    // as it was: a read-only file stays read-only (FR-19).
    let mode = (src.permissions().mode() & 0o777) | 0o400;
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
    /// copy its owner can still read: verify reads it back. A read-only one stays read-only.
    #[test]
    fn the_copy_stays_readable_by_its_owner_and_read_only_if_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::write(&src, b"x").unwrap();
        std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o044)).unwrap();
        let meta = std::fs::metadata(&src).unwrap();
        let out = File::create(&dst).unwrap();
        copy_to(&meta, &out).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dst), 0o444);
        std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o444)).unwrap();
        let out = File::options().write(true).open(&dst).unwrap();
        copy_to(&std::fs::metadata(&src).unwrap(), &out).unwrap();
        assert_eq!(mode(&dst), 0o444, "read-only stays read-only");
    }
}
