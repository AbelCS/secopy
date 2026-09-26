//! Runs a copy job: copy lanes, verify lanes, progress events and the checksum file (RFD §7).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use chrono::Local;

use crate::checksum_file;
use crate::copy::{self, CopyConfig, PartialCopy};
use crate::error::FileError;
use crate::scan::{ScanEntry, Selection};
use crate::verify::{self, CacheBypass};
use crate::{hash, os};

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

/// Lets another thread cancel a running job (FR-23).
#[derive(Debug, Default)]
pub struct JobControl {
    stop: AtomicBool,
}

impl JobControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops the job: no new files start, in-flight partial files are removed.
    pub fn cancel(&self) {
        self.stop.store(true, Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.load(Relaxed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    Copying = 0,
    Verifying = 1,
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

/// A file currently being copied or verified (RFD §5.3, "Active files").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveFile {
    /// Index in `Selection::files`; stable for the whole job.
    pub id: usize,
    pub rel: PathBuf,
    pub size: u64,
    pub phase: Phase,
    /// Bytes done in the current phase.
    pub bytes_done: u64,
}

/// Raw counters; the UI derives speed and ETA from successive snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub total_files: u64,
    pub total_bytes: u64,
    pub files_done: u64,
    pub copied_bytes: u64,
    pub verified_bytes: u64,
    pub active: Vec<ActiveFile>,
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
        let _ = fs::create_dir_all(dest.join(dir));
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
fn make_durable(dest: &Path, dirs: &[PathBuf]) {
    #[cfg(unix)]
    for dir in dirs
        .iter()
        .map(|d| dest.join(d))
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

/// Verify tasks that may wait per verify lane before copy lanes block (RFD §7.3).
const VERIFY_QUEUE_PER_LANE: usize = 4;

/// Sets the flag when dropped, including during a panic.
struct SetOnDrop<'a>(&'a AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Relaxed);
    }
}

/// Lock-free work list shared by the lanes of one kind.
struct Queue {
    items: Vec<usize>,
    next: AtomicUsize,
}

impl Queue {
    fn new(items: Vec<usize>) -> Self {
        Self {
            items,
            next: AtomicUsize::new(0),
        }
    }

    fn pop(&self) -> Option<usize> {
        self.items.get(self.next.fetch_add(1, Relaxed)).copied()
    }
}

/// Live state of one in-flight file.
struct Slot {
    id: usize,
    rel: PathBuf,
    size: u64,
    phase: AtomicU8,
    bytes: AtomicU64,
    /// True once this file's size is included in `Runner::copied_done`.
    copy_counted: AtomicBool,
}

impl Slot {
    fn set_phase(&self, phase: Phase) {
        self.bytes.store(0, Relaxed);
        self.phase.store(phase as u8, Relaxed);
    }

    fn phase(&self) -> Phase {
        if self.phase.load(Relaxed) == Phase::Verifying as u8 {
            Phase::Verifying
        } else {
            Phase::Copying
        }
    }
}

struct VerifyTask {
    slot: Arc<Slot>,
    idx: usize,
    partial: PartialCopy,
    started: Instant,
}

struct Runner<'a> {
    sel: &'a Selection,
    dest: &'a Path,
    opts: &'a JobOptions,
    copy_cfg: CopyConfig,
    control: &'a JobControl,
    on_event: &'a (dyn Fn(Event) + Sync),
    /// Indexed like `Selection::files`; true = skip with `FileError::NameClash`.
    clashes: Vec<bool>,
    copied_done: AtomicU64,
    verified_done: AtomicU64,
    files_done: AtomicU64,
    active: Mutex<Vec<Arc<Slot>>>,
    outcomes: Mutex<Vec<FileOutcome>>,
    fatal: Mutex<Option<FileError>>,
    bypass_unavailable: AtomicBool,
}

