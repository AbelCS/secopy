//! Cancelling a running job from another thread (FR-23).

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use crate::error::FileError;

/// Lets another thread cancel a running job.
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

    /// Called at every buffer boundary: fails with `Cancelled` once the job is stopped.
    pub fn checkpoint(&self) -> Result<(), FileError> {
        if self.stop.load(Relaxed) {
            Err(FileError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_passes_when_running_and_fails_when_stopped() {
        let control = JobControl::new();
        assert_eq!(control.checkpoint(), Ok(()));
        control.cancel();
        assert_eq!(control.checkpoint(), Err(FileError::Cancelled));
    }
}
