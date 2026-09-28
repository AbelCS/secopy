//! macOS flushing and cache control (FR-18, FR-26, RFD §7.4).

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// Pushes a file's data to the device. On macOS, `File::sync_all` is `F_FULLFSYNC`,
/// which also flushes the drive's cache and costs milliseconds per call; per file we
/// use plain `fsync` and flush the drive cache once per job with [`full_barrier`].
pub fn sync_file(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: `fsync` on a valid descriptor owned by `file`.
    if unsafe { libc::fsync(file.as_raw_fd()) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Makes a file durable, including the drive's own cache where the file system can
/// (`F_FULLFSYNC`). SMB shares and some other file systems can't; there a plain `fsync`
/// is as far as it goes (#32).
pub fn sync_durable(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let full = || {
        // SAFETY: `fcntl` on a valid descriptor owned by `file`.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    };
    full_or_plain(full, || sync_file(file))
}

/// `full`, or `plain` when the file system doesn't support `full`.
fn full_or_plain(
    full: impl FnOnce() -> io::Result<()>,
    plain: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    match full() {
        Err(e) if matches!(e.raw_os_error(), Some(libc::ENOTSUP | libc::EOPNOTSUPP)) => plain(),
        result => result,
    }
}

/// Makes everything written to the volume holding `dir` durable, including the
/// drive's own cache. Called once at the end of a job.
pub fn full_barrier(dir: &Path) -> io::Result<()> {
    sync_durable(&File::open(dir)?)
}

/// An error from the device while syncing, not a file system that can't sync a directory
/// or flush its cache (#58).
pub fn is_device_error(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(libc::EIO | libc::ENXIO | libc::ENODEV | libc::ENOSPC | libc::EROFS)
    )
}

/// Asks the OS not to keep this file's pages in cache (`F_NOCACHE`). Returns true if the
/// request was accepted.
pub fn set_nocache(file: &File) -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: `fcntl` on a valid descriptor owned by `file`, with an integer argument.
    unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) != -1 }
}

/// Opens `path` so reads come from the device, not the page cache; a link fails (`ELOOP`).
/// The bool is false when the file system cannot guarantee the bypass.
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let bypassed = set_nocache(&file);
    Ok((file, bypassed))
}

/// Renames `from` to `to` without replacing an existing `to`, in one system call.
/// Errors: `AlreadyExists` if the name is taken (as the file system judges names:
/// case, Unicode normalization), `Unsupported` if the file system can't do it.
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    let (from, to) = (c_path(from)?, c_path(to)?);
    // SAFETY: both pointers are valid NUL-terminated strings for the call's duration.
    let rc = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    if rc == 0 {
        Ok(())
    } else {
        Err(unsupported_or(io::Error::last_os_error()))
    }
}

pub(crate) fn c_path(path: &Path) -> io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

/// File systems without the flag report ENOTSUP; EINVAL and ENOSYS mean the same.
fn unsupported_or(e: io::Error) -> io::Error {
    match e.raw_os_error() {
        Some(libc::ENOTSUP | libc::EINVAL | libc::ENOSYS) => io::ErrorKind::Unsupported.into(),
        _ => e,
    }
}

/// Creates a new partial file that stays locked while it is open, so another job can tell
/// a live partial file from one left by an interrupted job (FR-18). The lock is an
/// advisory `flock`.
pub fn create_locked(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    let file = opts.open(path)?;
    // Only another job's cleanup can hold a brand-new file's lock, and only for a moment,
    // so wait for it. That cleanup may have removed the new file: then the name is no
    // longer ours. Lock errors (file systems without locks) leave the file unlocked.
    let _ = lock(&file, true);
    if !is_at(&file, path)? {
        return Err(io::ErrorKind::AlreadyExists.into());
    }
    Ok(file)
}

/// Removes a partial file left by an interrupted job. Returns `false`, and leaves the file
/// alone, if a live writer still holds it.
pub fn remove_stale(path: &Path) -> io::Result<bool> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(e),
    };
    if let Ok(false) = lock(&file, false) {
        return Ok(false);
    }
    // Creating and locking a file are two steps: a file modified within the last
    // moment may be one another writer has just created and not locked yet. A time in the
    // future (FAT's local time after a zone or clock change) is not that.
    let modified = file.metadata()?.modified()?;
    let distance = std::time::SystemTime::now()
        .duration_since(modified)
        .unwrap_or_else(|e| e.duration());
    if distance < STALE_AFTER {
        return Ok(false);
    }
    // The name may belong to another writer's new file by now.
    if !is_at(&file, path)? {
        return Ok(!path.exists());
    }
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(true),
    }
}

