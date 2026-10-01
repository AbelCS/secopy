//! Runs a copy job: copy lanes, verify lanes, progress events and the checksum file (RFD §7).

mod progress;
mod runner;
mod undo;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use chrono::Local;

use crate::awake::KeepAwake;
use crate::checksum_file;
use crate::copy::CopyConfig;
use crate::error::{FatalError, FileError, IoFailure};
use crate::plan::Plan;
use crate::scan::{DirEntry, ScanProblem};
use crate::verify::CacheBypass;
use crate::{fsinfo, metadata, os};

pub use crate::control::JobControl;
pub use progress::{ActiveFile, Phase, Progress, Recording};
use runner::{Queue, Runner, VERIFY_QUEUE_PER_LANE, VerifyTask};
pub use undo::{Undone, undo};

#[derive(Debug, Clone)]
pub struct JobOptions {
    /// Copy & Verify mode (FR-25).
    pub verify: bool,
    /// Write the `.xxh64` checksum file (FR-29).
    pub write_checksum_file: bool,
    pub copy: CopyConfig,
    /// Files up to this size go to the small-file lanes (RFD §7.2).
    pub small_file_threshold: u64,
    pub small_file_lanes: usize,
    pub large_file_lanes: usize,
    pub verify_lanes: usize,
    /// Keep the system from sleeping while the job runs, pauses included.
    pub keep_awake: bool,
    /// How often `Event::Progress` is emitted.
    pub progress_interval: Duration,
    /// Mirror (plan 7): an `Overwrite` file's old version is moved here, keeping its
    /// relative path, right before its verified copy replaces it.
    pub archive_replaced: Option<PathBuf>,
    /// Record an ASC MHL history once the copy is durable (#154); `None` writes none.
    pub mhl: Option<crate::mhl::MhlJob>,
    #[doc(hidden)]
    pub hooks: Hooks,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            verify: true,
            write_checksum_file: true,
            copy: CopyConfig::default(),
            // Equal to the buffer size, so small-file lanes never take the pipelined path
            // and buffer memory stays bounded (NFR-4).
            small_file_threshold: 4 << 20,
            small_file_lanes: 8,
            large_file_lanes: 1,
            verify_lanes: 2,
            keep_awake: true,
            progress_interval: Duration::from_millis(50),
            archive_replaced: None,
            mhl: None,
            hooks: Hooks::default(),
        }
    }
}

/// Fault injection for tests. Not part of the stable API.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Hooks {
    /// Called with the source file before each copy attempt.
    pub before_copy: Option<fn(&Path)>,
    /// Called with the partial file after each copy attempt (0 = first), before verifying.
    pub after_copy: Option<fn(&Path, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Copied,
    Verified,
    /// Not read or written (FR-17).
    Skipped(SkipReason),
    Failed(FileError),
    /// Stopped by Cancel before it was done; nothing of it is left at the destination.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Already at the destination with the same size and modification time. Not checked.
    Identical,
    /// A different file has this name, and the user chose Skip.
    Differs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOutcome {
    /// Index in `Plan::files`.
    pub id: usize,
    /// Path relative to the destination, as selected.
    pub rel: PathBuf,
    /// Where the copy landed; differs from `rel` for Keep both.
    pub final_rel: PathBuf,
    pub size: u64,
    /// Source hash; `None` if the file wasn't read or failed before it was hashed.
    pub hash: Option<u64>,
    pub status: FileStatus,
    /// Listed in this job's checksum file.
    pub in_checksum_file: bool,
    pub elapsed: Duration,
    /// The copy as it got its name: undo removes only that file, unchanged (#115).
    pub landed_as: Option<Landed>,
}

/// Which file a copy is, taken from the open copy just before it got its name: the same
/// device and inode after the rename, and the date the job gave it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Landed {
    pub dev: u64,
    pub ino: u64,
    pub mtime: Option<std::time::SystemTime>,
}

