//! The commands the UI calls (see `ui/src/lib/bindings.ts`, generated from these). Thin
//! wrappers: the work is in `session` and `jobs`, run off the main thread so the window
//! never freezes (NFR-5).

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::dto::{
    ConflictPolicy, ExtensionKey, FinishedRow, ProgressView, SessionView, SummaryView,
};
use crate::jobs::{Jobs, ProgressSink};
use crate::session::{Change, Session, scan_source as scan};

/// Everything the app keeps between commands.
pub struct AppState {
    pub session: Mutex<Session>,
    pub jobs: Jobs,
}

impl AppState {
    pub fn new(reports_dir: PathBuf) -> Self {
        Self {
            session: Mutex::new(Session::new()),
            jobs: Jobs::new(reports_dir),
        }
    }
}

impl ProgressSink for Channel<ProgressView> {
    fn send(&self, view: ProgressView) {
        // The window may be gone (the app is quitting); the job carries on regardless.
        let _ = Channel::send(self, view);
    }
}

/// Runs `f` on a blocking thread with the app state: scanning and checking touch the disk.
async fn blocking<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())
}

fn session(state: &AppState) -> std::sync::MutexGuard<'_, Session> {
    state.session.lock().expect("session lock poisoned")
}

/// FROM's Choose…: a folder or files, in one panel (FR-1, FR-2). `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn pick_source(app: AppHandle) -> Result<Option<Vec<String>>, String> {
    let (done, picked) = std::sync::mpsc::channel();
    crate::picker::pick_source(&app, done).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || picked.recv().ok().flatten())
        .await
        .map_err(|e| e.to_string())
}

/// Applies a change to FROM and runs the scan it needs without the session locked.
pub(crate) fn rescan(state: &AppState, change: Change) -> SessionView {
    let pending = match session(state).begin(change) {
        Ok(pending) => pending,
        Err(view) => return *view,
    };
    let scanned = scan(&pending.source);
    session(state).finish_scan(pending, scanned)
}

/// Scans a picked, dropped or chosen drive or source (FR-1..FR-3). A newer scan replaces
/// an older one.
#[tauri::command]
#[specta::specta]
pub async fn scan_source(app: AppHandle, paths: Vec<String>) -> Result<SessionView, String> {
    blocking(app, move |state| {
        rescan(
            state,
            Change::Pick(paths.into_iter().map(PathBuf::from).collect()),
        )
    })
    .await
}

/// The "Include the folder" checkbox (FR-4); this run's file types stay.
#[tauri::command]
#[specta::specta]
pub async fn set_include_folder(app: AppHandle, include: bool) -> Result<SessionView, String> {
    blocking(app, move |state| {
        rescan(state, Change::IncludeFolder(include))
    })
    .await
}

/// Clears the source; the destination stays ("New copy", RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn clear_source(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| session(state).clear_source()).await
}

/// `None` selects every extension (FR-8, FR-10).
#[tauri::command]
#[specta::specta]
pub async fn set_filter(
    app: AppHandle,
    selected: Option<Vec<ExtensionKey>>,
) -> Result<SessionView, String> {
    blocking(app, move |state| session(state).set_filter(selected)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_destination(app: AppHandle, path: Option<String>) -> Result<SessionView, String> {
    blocking(app, move |state| {
        session(state).set_destination(path.map(PathBuf::from))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_conflicts(app: AppHandle, policy: ConflictPolicy) -> Result<SessionView, String> {
    blocking(app, move |state| session(state).set_policy(policy)).await
}

#[tauri::command]
#[specta::specta]
pub async fn session_view(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| session(state).view()).await
}

/// Starts copying what the main window shows; progress arrives on `on_progress`.
#[tauri::command]
#[specta::specta]
pub async fn start_job(
    app: AppHandle,
    verify: bool,
    on_progress: Channel<ProgressView>,
) -> Result<(), String> {
    blocking(app, move |state| {
        let ready = session(state)
            .ready()
            .ok_or("Nothing to copy, or something blocks the copy.")?;
        state.jobs.start(ready, verify, on_progress)
    })
    .await?
}

#[tauri::command]
#[specta::specta]
pub fn pause_job(app: AppHandle) {
    app.state::<AppState>().jobs.pause();
}

#[tauri::command]
#[specta::specta]
pub fn resume_job(app: AppHandle) {
    app.state::<AppState>().jobs.resume();
}

#[tauri::command]
#[specta::specta]
pub fn cancel_job(app: AppHandle) {
    app.state::<AppState>().jobs.cancel();
}

#[tauri::command]
#[specta::specta]
pub fn job_running(app: AppHandle) -> bool {
    app.state::<AppState>().jobs.is_running()
}

#[tauri::command]
#[specta::specta]
pub async fn finished_page(
    app: AppHandle,
    offset: u32,
    limit: u32,
    failed_only: bool,
) -> Result<Vec<FinishedRow>, String> {
    blocking(app, move |state| {
        state.jobs.finished_page(offset, limit, failed_only)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn job_summary(app: AppHandle) -> Result<Option<SummaryView>, String> {
    blocking(app, |state| state.jobs.summary()).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_report(app: AppHandle, path: String) -> Result<(), String> {
    blocking(app, move |state| {
        state.jobs.save_report(&PathBuf::from(path))
    })
    .await?
}

/// "Retry failed": only the failed files, checked again (RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn retry_failed(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| {
        let (source, selection) = state.jobs.retry().ok_or("No files failed.")?;
        Ok(session(state).install_retry(source, selection))
    })
    .await?
}
