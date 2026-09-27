//! Copy and verify lanes (RFD §7.2, §7.3).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use super::progress::{Phase, Slot};
use super::{Event, FileOutcome, FileStatus, JobControl, JobOptions, SkipReason};
use crate::copy::{self, Commit, CopyConfig, PartialCopy};
use crate::error::{FatalError, FileError};
use crate::fsinfo;
use crate::hash;
use crate::plan::{Action, Plan, PlannedFile};
use crate::verify::{self, CacheBypass};

/// Verify tasks that may wait per verify lane before copy lanes block (RFD §7.3).
pub(super) const VERIFY_QUEUE_PER_LANE: usize = 4;

/// Sets the flag when dropped, including during a panic.
pub(super) struct SetOnDrop<'a>(pub(super) &'a AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Relaxed);
    }
}

/// Lock-free work list shared by the lanes of one kind.
pub(super) struct Queue {
    items: Vec<usize>,
    next: AtomicUsize,
}

impl Queue {
    pub(super) fn new(items: Vec<usize>) -> Self {
        Self {
            items,
            next: AtomicUsize::new(0),
        }
    }

    fn pop(&self) -> Option<usize> {
        self.items.get(self.next.fetch_add(1, Relaxed)).copied()
    }
}

pub(super) struct VerifyTask {
    slot: Arc<Slot>,
    idx: usize,
    partial: PartialCopy,
    started: Instant,
}

pub(super) struct Runner<'a> {
    pub(super) plan: &'a Plan,
    pub(super) opts: &'a JobOptions,
    pub(super) copy_cfg: CopyConfig,
    pub(super) control: &'a JobControl,
    pub(super) on_event: &'a (dyn Fn(Event) + Sync),
    pub(super) bytes_to_write: u64,
    pub(super) copied_done: AtomicU64,
    pub(super) verified_done: AtomicU64,
    pub(super) files_done: AtomicU64,
    pub(super) files_skipped: AtomicU64,
    pub(super) active: Mutex<Vec<Arc<Slot>>>,
    pub(super) outcomes: Mutex<Vec<FileOutcome>>,
    pub(super) fatal: Mutex<Option<FatalError>>,
    pub(super) bypass_unavailable: AtomicBool,
    /// Partial files left by interrupted jobs that were removed (FR-18).
    pub(super) removed_partials: AtomicU64,
    /// Set when the source is gone: no new copies start, but files already copied are
    /// still verified, since that only needs the destination (FR-21).
    stop_copying: AtomicBool,
    /// Folders already created, so each one costs one system call (FR-10).
    created_dirs: Mutex<HashSet<PathBuf>>,
}

impl<'a> Runner<'a> {
    pub(super) fn new(
        plan: &'a Plan,
        opts: &'a JobOptions,
        control: &'a JobControl,
        on_event: &'a (dyn Fn(Event) + Sync),
    ) -> Self {
        let copy_cfg = CopyConfig {
            uncached_write: opts.verify,
            ..opts.copy.clone()
        };
        Self {
            plan,
            opts,
            copy_cfg,
            control,
            on_event,
            bytes_to_write: plan.bytes_to_write(),
            copied_done: AtomicU64::new(0),
            verified_done: AtomicU64::new(0),
            files_done: AtomicU64::new(0),
            files_skipped: AtomicU64::new(0),
            active: Mutex::new(Vec::new()),
            outcomes: Mutex::new(Vec::new()),
            fatal: Mutex::new(None),
            bypass_unavailable: AtomicBool::new(false),
            removed_partials: AtomicU64::new(0),
            stop_copying: AtomicBool::new(false),
            created_dirs: Mutex::new(HashSet::new()),
        }
    }

    /// Records files the plan doesn't write: skipped ones and pre-flight failures.
    pub(super) fn finish_unwritten(&self, idx: usize) {
        let file = &self.plan.files[idx];
        let status = match &file.action {
            Action::SkipIdentical => FileStatus::Skipped(SkipReason::Identical),
            Action::SkipDiffers => FileStatus::Skipped(SkipReason::Differs),
            Action::Fail(e) => FileStatus::Failed(e.clone()),
            _ => unreachable!("only unwritten files"),
        };
        if matches!(status, FileStatus::Skipped(_)) {
            self.files_skipped.fetch_add(1, Relaxed);
        }
        self.finish(None, self.outcome(idx, None, status, None, Instant::now()));
    }

