//! Platform-specific flushing and cache control (FR-18, FR-26, RFD §7.4).

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// Pushes a file's data to the device. On macOS, `File::sync_all` is `F_FULLFSYNC`,
/// which also flushes the drive's cache and costs milliseconds per call; per file we
/// use plain `fsync` and flush the drive cache once per job with [`full_barrier`].
#[cfg(target_os = "macos")]
pub fn sync_file(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: `fsync` on a valid descriptor owned by `file`.
    if unsafe { libc::fsync(file.as_raw_fd()) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn sync_file(file: &File) -> io::Result<()> {
    file.sync_all()
}

/// Makes everything written to the volume holding `dir` durable, including the
/// drive's own cache. Called once at the end of a job.
#[cfg(target_os = "macos")]
pub fn full_barrier(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(target_os = "macos"))]
pub fn full_barrier(_dir: &Path) -> io::Result<()> {
    Ok(())
}

/// Asks the OS not to keep this file's pages in cache. Only macOS supports this per
/// file descriptor; elsewhere it is a no-op. Returns true if the request was accepted.
#[cfg(target_os = "macos")]
pub fn set_nocache(file: &File) -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: `fcntl` on a valid descriptor owned by `file`, with an integer argument.
    unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) != -1 }
}

#[cfg(not(target_os = "macos"))]
pub fn set_nocache(_file: &File) -> bool {
    false
}

/// Opens `path` so reads come from the device, not the page cache.
/// The bool is false when the platform or file system cannot guarantee that.
#[cfg(target_os = "macos")]
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    let file = File::open(path)?;
    let bypassed = set_nocache(&file);
    Ok((file, bypassed))
}

/// Linux: the file was fsynced, so its cached pages are clean and DONTNEED evicts them.
#[cfg(target_os = "linux")]
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    use std::os::fd::AsRawFd;
    let file = File::open(path)?;
    // SAFETY: `posix_fadvise` on a valid descriptor owned by `file`.
    let rc = unsafe { libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) };
    Ok((file, rc == 0))
}

/// Windows: FILE_FLAG_NO_BUFFERING. Reads must use sector-aligned buffers and sizes.
#[cfg(windows)]
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
    match OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_NO_BUFFERING)
        .open(path)
    {
        Ok(file) => Ok((file, true)),
        Err(_) => Ok((File::open(path)?, false)),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    Ok((File::open(path)?, false))
}

/// Renames `from` to `to` without replacing an existing `to`, in one system call.
/// Errors: `AlreadyExists` if the name is taken (as the file system judges names:
/// case, Unicode normalization), `Unsupported` if this OS or file system can't do it.
#[cfg(target_os = "macos")]
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

#[cfg(target_os = "linux")]
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    let (from, to) = (c_path(from)?, c_path(to)?);
    // SAFETY: both pointers are valid NUL-terminated strings for the call's duration.
    let rc = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(unsupported_or(io::Error::last_os_error()))
    }
}

/// Windows: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` fails if the name is taken.
/// It also fails on a file another writer has open without delete sharing.
#[cfg(windows)]
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
    let wide = |p: &Path| -> Vec<u16> { p.as_os_str().encode_wide().chain([0]).collect() };
    let (from, to) = (wide(from), wide(to));
    // SAFETY: both pointers are valid NUL-terminated wide strings for the call's duration.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub fn rename_noreplace(_from: &Path, _to: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn c_path(path: &Path) -> io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

/// File systems without the flag report ENOTSUP (macOS) or EINVAL (Linux).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unsupported_or(e: io::Error) -> io::Error {
    match e.raw_os_error() {
        Some(libc::ENOTSUP | libc::EINVAL | libc::ENOSYS) => io::ErrorKind::Unsupported.into(),
        _ => e,
    }
}

/// Creates a new partial file that stays locked while it is open, so another job can tell
/// a live partial file from one left by an interrupted job (FR-18). On Unix this is an
/// advisory `flock`; on Windows the file is opened without delete sharing, so nobody can
/// delete or rename it while it is open.
pub fn create_locked(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
        opts.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    }
    let file = opts.open(path)?;
    // Only another job's cleanup can hold a brand-new file's lock, and only for a moment,
    // so wait for it. That cleanup may have removed the new file: then the name is no
    // longer ours. Lock errors (file systems without locks) leave the file unlocked.
    #[cfg(unix)]
    {
        let _ = lock(&file, true);
        if !is_at(&file, path)? {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
    }
    Ok(file)
}

/// Removes a partial file left by an interrupted job. Returns `false`, and leaves the file
/// alone, if a live writer still holds it.
#[cfg(unix)]
pub fn remove_stale(path: &Path) -> io::Result<bool> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(e),
    };
    if let Ok(false) = lock(&file, false) {
        return Ok(false);
    }
    // Creating and locking a file are two steps on Unix: a file this young may be one
    // another writer has just created and not locked yet.
    let age = file.metadata()?.modified()?.elapsed().unwrap_or_default();
    if age < STALE_AFTER {
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

#[cfg(windows)]
pub fn remove_stale(path: &Path) -> io::Result<bool> {
    use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(e) if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32) => Ok(false),
        Err(e) => Err(e),
    }
}

/// A partial file younger than this is never treated as left by an interrupted job.
#[cfg(unix)]
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether `path` still names the open `file`.
#[cfg(unix)]
fn is_at(file: &File, path: &Path) -> io::Result<bool> {
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
#[cfg(unix)]
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

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;

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
    #[cfg(target_os = "macos")]
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

    /// FR-26: `open_uncached` evicts the fsynced file's pages before the verify read.
    #[cfg(target_os = "linux")]
    #[test]
    fn opening_for_verify_evicts_the_page_cache() {
        let dir = tempfile::tempdir().unwrap();
        if crate::fsinfo::fs_info(dir.path()).unwrap().kind
            == crate::fsinfo::FsKind::Other("0x1021994".into())
        {
            return; // tmpfs: the page cache is the storage
        }
        let path = dir.path().join("a.bin");
        let mut f = File::create(&path).unwrap();
        std::io::Write::write_all(&mut f, &vec![7u8; 300_123]).unwrap();
        f.sync_all().unwrap();
        drop(f);
        assert!(resident_pages(&path) > 0, "a normal write is cached");
        let (_file, bypassed) = open_uncached(&path).unwrap();
        assert!(bypassed);
        assert_eq!(resident_pages(&path), 0);
    }
}
