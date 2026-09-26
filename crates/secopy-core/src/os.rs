//! Platform-specific flushing and cache control (FR-18, FR-26, RFD §7.4).

use std::fs::File;
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

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn rename_noreplace(_from: &Path, _to: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn c_path(path: &Path) -> io::Result<std::ffi::CString> {
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
