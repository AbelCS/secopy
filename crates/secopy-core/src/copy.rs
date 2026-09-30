//! Copies one file to a temporary "partial" file while hashing it (FR-18, FR-20, RFD §7.2).

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::control::JobControl;
use crate::error::FileError;
use crate::{hash, metadata, os};

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

/// A fully written and flushed copy that still has its temporary name. It stays open, and
/// so locked, until it is committed or discarded: no other job can mistake it for a
/// partial file left by an interrupted job (FR-18).
#[derive(Debug)]
pub struct PartialCopy {
    pub partial: PathBuf,
    /// xxHash64 of the bytes read from the source.
    pub hash: u64,
    pub bytes: u64,
    /// A partial file left by an interrupted job was removed to make room for this one.
    pub removed_stale: bool,
    file: File,
}

/// How a finished copy takes its final name (FR-17, FR-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commit<'a> {
    /// Never replace an existing file.
    NoReplace,
    /// Replace an existing file, atomically.
    Replace,
    /// Like `NoReplace`; if the name was taken since pre-flight, try `original (n+1)`,
    /// `original (n+2)`, … instead.
    KeepBoth { original: &'a Path, n: u32 },
}

/// Names tried when a Keep-both name is taken at commit time.
const KEEP_BOTH_TRIES: u32 = 100;

impl PartialCopy {
    /// Gives the file its final name and returns the path it got. The file system itself
    /// decides whether a name is taken (case, Unicode normalization). On failure the
    /// partial file is removed.
    /// Which file this copy is, from its open handle (#115).
    pub fn landed(&self) -> Option<crate::job::Landed> {
        self.file
            .metadata()
            .ok()
            .map(|m| crate::job::Landed::of(&m))
    }

    pub fn commit(self, final_path: &Path, how: Commit) -> Result<PathBuf, FileError> {
        self.release(|partial| {
            let result = match how {
                Commit::NoReplace => {
                    commit_noreplace(partial, final_path).map(|()| final_path.to_path_buf())
                }
                Commit::Replace => commit_replace(partial, final_path),
                Commit::KeepBoth { original, n } => {
                    commit_keep_both(partial, final_path, original, n)
                }
            };
            if result.is_err() {
                let _ = fs::remove_file(partial);
            }
            result
        })
        .unwrap_or(Err(FileError::PartialInUse))
    }

    /// Deletes the partial file.
    pub fn discard(self) {
        self.release(|partial| {
            let _ = fs::remove_file(partial);
        });
    }

    /// Runs `op` on the partial path; `None` if the name no longer belongs to this file.
    /// The file stays open, and locked, until `op` has renamed or removed it, so no other
    /// job can take the name in between. Where locks don't separate writers (some network
    /// file systems), another job may already have replaced it by name; then the name is
    /// not ours to rename or delete.
    fn release<T>(self, op: impl FnOnce(&Path) -> T) -> Option<T> {
        let PartialCopy { partial, file, .. } = self;
        // An error here (e.g. the drive is gone) is left to `op` to report.
        if let Ok(false) = os::is_at(&file, &partial) {
            return None;
        }
        let out = op(&partial);
        drop(file);
        Some(out)
    }
}

/// Longest file name every supported file system accepts, in bytes and in UTF-16 units.
const MAX_NAME: usize = 255;

/// Temporary name used while a file is written, in the same directory:
/// `.<name>.secopy-partial`, or `.secopy-<hash>.partial` if that would be too long.
/// The hash ignores case, so two names a case-insensitive drive treats as one file
/// still map to one partial file.
pub fn partial_path(final_path: &Path) -> PathBuf {
    let name = final_path
        .file_name()
        .expect("destination path has a file name");
    let mut partial = OsString::from(".");
    partial.push(name);
    partial.push(".secopy-partial");
    if partial.len() <= MAX_NAME && partial.to_string_lossy().encode_utf16().count() <= MAX_NAME {
        return final_path.with_file_name(partial);
    }
    let key = hash::hash_bytes(name.to_string_lossy().to_lowercase().as_bytes());
    final_path.with_file_name(format!(".secopy-{}.partial", hash::to_hex(key)))
}

