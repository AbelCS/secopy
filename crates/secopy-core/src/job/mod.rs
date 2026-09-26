//! Runs a copy job: copy lanes, verify lanes, progress events and the checksum file (RFD §7).

mod progress;
mod runner;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use chrono::Local;

use crate::checksum_file;
use crate::copy::CopyConfig;
use crate::error::FileError;
use crate::os;
use crate::scan::{DirEntry, Selection};
use crate::verify::CacheBypass;

pub use crate::control::JobControl;
pub use progress::{ActiveFile, Phase, Progress};
use runner::{Queue, Runner, SetOnDrop, VERIFY_QUEUE_PER_LANE, VerifyTask};

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
    /// How often `Event::Progress` is emitted.
    pub progress_interval: Duration,
    #[doc(hidden)]
    pub hooks: Hooks,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            verify: true,
            write_checksum_file: true,
            copy: CopyConfig::default(),
            small_file_threshold: 8 << 20,
            small_file_lanes: 8,
            large_file_lanes: 1,
            verify_lanes: 2,
            progress_interval: Duration::from_millis(50),
            hooks: Hooks::default(),
        }
    }
}

/// Fault injection for tests. Not part of the stable API.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Hooks {
    /// Called with the partial file after each copy attempt (0 = first), before verifying.
    pub after_copy: Option<fn(&Path, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Copied,
    Verified,
    Failed(FileError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOutcome {
    pub rel: PathBuf,
    pub size: u64,
    /// Source hash; `None` if the file failed before it was hashed.
    pub hash: Option<u64>,
    pub status: FileStatus,
    pub elapsed: Duration,
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
    pub checksum_error: Option<String>,
    /// `None` when not verifying.
    pub cache_bypass: Option<CacheBypass>,
    pub fatal: Option<FileError>,
    pub cancelled: bool,
    pub elapsed: Duration,
}

impl JobReport {
    pub fn failed(&self) -> impl Iterator<Item = &FileOutcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, FileStatus::Failed(_)))
    }

    pub fn is_success(&self) -> bool {
        self.fatal.is_none()
            && !self.cancelled
            && self.not_started == 0
            && self.failed().next().is_none()
    }
}

/// Copies every file in `sel` into `dest`. Blocks until done; call from a worker thread.
/// `on_event` is called from several threads.
pub fn run_job(
    sel: &Selection,
    dest: &Path,
    opts: &JobOptions,
    control: &JobControl,
    on_event: &(dyn Fn(Event) + Sync),
) -> JobReport {
    let started = Instant::now();
    for dir in &sel.dirs {
        // A failure here surfaces as a per-file write error.
        let _ = fs::create_dir_all(dest.join(&dir.rel));
    }
    let clashes = find_name_clashes(sel);
    let runner = Runner::new(sel, dest, opts, control, on_event, clashes);
    let (small, large): (Vec<usize>, Vec<usize>) =
        (0..sel.files.len()).partition(|&i| sel.files[i].size <= opts.small_file_threshold);
    let small = Queue::new(small);
    let large = Queue::new(large);
    let finished = AtomicBool::new(false);

    std::thread::scope(|s| {
        let ticker = s.spawn(|| {
            while !finished.load(Relaxed) {
                std::thread::sleep(opts.progress_interval);
                runner.emit_progress();
            }
        });
        // Stops the ticker even when joining a worker panics below; otherwise the scope
        // would wait for the ticker forever and the panic would become a hang.
        let stop_ticker = SetOnDrop(&finished);
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
    let outcomes = runner
        .outcomes
        .into_inner()
        .expect("outcomes lock poisoned");
    let fatal = runner.fatal.into_inner().expect("fatal lock poisoned");
    let (checksum_file, checksum_error) = if opts.write_checksum_file {
        write_checksum(dest, &outcomes)
    } else {
        (None, None)
    };
    make_durable(dest, &sel.dirs);
    JobReport {
        not_started: (sel.files.len() - outcomes.len()) as u64,
        outcomes,
        checksum_file,
        checksum_error,
        cache_bypass: opts.verify.then_some(if bypass_unavailable {
            CacheBypass::Unavailable
        } else {
            CacheBypass::Active
        }),
        cancelled: control.is_stopped() && fatal.is_none(),
        fatal,
        elapsed: started.elapsed(),
    }
}

/// Marks every file whose destination path repeats an earlier one, ignoring case:
/// two lanes writing the same name would corrupt each other, and case-insensitive
/// destinations (macOS, Windows, exFAT) treat `a.txt` and `A.TXT` as one file.
fn find_name_clashes(sel: &Selection) -> Vec<bool> {
    let mut seen = HashSet::new();
    sel.files
        .iter()
        .map(|f| !seen.insert(checksum_file::slash_path(&f.rel).to_lowercase()))
        .collect()
}

fn write_checksum(dest: &Path, outcomes: &[FileOutcome]) -> (Option<PathBuf>, Option<String>) {
    let entries: Vec<(PathBuf, u64)> = outcomes
        .iter()
        .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
        .filter_map(|o| o.hash.map(|h| (o.rel.clone(), h)))
        .collect();
    if entries.is_empty() {
        return (None, None);
    }
    match checksum_file::write(dest, &entries, Local::now()) {
        Ok(path) => (Some(path), None),
        Err(e) => (None, Some(e.to_string())),
    }
}

/// Makes the job durable: one fsync per directory for the renames, then one
/// drive-cache flush for the whole volume (RFD §7.4).
fn make_durable(dest: &Path, dirs: &[DirEntry]) {
    #[cfg(unix)]
    for dir in dirs
        .iter()
        .map(|d| dest.join(&d.rel))
        .chain([dest.to_path_buf()])
    {
        if let Ok(f) = fs::File::open(&dir) {
            let _ = os::sync_file(&f);
        }
    }
    #[cfg(not(unix))]
    let _ = dirs;
    let _ = os::full_barrier(dest);
}