impl<'a> Runner<'a> {
    fn new(
        sel: &'a Selection,
        dest: &'a Path,
        opts: &'a JobOptions,
        control: &'a JobControl,
        on_event: &'a (dyn Fn(Event) + Sync),
        clashes: Vec<bool>,
    ) -> Self {
        let copy_cfg = CopyConfig {
            uncached_write: opts.verify,
            ..opts.copy.clone()
        };
        Self {
            sel,
            dest,
            opts,
            copy_cfg,
            control,
            on_event,
            clashes,
            copied_done: AtomicU64::new(0),
            verified_done: AtomicU64::new(0),
            files_done: AtomicU64::new(0),
            active: Mutex::new(Vec::new()),
            outcomes: Mutex::new(Vec::new()),
            fatal: Mutex::new(None),
            bypass_unavailable: AtomicBool::new(false),
        }
    }

    fn copy_lane(&self, queue: &Queue, verify_tx: &mpsc::SyncSender<VerifyTask>) {
        while !self.control.is_stopped() {
            let Some(idx) = queue.pop() else { break };
            self.copy_one(idx, verify_tx);
        }
    }

    fn copy_one(&self, idx: usize, verify_tx: &mpsc::SyncSender<VerifyTask>) {
        let entry = &self.sel.files[idx];
        let final_path = self.dest.join(&entry.rel);
        let started = Instant::now();
        let slot = self.begin(idx, entry);
        let blocked = if self.clashes[idx] {
            Some(FileError::NameClash)
        } else if fs::symlink_metadata(&final_path).is_ok() {
            Some(FileError::AlreadyExists)
        } else {
            None
        };
        if let Some(e) = blocked {
            return self.finish(&slot, entry, None, FileStatus::Failed(e), started);
        }
        let partial = match self.copy_attempt(entry, &final_path, &slot, 0) {
            Ok(partial) => partial,
            Err(e) => return self.finish(&slot, entry, None, FileStatus::Failed(e), started),
        };
        self.count_copied(&slot);
        if self.opts.verify {
            slot.set_phase(Phase::Verifying);
            let task = VerifyTask {
                slot,
                idx,
                partial,
                started,
            };
            // Blocks while the verify queue is full. Fails only if every verify lane died,
            // and then the panic propagates out of `run_job`.
            verify_tx.send(task).expect("verify lanes are running");
            return;
        }
        let hash = partial.hash;
        match copy::commit(&partial.partial, &final_path) {
            Ok(()) => self.finish(&slot, entry, Some(hash), FileStatus::Copied, started),
            Err(e) => {
                let _ = fs::remove_file(&partial.partial);
                self.finish(&slot, entry, Some(hash), FileStatus::Failed(e), started)
            }
        }
    }

    fn copy_attempt(
        &self,
        entry: &ScanEntry,
        final_path: &Path,
        slot: &Slot,
        attempt: u32,
    ) -> Result<PartialCopy, FileError> {
        slot.set_phase(Phase::Copying);
        let partial = copy::copy_to_partial(
            &entry.source,
            final_path,
            &self.copy_cfg,
            &|b| slot.bytes.store(b, Relaxed),
            &self.control.stop,
        )?;
        if let Some(hook) = self.opts.hooks.after_copy {
            hook(&partial.partial, attempt);
        }
        Ok(partial)
    }

    fn verify_lane(&self, rx: &Mutex<mpsc::Receiver<VerifyTask>>) {
        loop {
            let task = match rx.lock().expect("verify queue lock poisoned").recv() {
                Ok(task) => task,
                Err(_) => break,
            };
            self.verify_one(task);
        }
    }

