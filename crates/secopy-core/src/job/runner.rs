//! Copy and verify lanes (RFD §7.2, §7.3).

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use super::progress::{Phase, Slot};
use super::{Event, FileOutcome, FileStatus, JobControl, JobOptions};
use crate::copy::{self, Commit, CopyConfig, PartialCopy};
use crate::error::FileError;
use crate::hash;
use crate::scan::{ScanEntry, Selection};
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
    pub(super) sel: &'a Selection,
    pub(super) dest: &'a Path,
    pub(super) opts: &'a JobOptions,
    pub(super) copy_cfg: CopyConfig,
    pub(super) control: &'a JobControl,
    pub(super) on_event: &'a (dyn Fn(Event) + Sync),
    /// Indexed like `Selection::files`; true = skip with `FileError::NameClash`.
    pub(super) clashes: Vec<bool>,
    pub(super) copied_done: AtomicU64,
    pub(super) verified_done: AtomicU64,
    pub(super) files_done: AtomicU64,
    pub(super) active: Mutex<Vec<Arc<Slot>>>,
    pub(super) outcomes: Mutex<Vec<FileOutcome>>,
    pub(super) fatal: Mutex<Option<FileError>>,
    pub(super) bypass_unavailable: AtomicBool,
    /// Partial files left by interrupted jobs that were removed (FR-18).
    pub(super) removed_partials: AtomicU64,
}

impl<'a> Runner<'a> {
    pub(super) fn new(
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
            removed_partials: AtomicU64::new(0),
        }
    }

    pub(super) fn copy_lane(&self, queue: &Queue, verify_tx: &mpsc::SyncSender<VerifyTask>) {
        // Blocks while paused, so no new file starts during a pause (FR-22).
        while self.control.checkpoint().is_ok() {
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
        match partial.commit(&final_path, Commit::NoReplace) {
            Ok(_) => self.finish(&slot, entry, Some(hash), FileStatus::Copied, started),
            Err(e) => self.finish(&slot, entry, Some(hash), FileStatus::Failed(e), started),
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
            self.control,
        )?;
        if partial.removed_stale {
            self.removed_partials.fetch_add(1, Relaxed);
        }
        if let Some(hook) = self.opts.hooks.after_copy {
            hook(&partial.partial, attempt);
        }
        Ok(partial)
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
        let entry = &self.sel.files[idx];
        let final_path = self.dest.join(&entry.rel);
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
                break partial
                    .commit(&final_path, Commit::NoReplace)
                    .map(|_| expected);
            }
            partial.discard();
            if attempt == 1 {
                break Err(FileError::HashMismatch {
                    expected: hash::to_hex(expected),
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
}
