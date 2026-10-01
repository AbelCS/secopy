//! Live per-file state and the progress snapshots sent to the UI (RFD §5.3, §7).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering::Relaxed};

use super::Event;
use super::runner::Runner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    Copying = 0,
    Verifying = 1,
}

/// A file currently being copied or verified (RFD §5.3, "Active files").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveFile {
    /// Index in `Plan::files`; stable for the whole job.
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
    /// Bytes the job writes; skipped files are not included.
    pub total_bytes: u64,
    /// Finished files, including skipped and failed ones.
    pub files_done: u64,
    /// Files skipped: already at the destination, or differing with Skip chosen (FR-17).
    pub files_skipped: u64,
    pub copied_bytes: u64,
    pub verified_bytes: u64,
    pub active: Vec<ActiveFile>,
    /// The job is paused (FR-22); snapshots keep coming so the UI stays live.
    pub paused: bool,
    /// Reading files already in the destination for ASC MHL, after the copy (#154).
    pub recording: Option<Recording>,
}

/// Bytes read of the files ASC MHL records that this job didn't copy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Recording {
    pub bytes: u64,
    pub total: u64,
}

/// Live state of one in-flight file.
pub(super) struct Slot {
    pub(super) id: usize,
    pub(super) rel: PathBuf,
    pub(super) size: u64,
    pub(super) phase: AtomicU8,
    pub(super) bytes: AtomicU64,
    /// True once this file's size is included in `Runner::copied_done`.
    pub(super) copy_counted: AtomicBool,
}

impl Slot {
    pub(super) fn set_phase(&self, phase: Phase) {
        self.bytes.store(0, Relaxed);
        self.phase.store(phase as u8, Relaxed);
    }

    pub(super) fn phase(&self) -> Phase {
        if self.phase.load(Relaxed) == Phase::Verifying as u8 {
            Phase::Verifying
        } else {
            Phase::Copying
        }
    }
}

impl Runner<'_> {
    pub(super) fn emit_progress(&self) {
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
        let total_bytes = self.bytes_to_write;
        Progress {
            total_files: self.plan.files.len() as u64,
            total_bytes,
            files_done: self.files_done.load(Relaxed),
            files_skipped: self.files_skipped.load(Relaxed),
            copied_bytes: copied.min(total_bytes),
            verified_bytes: verified.min(total_bytes),
            active: files,
            paused: self.control.is_paused(),
            recording: *self.recording.lock().expect("recording lock poisoned"),
        }
    }
}