/// Copies `src` to the partial path of `final_path`, hashing the bytes as they are read,
/// then fsyncs it. On error or cancel the partial file is removed.
/// `progress` receives the number of bytes written so far.
///
/// A partial file that is already there is replaced if it was left by an interrupted job.
/// If a live writer holds it (another job, or a clashing name), this file fails with
/// `PartialInUse` and the other writer's data is left alone.
pub fn copy_to_partial(
    src: &Path,
    final_path: &Path,
    cfg: &CopyConfig,
    progress: &dyn Fn(u64),
    control: &JobControl,
) -> Result<PartialCopy, FileError> {
    let partial = partial_path(final_path);
    let reader = File::open(src).map_err(FileError::read_source)?;
    let (mut writer, removed_stale) = create_partial(&partial)?;
    match copy_inner(reader, &mut writer, cfg, progress, control) {
        Ok((hash, bytes)) => Ok(PartialCopy {
            partial,
            hash,
            bytes,
            removed_stale,
            file: writer,
        }),
        Err(e) => {
            // Close before removing.
            drop(writer);
            let _ = fs::remove_file(&partial);
            Err(e)
        }
    }
}

/// Creates the partial file, replacing one left by an interrupted job. Returns whether a
/// stale file was removed.
fn create_partial(partial: &Path) -> Result<(File, bool), FileError> {
    let mut removed = false;
    // A few rounds: another job's cleanup can race with ours.
    for _ in 0..3 {
        match os::create_locked(partial) {
            Ok(file) => return Ok((file, removed)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if !os::remove_stale(partial).map_err(FileError::write_dest)? {
                    return Err(FileError::PartialInUse);
                }
                removed = true;
            }
            Err(e) => return Err(FileError::write_dest(e)),
        }
    }
    Err(FileError::PartialInUse)
}

fn commit_replace(partial: &Path, final_path: &Path) -> Result<PathBuf, FileError> {
    fs::rename(partial, final_path).map_err(FileError::write_dest)?;
    Ok(final_path.to_path_buf())
}

fn commit_keep_both(
    partial: &Path,
    planned: &Path,
    original: &Path,
    n: u32,
) -> Result<PathBuf, FileError> {
    let later = (n + 1..n + KEEP_BOTH_TRIES).map(|m| crate::names::numbered(original, m));
    for candidate in std::iter::once(planned.to_path_buf()).chain(later) {
        match commit_noreplace(partial, &candidate) {
            Err(FileError::AlreadyExists) => continue,
            other => return other.map(|()| candidate),
        }
    }
    Err(FileError::AlreadyExists)
}

/// Uses the OS's no-replace rename. File systems without one (FAT and exFAT) fall back to
/// a hard link, and then to check-then-rename.
fn commit_noreplace(partial: &Path, final_path: &Path) -> Result<(), FileError> {
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
    let src_meta = reader.metadata().map_err(FileError::read_source)?;
    let len = src_meta.len();
    if cfg.uncached_write {
        os::set_nocache(writer);
    }
    let result = if len <= cfg.buffer_size as u64 {
        copy_small(&mut reader, writer, len, progress, control)?
    } else {
        copy_pipelined(reader, writer, cfg, progress, control)?
    };
    metadata::copy_to(&src_meta, writer).map_err(FileError::write_dest)?;
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
    // One byte past its size at most: enough to tell a file that grew since the scan (it
    // fails as changed), without reading it into memory without end (#118).
    reader
        .take(len + 1)
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

#[cfg(test)]
mod small_tests {
    use super::*;

    /// QA review (#118): a small file that grew since the scan (a recording in progress) is
    /// read one byte past its size, enough to tell it changed, not into memory without end.
    #[test]
    fn a_small_file_that_grew_is_read_one_byte_past_its_size() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("growing");
        std::fs::write(&src, vec![7u8; 100_000]).unwrap();
        let mut reader = File::open(&src).unwrap();
        let mut writer = File::create(dir.path().join("copy")).unwrap();
        let (_, read) =
            copy_small(&mut reader, &mut writer, 10, &|_| {}, &JobControl::new()).unwrap();
        assert_eq!(read, 11);
    }
}
