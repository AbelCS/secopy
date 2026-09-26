//! Keeps file and folder metadata on the copies (FR-19).

use std::fs::{File, FileTimes, Metadata};
use std::io;
use std::path::Path;
use std::time::SystemTime;

/// Gives `dst` the source's modification, access and (macOS, Windows) creation times,
/// and on macOS and Linux its permission bits. Call before the final `fsync`, so the
/// metadata is flushed with the data.
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = src.permissions().mode() & 0o777;
        dst.set_permissions(std::fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

fn with_extra_times(times: FileTimes, src: &Metadata) -> FileTimes {
    let times = match src.accessed() {
        Ok(t) => times.set_accessed(t),
        Err(_) => times,
    };
    #[cfg(target_os = "macos")]
    use std::os::macos::fs::FileTimesExt;
    #[cfg(windows)]
    use std::os::windows::fs::FileTimesExt;
    #[cfg(any(target_os = "macos", windows))]
    if let Ok(t) = src.created() {
        return times.set_created(t);
    }
    times
}

/// Sets a folder's modification time. Called after everything inside it is written,
/// deepest folders first, since writing into a folder changes its time.
pub fn set_dir_mtime(dir: &Path, mtime: SystemTime) -> io::Result<()> {
    open_dir(dir)?.set_modified(mtime)
}

#[cfg(unix)]
fn open_dir(dir: &Path) -> io::Result<File> {
    File::open(dir)
}

#[cfg(windows)]
fn open_dir(dir: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
    std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(dir)
}
