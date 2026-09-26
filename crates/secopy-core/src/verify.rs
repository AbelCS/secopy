//! Re-reads a written file from the device and hashes it (FR-25, FR-26, RFD §7.3).

use std::alloc::{self, Layout};
use std::io::{self, Read};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::FileError;
use crate::{hash, os};

/// Whether the verify read bypassed the OS page cache (FR-26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheBypass {
    Active,
    Unavailable,
}

/// Hashes `path` reading it from the device. `progress` receives bytes read so far.
pub fn hash_from_device(
    path: &Path,
    buffer_size: usize,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<(u64, CacheBypass), FileError> {
    let (mut file, bypassed) = os::open_uncached(path).map_err(FileError::read_back)?;
    let size = file.metadata().map_err(FileError::read_back)?.len();
    let mut buf = AlignedBuf::new(buffer_size);
    let mut hasher = hash::hasher();
    let mut done = 0u64;
    // Driven by the file size: with unbuffered I/O on Windows a read after a short
    // (unaligned) read fails, and on Unix a short read before EOF must not end the hash.
    while done < size {
        if cancel.load(Ordering::Relaxed) {
            return Err(FileError::Cancelled);
        }
        let chunk = buf.as_mut_slice();
        let n = read_once(&mut file, chunk).map_err(FileError::read_back)?;
        if n == 0 {
            return Err(FileError::read_back(io::ErrorKind::UnexpectedEof.into()));
        }
        hasher.update(&chunk[..n]);
        done += n as u64;
        progress(done);
    }
    let bypass = if bypassed {
        CacheBypass::Active
    } else {
        CacheBypass::Unavailable
    };
    Ok((hasher.digest(), bypass))
}

fn read_once(file: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match file.read(buf) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            other => return other,
        }
    }
}

/// Page-aligned buffer whose length is a multiple of 4096, as unbuffered I/O requires.
struct AlignedBuf {
    ptr: NonNull<u8>,
    layout: Layout,
}

const ALIGN: usize = 4096;

impl AlignedBuf {
    fn new(len: usize) -> Self {
        let len = len.max(ALIGN).next_multiple_of(ALIGN);
        let layout = Layout::from_size_align(len, ALIGN).expect("valid buffer layout");
        // SAFETY: `layout` has a non-zero size.
        let raw = unsafe { alloc::alloc_zeroed(layout) };
        let ptr = NonNull::new(raw).unwrap_or_else(|| alloc::handle_alloc_error(layout));
        Self { ptr, layout }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: `ptr` points to `layout.size()` initialised bytes owned by `self`.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.layout.size()) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        // SAFETY: allocated in `new` with this exact layout.
        unsafe { alloc::dealloc(self.ptr.as_ptr(), self.layout) }
    }
}