/// A partial file modified less than this long ago (or ahead) is never treated as left by
/// an interrupted job.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether `path` still names the open `file`.
pub fn is_at(file: &File, path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let held = file.metadata()?;
    match fs::symlink_metadata(path) {
        Ok(now) => Ok((now.dev(), now.ino()) == (held.dev(), held.ino())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Takes the exclusive advisory lock; without `wait`, `Ok(false)` if someone holds it.
/// `flock`, not `fcntl` locks: those are per process, and two jobs in one app must see
/// each other's locks.
fn lock(file: &File, wait: bool) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    let op = if wait {
        libc::LOCK_EX
    } else {
        libc::LOCK_EX | libc::LOCK_NB
    };
    // SAFETY: `flock` on a valid descriptor owned by `file`.
    if unsafe { libc::flock(file.as_raw_fd(), op) } == 0 {
        return Ok(true);
    }
    let e = io::Error::last_os_error();
    if e.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(false)
    } else {
        Err(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;

    /// #69: a device read never follows a link, even one put in place after the path was
    /// looked at (Verify checks the path first, then opens it).
    #[test]
    fn a_device_read_doesnt_follow_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("outside.mov");
        fs::write(&target, b"x").unwrap();
        let link = dir.path().join("clip.mov");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(open_uncached(&link).is_err());
        assert!(open_uncached(&target).is_ok());
    }

    /// SMB shares don't support F_FULLFSYNC (ENOTSUP); the file is still written, so a
    /// plain fsync must do instead of failing the checksum file (#32).
    #[test]
    fn a_full_flush_the_file_system_cant_do_falls_back_to_fsync() {
        let unsupported = || Err(io::Error::from_raw_os_error(libc::ENOTSUP));
        let mut plain_ran = false;
        assert!(
            full_or_plain(unsupported, || {
                plain_ran = true;
                Ok(())
            })
            .is_ok()
        );
        assert!(plain_ran);
        // Other errors are real: a full disk or a pulled drive must still fail.
        let full_disk = || Err(io::Error::from_raw_os_error(libc::ENOSPC));
        let err = full_or_plain(full_disk, || Ok(())).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(libc::ENOSPC));
        // And when the full flush works, fsync isn't run again.
        assert!(full_or_plain(|| Ok(()), || panic!("not needed")).is_ok());
    }

    /// Pages of `path` that are in the OS page cache, via `mincore` on a mapping.
    fn resident_pages(path: &Path) -> usize {
        let file = File::open(path).unwrap();
        let len = file.metadata().unwrap().len() as usize;
        // SAFETY: `sysconf` has no preconditions.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        let mut vec = vec![0u8; len.div_ceil(page)];
        // SAFETY: a read-only shared mapping of a file we own, unmapped below; `vec` has
        // one byte per page, as `mincore` requires.
        unsafe {
            let addr = libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            );
            assert_ne!(addr, libc::MAP_FAILED);
            assert_eq!(libc::mincore(addr, len, vec.as_mut_ptr().cast()), 0);
            libc::munmap(addr, len);
        }
        vec.iter().filter(|&&b| b & 1 != 0).count()
    }

    /// FR-26: after an uncached copy, no page of the file (including a partial last
    /// page) is left in RAM for the verify read to hit.
    #[test]
    fn uncached_copies_leave_nothing_in_the_page_cache() {
        use crate::control::JobControl;
        use crate::copy::{CopyConfig, copy_to_partial};
        let dir = tempfile::tempdir().unwrap();
        // One file for the single-read path, one for the pipeline; both end mid-page.
        for (name, len, buffer_size) in [("small", 50_000, 1 << 20), ("large", 300_123, 65_536)] {
            let src = dir.path().join(name);
            std::fs::write(&src, vec![7u8; len]).unwrap();
            let cfg = CopyConfig {
                buffer_size,
                buffers: 3,
                uncached_write: true,
            };
            let dst = dir.path().join(format!("{name}.copy"));
            let pc = copy_to_partial(&src, &dst, &cfg, &|_| {}, &JobControl::new()).unwrap();
            assert_eq!(resident_pages(&pc.partial), 0, "{name}");
        }
    }
}
