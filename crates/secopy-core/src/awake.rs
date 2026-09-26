//! Keeps the system from sleeping while a job runs (RFD §5.3). Best effort: without the
//! OS mechanism the job still runs. Only idle sleep is blocked; the display may sleep.

use std::marker::PhantomData;
#[cfg(unix)]
use std::process::{Child, Command, Stdio};

/// Holds the "stay awake" request until dropped. Not `Send`: on Windows the request
/// belongs to the thread that made it, so it must end on that thread too.
pub struct KeepAwake {
    #[cfg(unix)]
    child: Option<Child>,
    #[cfg(windows)]
    active: bool,
    _not_send: PhantomData<*const ()>,
}

impl KeepAwake {
    /// macOS: `caffeinate -i`. Linux: a logind idle inhibitor via `systemd-inhibit`.
    /// Both helpers watch this process, so they also end if it crashes.
    #[cfg(unix)]
    pub fn new() -> Self {
        let pid = std::process::id().to_string();
        let mut cmd = if cfg!(target_os = "macos") {
            let mut cmd = Command::new("caffeinate");
            cmd.args(["-i", "-w", &pid]);
            cmd
        } else {
            let mut cmd = Command::new("systemd-inhibit");
            cmd.args([
                "--what=idle",
                "--who=Secopy",
                "--why=Copying files",
                "--mode=block",
                "tail",
                &format!("--pid={pid}"),
                "-f",
                "/dev/null",
            ]);
            cmd
        };
        let child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        Self {
            child,
            _not_send: PhantomData,
        }
    }

    #[cfg(windows)]
    pub fn new() -> Self {
        use windows_sys::Win32::System::Power::{
            ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
        };
        // SAFETY: plain flags; returns 0 on failure.
        let active = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) } != 0;
        Self {
            active,
            _not_send: PhantomData,
        }
    }

    /// Whether the request was accepted.
    pub fn is_active(&self) -> bool {
        #[cfg(unix)]
        return self.child.is_some();
        #[cfg(windows)]
        return self.active;
    }
}

impl Default for KeepAwake {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        #[cfg(windows)]
        if self.active {
            use windows_sys::Win32::System::Power::{ES_CONTINUOUS, SetThreadExecutionState};
            // SAFETY: clears this thread's request.
            unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
        }
    }
}

#[cfg(all(test, any(target_os = "macos", windows)))]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_accepted() {
        assert!(KeepAwake::new().is_active());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn caffeinate_ends_when_released() {
        let awake = KeepAwake::new();
        let pid = awake.child.as_ref().unwrap().id().to_string();
        drop(awake);
        let alive = Command::new("kill")
            .args(["-0", &pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(!alive);
    }
}
