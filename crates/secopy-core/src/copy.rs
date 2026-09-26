//! Copies one file to a temporary "partial" file while hashing it (FR-18, FR-20, RFD §7.2).

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::control::JobControl;
use crate::error::FileError;
use crate::{hash, os};

/// Tuning for the copy pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyConfig {
    /// Size of each I/O buffer. Files up to this size are copied in one read.
    pub buffer_size: usize,
    /// Buffers in flight between the reader and writer threads (min 2).
    pub buffers: usize,
    /// Keep written data out of the OS cache (macOS), so a later verify reads the device.
    pub uncached_write: bool,
}

impl Default for CopyConfig {
    fn default() -> Self {
        Self {
            buffer_size: 4 << 20,
            buffers: 4,
            uncached_write: false,
        }
    }
}

/// A fully written and flushed copy that still has its temporary name.
#[derive(Debug)]
pub struct PartialCopy {
    pub partial: PathBuf,
    /// xxHash64 of the bytes read from the source.
    pub hash: u64,
    pub bytes: u64,
}

/// Temporary name used while a file is written: `.<name>.secopy-partial`, same directory.
pub fn partial_path(final_path: &Path) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(
        final_path
            .file_name()
            .expect("destination path has a file name"),
    );
    name.push(".secopy-partial");
    final_path.with_file_name(name)
}

/// Copies `src` to the partial path of `final_path`, hashing the bytes as they are read,
/// then fsyncs it. On error or cancel the partial file is removed.
/// `progress` receives the number of bytes written so far.
///
/// The partial file is created with `create_new`: if it already exists, another writer
/// (a clashing name in this job, or another job) owns it, and this file fails with
/// `NameClash` instead of truncating the other writer's data.
pub fn copy_to_partial(
    src: &Path,
    final_path: &Path,
    cfg: &CopyConfig,
    progress: &dyn Fn(u64),
    control: &JobControl,
) -> Result<PartialCopy, FileError> {
    let partial = partial_path(final_path);
    let reader = File::open(src).map_err(FileError::read_source)?;
    let mut writer = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
    {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(FileError::NameClash),
        Err(e) => return Err(FileError::write_dest(e)),
    };
    let result = copy_inner(reader, &mut writer, cfg, progress, control);
    // Close before removing: Windows cannot delete an open file.
    drop(writer);
    match result {
        Ok((hash, bytes)) => Ok(PartialCopy {
            partial,
            hash,
            bytes,
        }),
        Err(e) => {
            let _ = fs::remove_file(&partial);
            Err(e)
        }
    }
}

/// Gives a finished partial file its final name without ever replacing an existing file
/// (FR-18). The file system itself decides whether the name is taken (case, Unicode
/// normalization). Uses a no-replace rename where the OS has one (macOS, Linux), else a
/// hard link (Windows); file systems with neither (FAT, exFAT) fall back to
/// check-then-rename.
pub fn commit(partial: &Path, final_path: &Path) -> Result<(), FileError> {
    match os::rename_noreplace(partial, final_path) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(FileError::AlreadyExists),
        Err(e) if e.kind() != io::ErrorKind::Unsupported => return Err(FileError::write_dest(e)),
        Err(_) => {}
    }
    match fs::hard_link(partial, final_path) {
        Ok(()) => {
            // The copy is complete under its final name; a leftover partial is only clutter.
            let _ = fs::remove_file(partial);
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(FileError::AlreadyExists),
        Err(_) => {
            if fs::symlink_metadata(final_path).is_ok() {
                return Err(FileError::AlreadyExists);
            }
            fs::rename(partial, final_path).map_err(FileError::write_dest)
        }
    }
}

fn copy_inner(
    mut reader: File,
    writer: &mut File,
    cfg: &CopyConfig,
    progress: &dyn Fn(u64),
    control: &JobControl,
) -> Result<(u64, u64), FileError> {
    let len = reader.metadata().map_err(FileError::read_source)?.len();
    if cfg.uncached_write {
        os::set_nocache(writer);
    }
    let result = if len <= cfg.buffer_size as u64 {
        copy_small(&mut reader, writer, len, progress, control)?
    } else {
        copy_pipelined(reader, writer, cfg, progress, control)?
    };
    os::sync_file(writer).map_err(FileError::write_dest)?;
    Ok(result)
}

/// Small files: one read, one write, no extra thread.
fn copy_small(
    reader: &mut File,
    writer: &mut File,
    len: u64,
    progress: &dyn Fn(u64),
    control: &JobControl,
) -> Result<(u64, u64), FileError> {
    control.checkpoint()?;
    let mut buf = Vec::with_capacity(len as usize);
    reader
        .read_to_end(&mut buf)
        .map_err(FileError::read_source)?;
    writer.write_all(&buf).map_err(FileError::write_dest)?;
    progress(buf.len() as u64);
    Ok((hash::hash_bytes(&buf), buf.len() as u64))
}

/// Large files: a reader thread fills and hashes buffers while this thread writes them,
/// so reading chunk N+1 overlaps writing chunk N.
fn copy_pipelined(
    mut reader: File,
    writer: &mut File,
    cfg: &CopyConfig,
    progress: &dyn Fn(u64),
    control: &JobControl,
) -> Result<(u64, u64), FileError> {
    let buffers = cfg.buffers.max(2);
    let (full_tx, full_rx) = mpsc::sync_channel::<(Vec<u8>, usize)>(buffers);
    let (empty_tx, empty_rx) = mpsc::sync_channel::<Vec<u8>>(buffers);
    for _ in 0..buffers {
        empty_tx
            .send(vec![0; cfg.buffer_size])
            .expect("channel has room for every buffer");
    }
    std::thread::scope(|s| {
        let reader_thread = s.spawn(move || -> Result<u64, FileError> {
            let mut hasher = hash::hasher();
            while let Ok(mut buf) = empty_rx.recv() {
                // Pausing stops the reader too, not only the writer (FR-22).
                control.checkpoint()?;
                let n = read_full(&mut reader, &mut buf).map_err(FileError::read_source)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                if full_tx.send((buf, n)).is_err() {
                    break; // writer stopped
                }
            }
            Ok(hasher.digest())
        });
        let written = write_chunks(&full_rx, &empty_tx, writer, progress, control);
        // Unblock the reader if the writer stopped early.
        drop(full_rx);
        drop(empty_tx);
        let read = reader_thread.join().expect("reader thread panicked");
        let written = written?;
        let hash = read?;
        Ok((hash, written))
    })
}

fn write_chunks(
    full_rx: &mpsc::Receiver<(Vec<u8>, usize)>,
    empty_tx: &mpsc::SyncSender<Vec<u8>>,
    writer: &mut File,
    progress: &dyn Fn(u64),
    control: &JobControl,
) -> Result<u64, FileError> {
    let mut written = 0u64;
    for (buf, n) in full_rx.iter() {
        control.checkpoint()?;
        writer.write_all(&buf[..n]).map_err(FileError::write_dest)?;
        written += n as u64;
        progress(written);
        let _ = empty_tx.send(buf);
    }
    Ok(written)
}

/// Reads until `buf` is full or EOF; returns the number of bytes read.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}