    /// Verifies the partial file, re-copying once on mismatch (FR-27), then commits it.
    /// Removes only partial files this lane created: a failed re-copy may have hit a
    /// partial file that belongs to another writer.
    fn verify_one(&self, task: VerifyTask) {
        let VerifyTask {
            slot,
            idx,
            mut partial,
            started,
        } = task;
        let entry = &self.sel.files[idx];
        let final_path = self.dest.join(&entry.rel);
        let mut attempt = 0;
        let result = loop {
            slot.set_phase(Phase::Verifying);
            let (actual, bypass) = match verify::hash_from_device(
                &partial.partial,
                self.copy_cfg.buffer_size,
                &|b| slot.bytes.store(b, Relaxed),
                &self.control.stop,
            ) {
                Ok(v) => v,
                Err(e) => {
                    let _ = fs::remove_file(&partial.partial);
                    break Err(e);
                }
            };
            if bypass == CacheBypass::Unavailable {
                self.bypass_unavailable.store(true, Relaxed);
            }
            if actual == partial.hash {
                let committed = copy::commit(&partial.partial, &final_path);
                if committed.is_err() {
                    let _ = fs::remove_file(&partial.partial);
                }
                break committed.map(|()| partial.hash);
            }
            let _ = fs::remove_file(&partial.partial);
            if attempt == 1 {
                break Err(FileError::HashMismatch {
                    expected: hash::to_hex(partial.hash),
                    actual: hash::to_hex(actual),
                });
            }
            attempt += 1;
            partial = match self.copy_attempt(entry, &final_path, &slot, attempt) {
                Ok(p) => p,
                Err(e) => break Err(e),
            };
        };
        match result {
            Ok(hash) => self.finish(&slot, entry, Some(hash), FileStatus::Verified, started),
            Err(e) => self.finish(&slot, entry, None, FileStatus::Failed(e), started),
        }
    }

    fn begin(&self, idx: usize, entry: &ScanEntry) -> Arc<Slot> {
        let slot = Arc::new(Slot {
            id: idx,
            rel: entry.rel.clone(),
            size: entry.size,
            phase: AtomicU8::new(Phase::Copying as u8),
            bytes: AtomicU64::new(0),
            copy_counted: AtomicBool::new(false),
        });
        self.active
            .lock()
            .expect("active lock poisoned")
            .push(slot.clone());
        slot
    }

    fn count_copied(&self, slot: &Slot) {
        if !slot.copy_counted.swap(true, Relaxed) {
            self.copied_done.fetch_add(slot.size, Relaxed);
        }
    }

    fn finish(
        &self,
        slot: &Arc<Slot>,
        entry: &ScanEntry,
        hash: Option<u64>,
        status: FileStatus,
        started: Instant,
    ) {
        self.active
            .lock()
            .expect("active lock poisoned")
            .retain(|s| !Arc::ptr_eq(s, slot));
        // A finished file counts as fully processed, so the bars reach 100 % even with failures.
        self.count_copied(slot);
        if self.opts.verify {
            self.verified_done.fetch_add(slot.size, Relaxed);
        }
        self.files_done.fetch_add(1, Relaxed);
        if let FileStatus::Failed(e) = &status
            && e.is_fatal()
        {
            self.fatal
                .lock()
                .expect("fatal lock poisoned")
                .get_or_insert(e.clone());
            self.control.cancel();
        }
        let outcome = FileOutcome {
            rel: entry.rel.clone(),
            size: entry.size,
            hash,
            status,
            elapsed: started.elapsed(),
        };
        self.outcomes
            .lock()
            .expect("outcomes lock poisoned")
            .push(outcome.clone());
        (self.on_event)(Event::FileFinished(outcome));
    }

    fn emit_progress(&self) {
        (self.on_event)(Event::Progress(self.progress()));
    }

    fn progress(&self) -> Progress {
        let active = self.active.lock().expect("active lock poisoned");
        let mut copied = self.copied_done.load(Relaxed);
        let mut verified = self.verified_done.load(Relaxed);
        let files = active
            .iter()
            .map(|s| {
                let phase = s.phase();
                let bytes_done = s.bytes.load(Relaxed);
                match phase {
                    Phase::Copying if !s.copy_counted.load(Relaxed) => copied += bytes_done,
                    Phase::Copying => {}
                    Phase::Verifying => verified += bytes_done,
                }
                ActiveFile {
                    id: s.id,
                    rel: s.rel.clone(),
                    size: s.size,
                    phase,
                    bytes_done,
                }
            })
            .collect();
        let total_bytes = self.sel.total_bytes;
        Progress {
            total_files: self.sel.files.len() as u64,
            total_bytes,
            files_done: self.files_done.load(Relaxed),
            copied_bytes: copied.min(total_bytes),
            verified_bytes: verified.min(total_bytes),
            active: files,
        }
    }
}
