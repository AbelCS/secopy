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
    let mode = src.permissions().mode() & 0o777;
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