impl Landed {
    pub fn of(meta: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Landed {
            dev: meta.dev(),
            ino: meta.ino(),
            mtime: meta.modified().ok(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Progress(Progress),
    FileFinished(FileOutcome),
}

#[derive(Debug, Clone)]
pub struct JobReport {
    /// In the order files finished.
    pub outcomes: Vec<FileOutcome>,
    /// Files never started because the job was cancelled or hit a fatal error.
    pub not_started: u64,
    pub checksum_file: Option<PathBuf>,
    pub checksum_error: Option<IoFailure>,
    /// The checksum file was turned off (`JobOptions::write_checksum_file`).
    pub checksum_off: bool,
    /// `None` when not verifying.
    pub cache_bypass: Option<CacheBypass>,
    /// Partial files left by interrupted jobs that were removed (FR-18).
    pub removed_partials: u64,
    pub fatal: Option<FatalError>,
    pub cancelled: bool,
    pub elapsed: Duration,
    /// Directories this job created (full paths), so `undo` removes only those.
    pub created_dirs: Vec<PathBuf>,
    /// What the scan couldn't read, from the plan: not copied (#58).
    pub unread: Vec<ScanProblem>,
    /// The device reported an error while the copy was made durable (#58).
    pub durability_error: Option<IoFailure>,
    /// Empty directories that couldn't be created, with why (#58).
    pub dir_errors: Vec<(PathBuf, IoFailure)>,
    /// ASC MHL generations written, deepest first, so `undo` can take them back (#154).
    pub mhl_written: Vec<crate::mhl::write::Written>,
    /// ASC MHL couldn't be read or written; nothing of it was left.
    pub mhl_error: Option<IoFailure>,
    /// Files that don't match their earlier ASC MHL hash, from the copy root.
    pub mhl_failed: Vec<PathBuf>,
    /// ASC MHL wasn't asked for (`JobOptions::mhl`).
    pub mhl_off: bool,
}

impl JobReport {
    pub fn failed(&self) -> impl Iterator<Item = &FileOutcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, FileStatus::Failed(_)))
    }

    pub fn skipped(&self) -> impl Iterator<Item = &FileOutcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, FileStatus::Skipped(_)))
    }

    /// Every file copied (and verified), nothing left unread, and the checksum file written
    /// when it was asked for.
    pub fn is_success(&self) -> bool {
        self.fatal.is_none()
            && !self.cancelled
            && self.not_started == 0
            && self.failed().next().is_none()
            && self.unread.is_empty()
            && self.checksum_error.is_none()
            && self.durability_error.is_none()
            && self.dir_errors.is_empty()
            && self.mhl_error.is_none()
            && self.mhl_failed.is_empty()
    }
}