    pub(super) fn copy_lane(&self, queue: &Queue, verify_tx: &mpsc::SyncSender<VerifyTask>) {
        // Blocks while paused, so no new file starts during a pause (FR-22).
        while !self.stop_copying.load(Relaxed) && self.control.checkpoint().is_ok() {
            let Some(idx) = queue.pop() else { break };
            self.copy_one(idx, verify_tx);
        }
    }

    fn copy_one(&self, idx: usize, verify_tx: &mpsc::SyncSender<VerifyTask>) {
        let file = &self.plan.files[idx];
        let final_path = self.plan.dest.join(file.final_rel());
        let started = Instant::now();
        let slot = self.begin(idx, file);
        // A file that appeared since pre-flight is never replaced.
        if file.action == Action::Copy && fs::symlink_metadata(&final_path).is_ok() {
            let status = FileStatus::Failed(FileError::AlreadyExists);
            return self.finish(Some(&slot), self.outcome(idx, None, status, None, started));
        }
        let partial = match self.copy_attempt(file, &final_path, &slot, 0) {
            Ok(partial) => partial,
            Err(e) => {
                let status = FileStatus::Failed(e);
                return self.finish(Some(&slot), self.outcome(idx, None, status, None, started));
            }
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
        let outcome = match self.commit(file, partial) {
            Ok(landed) => self.outcome(idx, Some(hash), FileStatus::Copied, Some(landed), started),
            Err(e) => self.outcome(idx, Some(hash), FileStatus::Failed(e), None, started),
        };
        self.finish(Some(&slot), outcome);
    }

    fn copy_attempt(
        &self,
        file: &PlannedFile,
        final_path: &Path,
        slot: &Slot,
        attempt: u32,
    ) -> Result<PartialCopy, FileError> {
        slot.set_phase(Phase::Copying);
        self.create_parent(final_path)?;
        if let Some(hook) = self.opts.hooks.before_copy {
            hook(&file.entry.source);
        }
        let partial = copy::copy_to_partial(
            &file.entry.source,
            final_path,
            &self.copy_cfg,
            &|b| slot.bytes.store(b, Relaxed),
            self.control,
        )?;
        if partial.removed_stale {
            self.removed_partials.fetch_add(1, Relaxed);
        }
        // Growing or shrinking while it was read: the copy matches no version of the file.
        if partial.bytes != file.entry.size {
            partial.discard();
            return Err(FileError::SourceChanged);
        }
        if let Some(hook) = self.opts.hooks.after_copy {
            hook(&partial.partial, attempt);
        }
        Ok(partial)
    }

    /// Folders are created when their first file starts, so a cancelled or failed job
    /// leaves no empty folders behind (FR-10).
    fn create_parent(&self, final_path: &Path) -> Result<(), FileError> {
        let Some(parent) = final_path.parent() else {
            return Ok(());
        };
        if self
            .created_dirs
            .lock()
            .expect("dirs lock poisoned")
            .contains(parent)
        {
            return Ok(());
        }
        fs::create_dir_all(parent).map_err(FileError::write_dest)?;
        self.created_dirs
            .lock()
            .expect("dirs lock poisoned")
            .insert(parent.to_path_buf());
        Ok(())
    }

    /// Gives the copy its final name as the plan says; returns where it landed,
    /// relative to the destination.
    fn commit(&self, file: &PlannedFile, partial: PartialCopy) -> Result<PathBuf, FileError> {
        let dest = &self.plan.dest;
        let original = dest.join(&file.entry.rel);
        let how = match &file.action {
            Action::Overwrite => Commit::Replace,
            Action::KeepBoth { n, .. } => Commit::KeepBoth {
                original: &original,
                n: *n,
            },
            _ => Commit::NoReplace,
        };
        let landed = partial.commit(&dest.join(file.final_rel()), how)?;
        Ok(landed
            .strip_prefix(dest)
            .map(Path::to_path_buf)
            .unwrap_or(landed))
    }

    pub(super) fn verify_lane(&self, rx: &Mutex<mpsc::Receiver<VerifyTask>>) {
        loop {
            let task = match rx.lock().expect("verify queue lock poisoned").recv() {
                Ok(task) => task,
                Err(_) => break,
            };
            self.verify_one(task);
        }
    }

    /// Verifies the partial file, re-copying once on mismatch (FR-27), then commits it.
    fn verify_one(&self, task: VerifyTask) {
        let VerifyTask {
            slot,
            idx,
            mut partial,
            started,
        } = task;
        let file = &self.plan.files[idx];
        let final_path = self.plan.dest.join(file.final_rel());
        let mut attempt = 0;
        let result = loop {
            slot.set_phase(Phase::Verifying);
            let (actual, bypass) = match verify::hash_from_device(
                &partial.partial,
                self.copy_cfg.buffer_size,
                &|b| slot.bytes.store(b, Relaxed),
                self.control,
            ) {
                Ok(v) => v,
                Err(e) => {
                    partial.discard();
                    break Err(e);
                }
            };
            if bypass == CacheBypass::Unavailable {
                self.bypass_unavailable.store(true, Relaxed);
            }
            let expected = partial.hash;
            if actual == expected {
                break self.commit(file, partial).map(|landed| (expected, landed));
            }
            partial.discard();
            if attempt == 1 {
                break Err(FileError::HashMismatch {
                    expected: hash::to_hex(expected),
                    actual: hash::to_hex(actual),
                });
            }
            attempt += 1;
            partial = match self.copy_attempt(file, &final_path, &slot, attempt) {
                Ok(p) => p,
                Err(e) => break Err(e),
            };
        };
        let outcome = match result {
            Ok((hash, landed)) => {
                self.outcome(idx, Some(hash), FileStatus::Verified, Some(landed), started)
            }
            Err(e) => self.outcome(idx, None, FileStatus::Failed(e), None, started),
        };
        self.finish(Some(&slot), outcome);
    }

    fn begin(&self, idx: usize, file: &PlannedFile) -> Arc<Slot> {
        let slot = Arc::new(Slot {
            id: idx,
            rel: file.final_rel().to_path_buf(),
            size: file.entry.size,
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

    fn outcome(
        &self,
        idx: usize,
        hash: Option<u64>,
        status: FileStatus,
        landed: Option<PathBuf>,
        started: Instant,
    ) -> FileOutcome {
        let file = &self.plan.files[idx];
        let ok = matches!(status, FileStatus::Copied | FileStatus::Verified);
        FileOutcome {
            id: idx,
            rel: file.entry.rel.clone(),
            final_rel: landed.unwrap_or_else(|| file.final_rel().to_path_buf()),
            size: file.entry.size,
            hash,
            in_checksum_file: ok
                && hash.is_some()
                && file.in_checksum_file
                && self.opts.write_checksum_file,
            status,
            elapsed: started.elapsed(),
        }
    }

    /// After an I/O error, checks whether the whole source or destination went away
    /// (FR-21): the folder is gone, or its volume changed (unplugged or remounted).
    fn fatal_cause(&self, idx: usize, e: &FileError) -> Option<FatalError> {
        let plan = self.plan;
        if e.is_disk_full() {
            return Some(FatalError::DiskFull);
        }
        match e {
            FileError::WriteDest(_) | FileError::ReadBack(_)
                if !root_is_there(&plan.dest, plan.fs.device) =>
            {
                Some(FatalError::DestinationGone)
            }
            FileError::ReadSource(_) => {
                let source = &plan.files[idx].entry.source;
                plan.source_roots
                    .iter()
                    .any(|r| source.starts_with(&r.path) && !root_is_there(&r.path, r.device))
                    .then_some(FatalError::SourceGone)
            }
            _ => None,
        }
    }

    fn finish(&self, slot: Option<&Arc<Slot>>, outcome: FileOutcome) {
        if let Some(slot) = slot {
            self.active
                .lock()
                .expect("active lock poisoned")
                .retain(|s| !Arc::ptr_eq(s, slot));
            // A finished file counts as fully processed, so the bars reach 100 % even
            // with failures.
            self.count_copied(slot);
            if self.opts.verify {
                self.verified_done.fetch_add(slot.size, Relaxed);
            }
        }
        self.files_done.fetch_add(1, Relaxed);
        if let FileStatus::Failed(e) = &outcome.status
            && let Some(fatal) = self.fatal_cause(outcome.id, e)
        {
            self.fatal
                .lock()
                .expect("fatal lock poisoned")
                .get_or_insert(fatal.clone());
            if fatal == FatalError::SourceGone {
                self.stop_copying.store(true, Relaxed);
            } else {
                self.control.cancel();
            }
        }
        self.outcomes
            .lock()
            .expect("outcomes lock poisoned")
            .push(outcome.clone());
        (self.on_event)(Event::FileFinished(outcome));
    }
}

/// The folder still exists, as a folder, on the same volume.
fn root_is_there(path: &Path, device: u64) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_dir())
        && fsinfo::device_id(path).is_ok_and(|d| d == device)
}
