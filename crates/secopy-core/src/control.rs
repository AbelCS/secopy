//! Pausing and cancelling a running job from another thread (FR-22, FR-23).

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Condvar, Mutex};

use crate::error::FileError;

/// Lets another thread pause, resume or cancel a running job.
#[derive(Debug, Default)]
pub struct JobControl {
    stop: AtomicBool,
    paused: AtomicBool,
    /// Guards changes to the flags above, so a waiting lane never misses a wake-up.
    lock: Mutex<()>,
    wake: Condvar,
}

impl JobControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops the job: no new files start, in-flight partial files are removed.
    pub fn cancel(&self) {
        self.set(&self.stop, true);
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.load(Relaxed)
    }

    /// Stops all I/O at the next buffer boundary until [`resume`](Self::resume).
    pub fn pause(&self) {
        self.set(&self.paused, true);
    }

    pub fn resume(&self) {
        self.set(&self.paused, false);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Relaxed)
    }

    /// Called at every buffer boundary and before each file: blocks while the job is
    /// paused, and fails with `Cancelled` once it is stopped.
    pub fn checkpoint(&self) -> Result<(), FileError> {
        if self.paused.load(Relaxed) {
            let guard = self.lock.lock().expect("control lock poisoned");
            let _guard = self
                .wake
                .wait_while(guard, |_| {
                    self.paused.load(Relaxed) && !self.stop.load(Relaxed)
                })
                .expect("control lock poisoned");
        }
        if self.stop.load(Relaxed) {
            Err(FileError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn set(&self, flag: &AtomicBool, value: bool) {
        let _guard = self.lock.lock().expect("control lock poisoned");
        flag.store(value, Relaxed);
        self.wake.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn checkpoint_passes_when_running_and_fails_when_stopped() {
        let control = JobControl::new();
        assert_eq!(control.checkpoint(), Ok(()));
        control.cancel();
        assert_eq!(control.checkpoint(), Err(FileError::Cancelled));
    }

    #[test]
    fn checkpoint_blocks_while_paused_until_resumed() {
        let control = JobControl::new();
        control.pause();
        let (tx, rx) = mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| tx.send(control.checkpoint()).unwrap());
            assert!(
                rx.recv_timeout(Duration::from_millis(100)).is_err(),
                "must block"
            );
            control.resume();
            assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Ok(()));
        });
    }

    #[test]
    fn cancel_wakes_a_paused_checkpoint() {
        let control = JobControl::new();
        control.pause();
        let (tx, rx) = mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| tx.send(control.checkpoint()).unwrap());
            control.cancel();
            assert_eq!(
                rx.recv_timeout(Duration::from_secs(5)).unwrap(),
                Err(FileError::Cancelled)
            );
        });
    }
}