/// Carries out `plan`. Blocks until done; call from a worker thread.
/// `on_event` is called from several threads.
pub fn run_job(
    plan: &Plan,
    opts: &JobOptions,
    control: &JobControl,
    on_event: &(dyn Fn(Event) + Sync),
) -> JobReport {
    let started = Instant::now();
    let _awake = opts.keep_awake.then(KeepAwake::new);
    let dest = plan.dest.as_path();
    let runner = Runner::new(plan, opts, control, on_event);
    let (write, unwritten): (Vec<usize>, Vec<usize>) =
        (0..plan.files.len()).partition(|&i| plan.files[i].action.writes());
    for idx in unwritten {
        runner.finish_unwritten(idx);
    }
    let (small, large): (Vec<usize>, Vec<usize>) = write
        .into_iter()
        .partition(|&i| plan.files[i].entry.size <= opts.small_file_threshold);
    let small = Queue::new(small);
    let large = Queue::new(large);
    std::thread::scope(|s| {
        // Dropping the sender ends the ticker at once: when the workers are done, and also
        // when joining one panics below (otherwise the scope would wait for the ticker, and
        // the panic would become a hang).
        let (stop_ticker, stop) = mpsc::channel::<()>();
        let ticker = {
            let runner = &runner;
            s.spawn(move || {
                while let Err(mpsc::RecvTimeoutError::Timeout) =
                    stop.recv_timeout(opts.progress_interval)
                {
                    runner.emit_progress();
                }
            })
        };
        // Bounded, so copying can't run thousands of files ahead of verification.
        let (verify_tx, verify_rx) =
            mpsc::sync_channel::<VerifyTask>(VERIFY_QUEUE_PER_LANE * opts.verify_lanes.max(1));
        // Owned by the verify lanes only: if they all die, sends fail instead of blocking.
        let verify_rx = Arc::new(Mutex::new(verify_rx));
        let mut workers = Vec::new();
        for (queue, lanes) in [
            (&small, opts.small_file_lanes),
            (&large, opts.large_file_lanes),
        ] {
            for _ in 0..lanes.max(1) {
                let (runner, verify_tx) = (&runner, verify_tx.clone());
                workers.push(s.spawn(move || runner.copy_lane(queue, &verify_tx)));
            }
        }
        // Verify lanes end when every copy lane has dropped its sender.
        drop(verify_tx);
        if opts.verify {
            for _ in 0..opts.verify_lanes.max(1) {
                let (runner, verify_rx) = (&runner, verify_rx.clone());
                workers.push(s.spawn(move || runner.verify_lane(&verify_rx)));
            }
        }
        drop(verify_rx);
        for worker in workers {
            worker.join().expect("worker thread panicked");
        }
        drop(stop_ticker);
        ticker.join().expect("progress thread panicked");
        runner.emit_progress();
    });

    let bypass_unavailable = runner.bypass_unavailable.load(Relaxed);
    let outcomes = std::mem::take(&mut *runner.outcomes.lock().expect("outcomes lock poisoned"));
    let fatal = runner.fatal.lock().expect("fatal lock poisoned").take();
    let mut created_dirs =
        std::mem::take(&mut *runner.made_dirs.lock().expect("dirs lock poisoned"));
    let mut removed_partials = runner.removed_partials.load(Relaxed);
    let mut dir_errors = Vec::new();
    if !control.is_stopped() && fatal.is_none() {
        // Before the folder times: removing a file changes its folder's time.
        removed_partials += remove_leftover_partials(plan);
        let (made, errors) = create_empty_dirs(plan);
        created_dirs.extend(made);
        dir_errors = errors;
        restore_dir_mtimes(plan, &created_dirs);
    }
    let (checksum_file, checksum_error) = if opts.write_checksum_file {
        write_checksum(dest, &outcomes)
    } else {
        (None, None)
    };
    let durability_error = make_durable(dest, &plan.dirs, plan.fs.device);
    // After the copy is on disk; not for a job that stopped or failed as a whole.
    let recorded = match &opts.mhl {
        Some(job) if fatal.is_none() && !control.is_stopped() => {
            let total = job.plan.to_read_bytes;
            let last = Mutex::new(None::<Instant>);
            crate::mhl::run::record(plan, job, &outcomes, control, &|bytes| {
                *runner.recording.lock().expect("recording lock poisoned") =
                    Some(Recording { bytes, total });
                let mut last = last.lock().expect("last lock poisoned");
                if last.is_none_or(|t| t.elapsed() >= opts.progress_interval) || bytes == total {
                    *last = Some(Instant::now());
                    runner.emit_progress();
                }
            })
        }
        _ => crate::mhl::run::Recorded::default(),
    };
    JobReport {
        not_started: (plan.files.len() - outcomes.len()) as u64,
        outcomes,
        checksum_file,
        checksum_error,
        checksum_off: !opts.write_checksum_file,
        cache_bypass: opts.verify.then_some(if bypass_unavailable {
            CacheBypass::Unavailable
        } else {
            CacheBypass::Active
        }),
        removed_partials,
        cancelled: control.is_stopped() && fatal.is_none(),
        fatal,
        elapsed: started.elapsed(),
        created_dirs,
        unread: plan.unread.clone(),
        durability_error,
        dir_errors,
        mhl_written: recorded.written,
        mhl_error: recorded.error,
        mhl_failed: recorded.failed,
        mhl_off: opts.mhl.is_none(),
    }
}

/// Partial files left by an interrupted job next to files this job didn't write (skipped
/// or failed); the ones for written files were replaced when those were copied (FR-18).
fn remove_leftover_partials(plan: &Plan) -> u64 {
    plan.stale_partials
        .iter()
        .filter(|p| fs::symlink_metadata(p).is_ok() && matches!(os::remove_stale(p), Ok(true)))
        .count() as u64
}

/// Source folders with no files in the plan are created at the end (FR-6). Folders with
/// files were created when their first file started. Returns the folders made, and the ones
/// that couldn't be.
fn create_empty_dirs(plan: &Plan) -> (Vec<PathBuf>, Vec<(PathBuf, IoFailure)>) {
    let mut with_files = HashSet::new();
    for file in &plan.files {
        for dir in file.entry.rel.ancestors().skip(1) {
            if !with_files.insert(dir) {
                break;
            }
        }
    }
    let mut made = Vec::new();
    let mut errors = Vec::new();
    for dir in plan
        .dirs
        .iter()
        .filter(|d| !with_files.contains(d.rel.as_path()))
    {
        let path = plan.dest.join(&dir.rel);
        let missing: Vec<PathBuf> = path
            .ancestors()
            .take_while(|d| fs::symlink_metadata(d).is_err())
            .map(Path::to_path_buf)
            .collect();
        match fs::create_dir_all(&path) {
            Ok(()) => made.extend(missing),
            Err(e) => errors.push((dir.rel.clone(), e.into())),
        }
    }
    (made, errors)
}

