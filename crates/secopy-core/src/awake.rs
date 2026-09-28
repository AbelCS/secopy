//! Keeps the system from sleeping while a job runs (RFD §5.3). Best effort: without the
//! OS mechanism the job still runs. Only idle sleep is blocked; the display may sleep.

use std::marker::PhantomData;

/// Why the Mac stays awake, as `pmset -g assertions` shows it.
const REASON: &str = "Secopy is copying files";

/// Holds the "stay awake" request until dropped. Not `Send`, so it ends on the thread
/// that made it.
pub struct KeepAwake {
    /// A power assertion inside this process: no helper process, which macOS 27 would
    /// report as the app running in the background.
    assertion: Option<u32>,
    _not_send: PhantomData<*const ()>,
}

impl KeepAwake {
    /// An IOKit assertion that prevents idle sleep (what `caffeinate -i` does); it ends
    /// with this process if it crashes.
    pub fn new() -> Self {
        Self {
            assertion: iokit::prevent_idle_sleep(REASON),
            _not_send: PhantomData,
        }
    }

    /// Whether the request was accepted.
    pub fn is_active(&self) -> bool {
        self.assertion.is_some()
    }
}

impl Default for KeepAwake {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        if let Some(id) = self.assertion {
            iokit::release(id);
        }
    }
}

/// IOKit's power assertions (`IOPMLib.h`).
mod iokit {
    use std::ffi::{CString, c_char, c_void};

    type CFStringRef = *const c_void;
    const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8
    const LEVEL_ON: u32 = 255; // kIOPMAssertionLevelOn

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            s: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPMAssertionCreateWithName(
            kind: CFStringRef,
            level: u32,
            name: CFStringRef,
            id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }

    /// A CoreFoundation string, released when dropped.
    struct CfString(CFStringRef);

    impl CfString {
        fn new(s: &str) -> Option<Self> {
            let c = CString::new(s).ok()?;
            // SAFETY: `c` is a valid C string; a null result is checked.
            let r = unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8) };
            (!r.is_null()).then_some(Self(r))
        }
    }

    impl Drop for CfString {
        fn drop(&mut self) {
            // SAFETY: created by CFStringCreateWithCString and released once.
            unsafe { CFRelease(self.0) };
        }
    }

    /// The assertion's id, or `None` if macOS refused it.
    pub fn prevent_idle_sleep(reason: &str) -> Option<u32> {
        let kind = CfString::new("PreventUserIdleSystemSleep")?;
        let name = CfString::new(reason)?;
        let mut id = 0;
        // SAFETY: valid strings and a valid out-parameter; 0 is kIOReturnSuccess.
        let result = unsafe { IOPMAssertionCreateWithName(kind.0, LEVEL_ON, name.0, &mut id) };
        (result == 0).then_some(id)
    }

    pub fn release(id: u32) {
        // SAFETY: `id` came from IOPMAssertionCreateWithName and is released once.
        unsafe { IOPMAssertionRelease(id) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_accepted() {
        assert!(KeepAwake::new().is_active());
    }

    /// macOS 27 reports an app whose helper processes outlive its window as "running in the
    /// background", so staying awake must not start one.
    #[test]
    fn staying_awake_starts_no_helper_process_and_ends_when_released() {
        let assertions = || {
            let out = std::process::Command::new("pmset")
                .args(["-g", "assertions"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).into_owned()
        };
        let awake = KeepAwake::new();
        let children = std::process::Command::new("pgrep")
            .args(["-P", &std::process::id().to_string()])
            .output()
            .unwrap();
        assert!(
            children.stdout.is_empty(),
            "helpers: {}",
            String::from_utf8_lossy(&children.stdout)
        );
        assert!(assertions().contains(REASON), "the Mac is kept awake");
        drop(awake);
        assert!(!assertions().contains(REASON), "and allowed to sleep again");
    }
}
