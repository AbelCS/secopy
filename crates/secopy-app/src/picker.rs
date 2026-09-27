//! FROM's Choose… (RFD §5.2): one macOS open panel where a folder or files can be picked.
//! The dialog plugin only picks one kind at a time.

use std::sync::mpsc::Sender;

use tauri::{AppHandle, Manager, Runtime};

/// Shows the panel as a sheet on the main window; sends the picked paths, or `None` if
/// cancelled, to `done`.
#[cfg(target_os = "macos")]
pub fn pick_source<R: Runtime>(
    app: &AppHandle<R>,
    done: Sender<Option<Vec<String>>>,
) -> tauri::Result<()> {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSOpenPanel, NSWindow};
    use objc2_foundation::NSString;

    let window = app.get_webview_window("main");
    app.run_on_main_thread(move || {
        let mtm = MainThreadMarker::new().expect("runs on the main thread");
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(true);
        panel.setAllowsMultipleSelection(true);
        panel.setCanCreateDirectories(false);
        panel.setMessage(Some(&NSString::from_str(
            "Choose a folder, or one or more files",
        )));
        panel.setPrompt(Some(&NSString::from_str("Choose")));
        let answered = panel.clone();
        let handler = RcBlock::new(move |response: NSModalResponse| {
            let paths = (response == NSModalResponseOK).then(|| {
                answered
                    .URLs()
                    .iter()
                    .filter_map(|url| url.path())
                    .map(|path| path.to_string())
                    .collect()
            });
            let _ = done.send(paths);
        });
        match window.and_then(|w| w.ns_window().ok()) {
            // SAFETY: Tauri's `ns_window` is the window's live NSWindow, used here on the
            // main thread while the window exists.
            Some(parent) => unsafe {
                let parent = &*(parent as *const NSWindow);
                panel.beginSheetModalForWindow_completionHandler(parent, &handler);
            },
            None => panel.beginWithCompletionHandler(&handler),
        }
    })
}

#[cfg(not(target_os = "macos"))]
pub fn pick_source<R: Runtime>(
    _app: &AppHandle<R>,
    done: Sender<Option<Vec<String>>>,
) -> tauri::Result<()> {
    let _ = done.send(None);
    Ok(())
}