/// Deepest folders first: setting a folder's time doesn't change its parent's (FR-19). Only
/// folders this job created: one already at the destination keeps its own (#115).
fn restore_dir_mtimes(plan: &Plan, created: &[PathBuf]) {
    let created: HashSet<&Path> = created.iter().map(PathBuf::as_path).collect();
    let mut dirs: Vec<&DirEntry> = plan.dirs.iter().collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.rel.components().count()));
    for dir in dirs {
        let path = plan.dest.join(&dir.rel);
        if let Some(mtime) = dir.mtime
            && created.contains(path.as_path())
        {
            let _ = metadata::set_dir_mtime(&path, mtime);
        }
    }
}

fn write_checksum(dest: &Path, outcomes: &[FileOutcome]) -> (Option<PathBuf>, Option<IoFailure>) {
    let entries: Vec<(PathBuf, u64)> = outcomes
        .iter()
        .filter(|o| o.in_checksum_file)
        .filter_map(|o| o.hash.map(|h| (o.final_rel.clone(), h)))
        .collect();
    if entries.is_empty() {
        return (None, None);
    }
    match checksum_file::write(dest, &entries, Local::now()) {
        Ok(path) => (Some(path), None),
        Err(e) => (None, Some(e.into())),
    }
}

/// Makes the job durable (RFD §7.4): one fsync per directory for the new names, then one
/// drive-cache flush for the whole volume. `Some` when the device reported an error doing so:
/// the destination can't confirm the files are on disk.
fn make_durable(dest: &Path, dirs: &[DirEntry], device: u64) -> Option<IoFailure> {
    // Another drive at the destination's path (the job's was pulled out): nothing it can
    // confirm is this job's (#115).
    if fsinfo::device_id(dest).is_ok_and(|d| d != device) {
        return Some(std::io::Error::other("the destination's drive changed").into());
    }
    // Every directory, then the drive's cache, whatever happened before: the first device
    // error is kept. Folders that were never created don't exist, which is no error; the
    // destination itself gone (pulled out) is (#115).
    let problems: Vec<Option<IoFailure>> = dirs
        .iter()
        .map(|d| (dest.join(&d.rel), false))
        .chain([(dest.to_path_buf(), true)])
        .map(|(dir, root)| match fs::File::open(&dir) {
            Ok(f) => durability_problem(os::sync_file(&f)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !root => None,
            Err(e) if root => Some(e.into()),
            Err(e) => durability_problem(Err(e)),
        })
        .collect();
    let barrier = durability_problem(os::full_barrier(dest));
    problems.into_iter().flatten().next().or(barrier)
}

/// A device error (not a file system that can't sync a directory or flush its cache).
fn durability_problem(result: std::io::Result<()>) -> Option<IoFailure> {
    let e = result.err()?;
    os::is_device_error(&e).then(|| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #58: a device error while making the copy durable is a problem; a file system that
    /// can't sync a directory is not.
    #[test]
    fn only_device_errors_make_durability_fail() {
        let err = |code| std::io::Error::from_raw_os_error(code);
        assert!(durability_problem(Err(err(libc::EIO))).is_some());
        assert!(durability_problem(Err(err(libc::ENXIO))).is_some());
        assert!(durability_problem(Err(err(libc::ENOTSUP))).is_none());
        assert!(durability_problem(Err(err(libc::EINVAL))).is_none());
        assert!(durability_problem(Ok(())).is_none());
    }

    /// QA review (#115): a destination that's gone (pulled out) can't confirm anything; a
    /// folder that was never created is no error.
    #[test]
    fn a_destination_gone_before_the_flush_is_a_durability_error() {
        let dir = tempfile::tempdir().unwrap();
        let never = [DirEntry {
            rel: "never".into(),
            mtime: None,
        }];
        let device = crate::fsinfo::device_id(dir.path()).unwrap();
        assert!(make_durable(dir.path(), &never, device).is_none());
        assert!(make_durable(&dir.path().join("gone"), &[], device).is_some());
        // Another drive mounted at the same path: nothing on it is this job's.
        assert!(make_durable(dir.path(), &[], device + 1).is_some());
    }
}
