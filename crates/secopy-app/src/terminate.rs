//! Quitting from outside the app (the Dock's Quit, logout, restart, shutdown) asks first while
//! a job runs, as ⌘Q does (#211). macOS asks the app delegate `applicationShouldTerminate:`;
//! tao's delegate doesn't answer it, so macOS quit at once and the job was stopped.

use std::sync::OnceLock;

use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::NSApplication;
use tauri::{AppHandle, Manager};

use crate::commands::AppState;

/// `NSTerminateCancel`: the app stays.
pub(crate) const CANCEL: usize = 0;
/// `NSTerminateNow`: the app quits.
pub(crate) const NOW: usize = 1;

/// What to tell macOS: while a job or the queue runs, not now (the window asks, as for ⌘Q).
pub(crate) fn reply(busy: bool) -> usize {
    if busy { CANCEL } else { NOW }
}

/// The app the delegate method asks, set once by `install`.
static APP: OnceLock<AppHandle> = OnceLock::new();

/// From now on, macOS asks Secopy before quitting it. Call on the main thread once tao has
/// made its app delegate (in `setup`). tao has no such method, so none is replaced.
pub(crate) fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let Some(class) = AnyClass::get(c"TaoAppDelegateParent") else {
        eprintln!("Secopy: no app delegate to ask before quitting");
        return;
    };
    let answer: extern "C-unwind" fn(&AnyObject, Sel, *mut AnyObject) -> usize = should_terminate;
    // SAFETY: `answer` has the method's signature, `- (NSApplicationTerminateReply)
    // applicationShouldTerminate:(NSApplication *)sender`, whose type encoding is "Q@:@";
    // the runtime calls it through `Imp`, the untyped function pointer it stores.
    let added = unsafe {
        let imp: Imp = std::mem::transmute::<_, Imp>(answer);
        objc2::ffi::class_addMethod(
            std::ptr::from_ref(class).cast_mut(),
            sel!(applicationShouldTerminate:),
            imp,
            c"Q@:@".as_ptr(),
        )
    };
    if !added.as_bool() {
        eprintln!("Secopy: couldn't add applicationShouldTerminate:");
        return;
    }
    // AppKit notes what its delegate answers when the delegate is set: set again, so it
    // sees the new method.
    if let Some(mtm) = MainThreadMarker::new() {
        let ns = NSApplication::sharedApplication(mtm);
        if let Some(delegate) = ns.delegate() {
            ns.setDelegate(Some(&delegate));
        }
    }
}

/// `applicationShouldTerminate:`: macOS wants Secopy to quit (the Dock, logout, shutdown).
extern "C-unwind" fn should_terminate(_: &AnyObject, _: Sel, _: *mut AnyObject) -> usize {
    let Some(app) = APP.get() else { return NOW };
    let answer = reply(crate::asks_before_quitting(&app.state::<AppState>()));
    if answer == CANCEL {
        // As ⌘Q: the window comes to the front and asks.
        crate::quit(app);
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #211: while a job or the queue runs, the quit is refused (the window asks instead);
    /// with nothing running it goes ahead.
    #[test]
    fn a_quit_from_outside_waits_for_the_answer_while_a_job_runs() {
        assert_eq!(reply(true), CANCEL);
        assert_eq!(reply(false), NOW);
    }
}
