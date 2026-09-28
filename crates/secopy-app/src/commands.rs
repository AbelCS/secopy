//! The commands the UI calls (see `ui/src/lib/bindings.ts`, generated from these). Thin
//! wrappers: the work is in `session` and `jobs`, run off the main thread so the window
//! never freezes (NFR-5).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::dto::{
    ConflictPolicy, ExtensionKey, FinishedRow, JobOutcome, ProfilesView, ProgressView, QueueEvent,
    QueueResult, QueueResultView, QueueSummaryView, QueueView, QueuedJobView, SessionView,
    StartView, SummaryView, show,
};
use crate::jobs::{JobHandle, JobSettings, Jobs, ProgressSink};
use crate::queue::{Entry, OnFailure, QUEUE, Queue, QueuedJob};
use crate::session::{Change, Session, scan_source as scan};
use crate::store::{
    PROFILES, Profile, ProfileInput, Profiles, REMEMBERED, Remembered, SETTINGS, Settings, Store,
    WindowSize,
};

/// Everything the app keeps between commands.
pub struct AppState {
    pub session: Mutex<Session>,
    pub jobs: Jobs,
    pub store: Store,
    pub settings: Mutex<Settings>,
    pub profiles: Mutex<Profiles>,
    pub remembered: Mutex<Remembered>,
    /// Saved files that couldn't be read, handed to the UI once.
    warnings: Mutex<Vec<String>>,
    /// The saved queue (plan 6).
    pub(crate) queue: Mutex<Queue>,
    pub(crate) queue_run: Mutex<QueueRun>,
}

/// The queue run in progress, and the jobs of the last one.
#[derive(Default)]
pub(crate) struct QueueRun {
    pub running: bool,
    pub cancelled: bool,
    pub handles: Vec<Option<JobHandle>>,
    /// The thread running the queue, joined at quit.
    pub thread: Option<std::thread::JoinHandle<()>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().expect("app state lock poisoned")
}

impl AppState {
    /// Loads what was saved in `data_dir`; reports go to `data_dir/reports`.
    pub fn new(data_dir: PathBuf) -> Self {
        let store = Store::new(data_dir.clone());
        let (settings, w1) = store.load::<Settings>(SETTINGS);
        let (profiles, w2) = store.load::<Profiles>(PROFILES);
        let (remembered, w3) = store.load::<Remembered>(REMEMBERED);
        let (queue, w4) = store.load::<Queue>(QUEUE);
        Self {
            session: Mutex::new(Session::new()),
            jobs: Jobs::new(data_dir.join("reports")),
            store,
            settings: Mutex::new(settings),
            profiles: Mutex::new(profiles),
            remembered: Mutex::new(remembered),
            warnings: Mutex::new([w1, w2, w3, w4].into_iter().flatten().collect()),
            queue: Mutex::new(queue),
            queue_run: Mutex::new(QueueRun::default()),
        }
    }

    pub fn start_view(&self) -> StartView {
        // One lock at a time: a guard in a struct literal lives to the end of it.
        let session = session(self).view();
        let settings = lock(&self.settings).clone();
        let profiles = lock(&self.profiles).profiles.clone();
        let verify = lock(&self.remembered).verify;
        let warnings = std::mem::take(&mut *lock(&self.warnings));
        // Loaded again only when its source is there: no error at launch for a card that
        // isn't inserted.
        let last = lock(&self.remembered).last_profile.clone();
        let last_profile = last.filter(|id| {
            profiles
                .iter()
                .any(|p| &p.id == id && (p.source.is_empty() || Path::new(&p.source).exists()))
        });
        StartView {
            session,
            settings,
            profiles,
            verify,
            recent_destinations: self.recent(),
            warnings,
            last_profile,
        }
    }

    /// The recent destinations that still exist (B7).
    pub fn recent(&self) -> Vec<String> {
        lock(&self.remembered)
            .recent_destinations
            .iter()
            .filter(|d| Path::new(d).is_dir())
            .cloned()
            .collect()
    }

    /// Applies a change to FROM and runs the scan it needs without the session locked.
    pub fn rescan(&self, change: Change) -> SessionView {
        let pending = match session(self).begin(change) {
            Ok(pending) => pending,
            Err(view) => return *view,
        };
        let scanned = scan(&pending.source);
        session(self).finish_scan(pending, scanned)
    }

    /// Changes what the app remembers by itself and saves it; a failed save is only logged.
    pub fn remember(&self, change: impl FnOnce(&mut Remembered)) {
        let mut remembered = lock(&self.remembered);
        change(&mut remembered);
        if let Err(e) = self.store.save(REMEMBERED, &*remembered) {
            eprintln!("Secopy: couldn't save {REMEMBERED}: {e}");
        }
    }

    /// Kept in memory; written when the app quits.
    pub fn window_resized(&self, size: WindowSize) {
        lock(&self.remembered).window = Some(size);
    }

    pub fn saved_window(&self) -> Option<WindowSize> {
        lock(&self.remembered).window
    }

    pub fn select_profile(&self, id: Option<String>) -> Result<SessionView, String> {
        let profile = match &id {
            Some(id) => Some(
                lock(&self.profiles)
                    .get(id)
                    .cloned()
                    .ok_or("That profile no longer exists.")?,
            ),
            None => None,
        };
        self.remember(|r| r.last_profile = id);
        Ok(self.rescan(Change::Profile(profile)))
    }

    /// Changes the profiles and saves them, holding their lock throughout so saves at the
    /// same time can't undo each other; memory changes only when the file is written.
    fn change_profiles<T>(
        &self,
        change: impl FnOnce(&mut Profiles) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut profiles = lock(&self.profiles);
        let mut next = profiles.clone();
        let result = change(&mut next)?;
        self.store
            .save(PROFILES, &next)
            .map_err(|e| format!("Couldn't save the profile: {e}"))?;
        *profiles = next;
        Ok(result)
    }

    fn profiles_view(&self, session: SessionView) -> ProfilesView {
        ProfilesView {
            profiles: lock(&self.profiles).profiles.clone(),
            session,
        }
    }

    /// Update profile: this run's choices go into the selected profile.
    pub fn update_profile(&self) -> Result<ProfilesView, String> {
        let updated = {
            let s = session(self);
            if s.scan_pending() {
                return Err("Wait until the scan finishes.".into());
            }
            s.updated_profile().ok_or("No profile is selected.")?
        };
        self.change_profiles(|p| {
            p.replace(updated.clone());
            Ok(())
        })?;
        let view = session(self).profile_saved(updated);
        Ok(self.profiles_view(view))
    }

    /// Save as new…: this run's source and choices under a new name, then selected.
    pub fn save_profile_as(&self, name: String) -> Result<ProfilesView, String> {
        let (source, (include_folder, extensions)) = {
            let s = session(self);
            let choices = s.choices().ok_or("Wait until the scan finishes.")?;
            let source = s
                .picked_source()
                .ok_or("A profile saves a directory as its source; pick a directory first.")?;
            (source, choices)
        };
        let profile = self.change_profiles(|p| {
            p.add(ProfileInput {
                name,
                source,
                include_folder,
                extensions,
            })
        })?;
        self.remember(|r| r.last_profile = Some(profile.id.clone()));
        let view = self.rescan(Change::Profile(Some(profile)));
        Ok(self.profiles_view(view))
    }

    /// Settings → New.
    pub fn create_profile(&self, input: ProfileInput) -> Result<Vec<Profile>, String> {
        self.change_profiles(|p| p.add(input))?;
        Ok(lock(&self.profiles).profiles.clone())
    }

    /// Settings → Edit; the selected profile is applied again.
    pub fn edit_profile(&self, id: &str, input: ProfileInput) -> Result<ProfilesView, String> {
        let edited = self.change_profiles(|p| p.edit(id, input))?;
        let selected = session(self).profile().is_some_and(|p| p.id == id);
        let view = if selected {
            self.rescan(Change::Profile(Some(edited)))
        } else {
            session(self).view()
        };
        Ok(self.profiles_view(view))
    }

    /// Settings → Delete; a selected profile becomes None.
    pub fn delete_profile(&self, id: &str) -> Result<ProfilesView, String> {
        self.change_profiles(|p| {
            p.delete(id);
            Ok(())
        })?;
        let selected = session(self).profile().is_some_and(|p| p.id == id);
        let view = if selected {
            self.remember(|r| r.last_profile = None);
            self.rescan(Change::Profile(None))
        } else {
            session(self).view()
        };
        Ok(self.profiles_view(view))
    }

    /// Applies at once (the next job uses it) and saves.
    pub fn set_settings(&self, settings: Settings) -> Result<Settings, String> {
        // Held across the save, so the last change in memory is also the last one on disk.
        let mut current = lock(&self.settings);
        *current = settings.clone();
        self.store
            .save(SETTINGS, &*current)
            .map_err(|e| format!("Couldn't save the settings: {e}"))?;
        Ok(settings)
    }

    /// Starts the job with the current settings and remembers the destination (B7).
    pub fn start(&self, verify: bool, sink: impl ProgressSink) -> Result<(), String> {
        if lock(&self.queue_run).running {
            return Err("The queue is running.".into());
        }
        let (ready, dest) = {
            let s = session(self);
            let ready = s
                .ready()
                .ok_or("Nothing to copy, or something blocks the copy.")?;
            (ready, s.destination().map(show))
        };
        let settings = JobSettings::from(&*lock(&self.settings));
        self.jobs.start(ready, verify, settings, sink)?;
        if let Some(dest) = dest {
            self.remember(|r| r.used_destination(&dest));
        }
        Ok(())
    }

    /// "Retry failed": the failed files of the last job become the source.
    pub fn retry_failed(&self) -> Result<SessionView, String> {
        let (source, selection) = self.jobs.retry().ok_or("No files failed.")?;
        // With the card pulled out: say so instead of failing every file.
        if let Some(path) = crate::jobs::source_path(&source)
            && !path.exists()
        {
            return Err(format!(
                "The source isn't there any more ({}). Connect the card again to retry.",
                path.display()
            ));
        }
        Ok(session(self).install_retry(source, selection))
    }
}

pub trait QueueSink: Clone + Send + Sync + 'static {
    fn send(&self, event: QueueEvent);
}

/// A queued job's progress, forwarded as queue events.
struct Forward<S>(S);
impl<S: QueueSink> ProgressSink for Forward<S> {
    fn send(&self, view: ProgressView) {
        self.0.send(QueueEvent::Progress { view });
    }
}

fn job_view(entry: &Entry) -> QueuedJobView {
    match &entry.job {
        QueuedJob::Copy(job) => QueuedJobView {
            kind: "copy".into(),
            verify: job.verify,
            source: match job.sources.as_slice() {
                [one] => show(one),
                many => format!("{} files", many.len()),
            },
            destination: show(&job.destination),
            last_error: entry.last_error.clone(),
            supported: true,
        },
        QueuedJob::Unknown(_) => QueuedJobView {
            kind: "unknown".into(),
            verify: false,
            source: String::new(),
            destination: String::new(),
            last_error: Some("Needs a newer Secopy.".into()),
            supported: false,
        },
    }
}

impl AppState {
    pub fn queue_view(&self) -> QueueView {
        let q = lock(&self.queue);
        QueueView {
            jobs: q.jobs.iter().map(job_view).collect(),
            on_failure: q.on_failure,
            running: lock(&self.queue_run).running,
        }
    }

    /// Changes the queue and saves it; `change` returns false for a bad index.
    pub fn change_queue(
        &self,
        change: impl FnOnce(&mut Queue) -> bool,
    ) -> Result<QueueView, String> {
        {
            let mut q = lock(&self.queue);
            let mut next = q.clone();
            if !change(&mut next) {
                return Err("That job is no longer in the queue.".into());
            }
            self.store
                .save(QUEUE, &next)
                .map_err(|e| format!("Couldn't save the queue: {e}"))?;
            *q = next;
        }
        Ok(self.queue_view())
    }

    pub fn add_to_queue(&self, verify: bool) -> Result<QueueView, String> {
        let job = session(self)
            .copy_job(verify)
            .ok_or("Set up a copy first: a source, a destination and something to copy.")?;
        self.change_queue(|q| {
            q.add(job);
            true
        })
    }

    /// A copy or the queue is running.
    pub fn busy(&self) -> bool {
        self.jobs.is_running() || lock(&self.queue_run).running
    }

    /// Cancel: the current job, and the queue if it runs (spec Q5).
    pub fn cancel(&self) {
        // Under the queue's lock, so it can't slip between a queued job's check and its start.
        let mut run = lock(&self.queue_run);
        run.cancelled = true;
        self.jobs.cancel();
    }

    pub fn queue_job(&self, index: usize) -> Option<JobHandle> {
        lock(&self.queue_run).handles.get(index).cloned().flatten()
    }

    /// Runs the saved queue, one job after another (FR-40..FR-43). Blocking.
    pub fn run_queue(&self, sink: impl QueueSink) -> Result<QueueSummaryView, String> {
        {
            let mut run = lock(&self.queue_run);
            if run.running || self.jobs.is_running() {
                return Err("A copy or the queue is already running.".into());
            }
            run.running = true;
            run.cancelled = false;
            run.handles.clear();
        }
        let _awake = secopy_core::awake::KeepAwake::new();
        let clock = std::time::Instant::now();
        let (entries, on_failure) = {
            let q = lock(&self.queue);
            (q.jobs.clone(), q.on_failure)
        };
        let count = entries.len() as u32;
        let mut results = Vec::new();
        let mut keep = Vec::new();
        let mut stop = false;
        for (index, entry) in entries.iter().enumerate() {
            if stop || lock(&self.queue_run).cancelled {
                lock(&self.queue_run).handles.push(None);
                results.push(QueueResultView {
                    job: job_view(entry),
                    result: QueueResult::NotRun,
                    reason: Some("Not run: the queue stopped.".into()),
                    summary: None,
                });
                keep.push(entry.clone());
                continue;
            }
            sink.send(QueueEvent::JobStarted {
                index: index as u32,
                count,
            });
            let (result, reason, handle) = self.run_one(entry, &sink);
            let summary = handle.as_ref().and_then(JobHandle::summary);
            lock(&self.queue_run).handles.push(handle);
            let failed = matches!(result, QueueResult::Failed);
            if result != QueueResult::Complete {
                keep.push(Entry {
                    last_error: if failed {
                        reason.clone()
                    } else {
                        entry.last_error.clone()
                    },
                    ..entry.clone()
                });
            }
            stop |= result == QueueResult::Cancelled || (failed && on_failure == OnFailure::Stop);
            results.push(QueueResultView {
                job: job_view(entry),
                result,
                reason,
                summary,
            });
            // Saved after every job: completed ones gone, the rest kept (Review focus 3).
            let remaining: Vec<Entry> = keep
                .iter()
                .cloned()
                .chain(entries[index + 1..].iter().cloned())
                .collect();
            let _ = self.change_queue(|q| {
                q.jobs = remaining;
                true
            });
        }
        lock(&self.queue_run).running = false;
        let summary = QueueSummaryView {
            complete: count_of(&results, QueueResult::Complete),
            count,
            millis: clock.elapsed().as_millis() as u64,
            results,
        };
        sink.send(QueueEvent::Done {
            summary: summary.clone(),
        });
        Ok(summary)
    }

    fn run_one(
        &self,
        entry: &Entry,
        sink: &impl QueueSink,
    ) -> (QueueResult, Option<String>, Option<JobHandle>) {
        let QueuedJob::Copy(job) = &entry.job else {
            return (
                QueueResult::Failed,
                Some("Needs a newer Secopy.".into()),
                None,
            );
        };
        let ready = match crate::queue::prepare(job) {
            Ok(ready) => ready,
            Err(reason) => return (QueueResult::Failed, Some(reason), None),
        };
        let settings = JobSettings::from(&*lock(&self.settings));
        {
            // A cancel during the scan above stops the job before it starts: the check and
            // the start happen under the lock `cancel` takes.
            let run = lock(&self.queue_run);
            if run.cancelled {
                return (QueueResult::Cancelled, Some("Cancelled.".into()), None);
            }
            if let Err(reason) = self
                .jobs
                .start(ready, job.verify, settings, Forward(sink.clone()))
            {
                return (QueueResult::Failed, Some(reason), None);
            }
        }
        self.remember(|r| r.used_destination(&show(&job.destination)));
        self.jobs.wait();
        let handle = self.jobs.current_handle();
        let outcome = handle
            .as_ref()
            .and_then(JobHandle::summary)
            .map(|s| s.outcome);
        let (result, reason) = match outcome {
            Some(JobOutcome::Complete) => (QueueResult::Complete, None),
            Some(JobOutcome::Cancelled) => (QueueResult::Cancelled, Some("Cancelled.".into())),
            Some(JobOutcome::Failures) => {
                let n = handle
                    .as_ref()
                    .and_then(JobHandle::summary)
                    .map_or(0, |s| s.failed);
                (
                    QueueResult::Failed,
                    Some(format!(
                        "{n} {} failed.",
                        if n == 1 { "file" } else { "files" }
                    )),
                )
            }
            Some(JobOutcome::Stopped) | None => (
                QueueResult::Failed,
                handle
                    .as_ref()
                    .and_then(JobHandle::summary)
                    .and_then(|s| s.stopped_because)
                    .or(Some("Stopped.".into())),
            ),
        };
        (result, reason, handle)
    }
}

fn count_of(results: &[QueueResultView], kind: QueueResult) -> u32 {
    results.iter().filter(|r| r.result == kind).count() as u32
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

/// Scans a picked, dropped or chosen drive or source (FR-1..FR-3). A newer scan replaces
/// an older one.
#[tauri::command]
#[specta::specta]
pub async fn scan_source(app: AppHandle, paths: Vec<String>) -> Result<SessionView, String> {
    blocking(app, move |state| {
        state.rescan(Change::Pick(paths.into_iter().map(PathBuf::from).collect()))
    })
    .await
}

/// The "Include the folder" checkbox (FR-4); this run's file types stay.
#[tauri::command]
#[specta::specta]
pub async fn set_include_folder(app: AppHandle, include: bool) -> Result<SessionView, String> {
    blocking(app, move |state| {
        state.rescan(Change::IncludeFolder(include))
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
    blocking(app, move |state| state.start(verify, on_progress)).await?
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
    app.state::<AppState>().cancel();
}

#[tauri::command]
#[specta::specta]
pub fn job_running(app: AppHandle) -> bool {
    app.state::<AppState>().busy()
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

/// The UI says which File menu items apply.
#[tauri::command]
#[specta::specta]
pub fn set_menu_state(app: AppHandle, setup: bool, can_start: bool, copying: bool) {
    if let Some(menu) = app.try_state::<crate::FileMenu<tauri::Wry>>() {
        menu.update(setup, can_start, copying);
    }
}

impl QueueSink for Channel<QueueEvent> {
    fn send(&self, event: QueueEvent) {
        let _ = Channel::send(self, event);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn queue(app: AppHandle) -> Result<QueueView, String> {
    blocking(app, |state| state.queue_view()).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_to_queue(app: AppHandle, verify: bool) -> Result<QueueView, String> {
    blocking(app, move |state| state.add_to_queue(verify)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn remove_from_queue(app: AppHandle, index: u32) -> Result<QueueView, String> {
    blocking(app, move |state| {
        state.change_queue(|q| q.remove(index as usize))
    })
    .await?
}

#[tauri::command]
#[specta::specta]
pub async fn move_in_queue(app: AppHandle, from: u32, to: u32) -> Result<QueueView, String> {
    blocking(app, move |state| {
        state.change_queue(|q| q.move_job(from as usize, to as usize))
    })
    .await?
}

#[tauri::command]
#[specta::specta]
pub async fn clear_queue(app: AppHandle) -> Result<QueueView, String> {
    blocking(app, |state| {
        state.change_queue(|q| {
            q.clear();
            true
        })
    })
    .await?
}

#[tauri::command]
#[specta::specta]
pub async fn set_queue_on_failure(
    app: AppHandle,
    on_failure: OnFailure,
) -> Result<QueueView, String> {
    blocking(app, move |state| {
        state.change_queue(|q| {
            q.on_failure = on_failure;
            true
        })
    })
    .await?
}

/// Starts the queue on its own thread; events arrive on `on_event`.
#[tauri::command]
#[specta::specta]
pub fn run_queue(app: AppHandle, on_event: Channel<QueueEvent>) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.busy() {
        return Err("A copy or the queue is already running.".into());
    }
    let runner = app.clone();
    let thread = std::thread::spawn(move || {
        let _ = runner.state::<AppState>().run_queue(on_event);
    });
    lock(&state.queue_run).thread = Some(thread);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn queue_finished_page(
    app: AppHandle,
    index: u32,
    offset: u32,
    limit: u32,
    failed_only: bool,
) -> Result<Vec<FinishedRow>, String> {
    blocking(app, move |state| {
        state
            .queue_job(index as usize)
            .map_or_else(Vec::new, |job| {
                job.finished_page(offset, limit, failed_only)
            })
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn queue_save_report(app: AppHandle, index: u32, path: String) -> Result<(), String> {
    blocking(app, move |state| {
        state
            .queue_job(index as usize)
            .ok_or("That job has no report.")?
            .save_report(&PathBuf::from(path))
    })
    .await?
}

/// "Retry failed": only the failed files, checked again (RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn retry_failed(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| state.retry_failed()).await?
}

/// Everything the window needs at start; load problems are handed out once.
#[tauri::command]
#[specta::specta]
pub async fn app_start(app: AppHandle) -> Result<StartView, String> {
    blocking(app, |state| state.start_view()).await
}

#[tauri::command]
#[specta::specta]
pub async fn recent_destinations(app: AppHandle) -> Result<Vec<String>, String> {
    blocking(app, |state| state.recent()).await
}

#[tauri::command]
#[specta::specta]
pub async fn select_profile(app: AppHandle, id: Option<String>) -> Result<SessionView, String> {
    blocking(app, move |state| state.select_profile(id)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn update_profile(app: AppHandle) -> Result<ProfilesView, String> {
    blocking(app, |state| state.update_profile()).await?
}

#[tauri::command]
#[specta::specta]
pub async fn save_profile_as(app: AppHandle, name: String) -> Result<ProfilesView, String> {
    blocking(app, move |state| state.save_profile_as(name)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn create_profile(app: AppHandle, input: ProfileInput) -> Result<Vec<Profile>, String> {
    blocking(app, move |state| state.create_profile(input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn edit_profile(
    app: AppHandle,
    id: String,
    input: ProfileInput,
) -> Result<ProfilesView, String> {
    blocking(app, move |state| state.edit_profile(&id, input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn delete_profile(app: AppHandle, id: String) -> Result<ProfilesView, String> {
    blocking(app, move |state| state.delete_profile(&id)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn set_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    blocking(app, move |state| state.set_settings(settings)).await?
}

/// Copy or Copy & Verify, remembered for the next launch (FR-36).
#[tauri::command]
#[specta::specta]
pub async fn set_mode(app: AppHandle, verify: bool) -> Result<(), String> {
    blocking(app, move |state| state.remember(|r| r.verify = verify)).await
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Mutex as StdMutex};

    use super::*;
    use crate::store::{PROFILES, REMEMBERED, SETTINGS};

    #[derive(Clone, Default)]
    struct Sink(Arc<StdMutex<Vec<ProgressView>>>);

    impl ProgressSink for Sink {
        fn send(&self, view: ProgressView) {
            self.0.lock().unwrap().push(view);
        }
    }

    fn input(name: &str, source: &Path) -> ProfileInput {
        ProfileInput {
            name: name.into(),
            source: show(source),
            include_folder: true,
            extensions: None,
        }
    }

    #[test]
    fn a_missing_last_profile_starts_with_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(REMEMBERED),
            r#"{"version": 1, "lastProfile": "gone"}"#,
        )
        .unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let start = state.start_view();
        assert_eq!(start.last_profile, None);
        assert!(start.warnings.is_empty());
    }

    #[test]
    fn the_last_profile_and_mode_come_back() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        let state = AppState::new(dir.path().join("data"));
        let id = state.create_profile(input("FX3", &card)).unwrap()[0]
            .id
            .clone();
        state.select_profile(Some(id.clone())).unwrap();
        state.remember(|r| r.verify = false);
        let again = AppState::new(dir.path().join("data")).start_view();
        assert_eq!(again.last_profile, Some(id), "the UI loads it again");
        assert!(!again.verify);
        assert!(
            again.session.destination.is_none(),
            "the destination is never restored"
        );
        assert_eq!(again.profiles.len(), 1);
        fs::remove_dir(&card).unwrap();
        let later = AppState::new(dir.path().join("data")).start_view();
        assert_eq!(
            later.last_profile, None,
            "its card isn't there: nothing to load, and no error at launch"
        );
    }

    #[test]
    fn a_damaged_file_is_reported_once() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SETTINGS), b"nonsense").unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let first = state.start_view();
        assert_eq!(first.warnings.len(), 1);
        assert!(first.settings.write_checksum_file, "defaults");
        assert!(state.start_view().warnings.is_empty());
    }

    #[test]
    fn deleting_the_selected_profile_selects_none() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let id = state.create_profile(input("FX3", Path::new(""))).unwrap()[0]
            .id
            .clone();
        state.select_profile(Some(id.clone())).unwrap();
        let after = state.delete_profile(&id).unwrap();
        assert!(after.profiles.is_empty());
        assert_eq!(after.session.profile_id, None);
        assert_eq!(state.remembered.lock().unwrap().last_profile, None);
        let saved = fs::read_to_string(dir.path().join(PROFILES)).unwrap();
        assert!(!saved.contains("FX3"));
    }

    #[test]
    fn editing_the_selected_profile_applies_it() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let clip = dir.path().join("CLIP");
        fs::create_dir_all(&clip).unwrap();
        fs::write(clip.join("a.mp4"), b"a").unwrap();
        let id = state.create_profile(input("FX3", Path::new(""))).unwrap()[0]
            .id
            .clone();
        state.select_profile(Some(id.clone())).unwrap();
        let after = state.edit_profile(&id, input("FX3 A-cam", &clip)).unwrap();
        assert_eq!(after.profiles[0].name, "FX3 A-cam");
        assert_eq!(after.session.profile_id, Some(id));
        assert_eq!(after.session.source.unwrap().label, show(&clip), "loaded");
    }

    #[test]
    fn save_as_new_selects_and_saves_the_new_profile() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(card.join("DCIM")).unwrap();
        fs::write(card.join("DCIM/a.jpg"), b"a").unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.rescan(Change::Pick(vec![card.join("DCIM")]));
        let after = state.save_profile_as("Photos".into()).unwrap();
        let id = after.profiles[0].id.clone();
        assert_eq!(after.profiles[0].source, show(&card.join("DCIM")));
        assert_eq!(after.session.profile_id, Some(id.clone()));
        assert!(!after.session.profile_changed);
        assert_eq!(state.remembered.lock().unwrap().last_profile, Some(id));
        assert!(
            state
                .save_profile_as("photos".into())
                .unwrap_err()
                .contains("already a profile")
        );
        state.rescan(Change::Pick(vec![card.join("DCIM/a.jpg")]));
        assert_eq!(
            state.save_profile_as("One file".into()).unwrap_err(),
            "A profile saves a directory as its source; pick a directory first."
        );
    }

    #[test]
    fn starting_a_job_remembers_the_destination_and_uses_the_settings() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("a.mov"), b"a").unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        let state = AppState::new(dir.path().join("data"));
        state
            .set_settings(Settings {
                write_checksum_file: false,
                ..Settings::default()
            })
            .unwrap();
        state.rescan(Change::Pick(vec![card]));
        state
            .session
            .lock()
            .unwrap()
            .set_destination(Some(dest.clone()));
        state.start(true, Sink::default()).unwrap();
        state.jobs.wait();
        assert!(state.jobs.summary().unwrap().checksum_off);
        assert_eq!(state.recent(), [show(&dest)]);
        fs::remove_dir_all(&dest).unwrap();
        assert!(
            state.recent().is_empty(),
            "folders that are gone are left out"
        );
    }

    #[test]
    fn saves_at_the_same_time_all_succeed_and_disk_matches_memory() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let errors = std::thread::scope(|s| {
            let tasks: Vec<_> = (0..16)
                .map(|i| {
                    let state = &state;
                    s.spawn(move || {
                        let settings = Settings {
                            write_checksum_file: i % 2 == 0,
                            show_system_count: i % 3 == 0,
                            report_next_to_checksum: i % 5 == 0,
                            notify_when_done: i % 7 == 0,
                        };
                        let a = state.set_settings(settings).err();
                        let b = state
                            .create_profile(input(&format!("P{i}"), Path::new("")))
                            .err();
                        a.into_iter().chain(b).collect::<Vec<_>>()
                    })
                })
                .collect();
            tasks
                .into_iter()
                .flat_map(|t| t.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(errors.is_empty(), "{errors:?}");
        let store = Store::new(dir.path().to_path_buf());
        assert_eq!(
            store.load::<Settings>(SETTINGS).0,
            *state.settings.lock().unwrap()
        );
        let saved = store.load::<Profiles>(PROFILES).0;
        assert_eq!(saved.profiles.len(), 16, "no profile lost");
        assert_eq!(saved, *state.profiles.lock().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn retry_after_the_source_is_gone_has_nothing_to_start() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("a.mov"), b"a").unwrap();
        fs::write(card.join("b.mov"), b"b").unwrap();
        fs::set_permissions(card.join("b.mov"), fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(card.join("b.mov")).is_ok() {
            return; // running as root
        }
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.rescan(Change::Pick(vec![card.clone()]));
        state.session.lock().unwrap().set_destination(Some(dest));
        state.start(false, Sink::default()).unwrap();
        state.jobs.wait();
        fs::set_permissions(card.join("b.mov"), fs::Permissions::from_mode(0o644)).unwrap();
        fs::remove_dir_all(&card).unwrap(); // the card was ejected
        let error = state.retry_failed().unwrap_err();
        assert!(
            error.starts_with("The source isn't there any more"),
            "{error}"
        );
        assert!(!state.jobs.is_running(), "nothing started");
    }

    use crate::queue::{CopyJob, OnFailure, Queue, QueuedJob};

    #[derive(Clone, Default)]
    struct Events(Arc<StdMutex<Vec<QueueEvent>>>);
    impl QueueSink for Events {
        fn send(&self, e: QueueEvent) {
            self.0.lock().unwrap().push(e);
        }
    }

    /// A state with `n` sources of `files` files each, all queued to one destination.
    fn queued(dir: &Path, sources: &[(&str, usize)]) -> AppState {
        let state = AppState::new(dir.join("data"));
        let dest = dir.join("dest");
        fs::create_dir_all(&dest).unwrap();
        for (name, files) in sources {
            let src = dir.join(name);
            fs::create_dir_all(&src).unwrap();
            for i in 0..*files {
                fs::write(src.join(format!("{i}.mov")), b"clip").unwrap();
            }
            lock(&state.queue).add(CopyJob {
                sources: vec![src],
                include_folder: true,
                extensions: None,
                destination: dest.clone(),
                conflicts: ConflictPolicy::KeepBoth,
                verify: true,
            });
        }
        state
    }

    #[test]
    fn the_queue_runs_every_job_and_empties() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 2), ("B", 1)]);
        let events = Events::default();
        let summary = state.run_queue(events.clone()).unwrap();
        assert_eq!((summary.complete, summary.count), (2, 2));
        assert!(
            dir.path().join("dest/A/0.mov").exists() && dir.path().join("dest/B/0.mov").exists()
        );
        assert!(
            state.queue_view().jobs.is_empty(),
            "complete jobs leave the queue"
        );
        let saved: Queue = state.store.load(crate::queue::QUEUE).0;
        assert!(saved.jobs.is_empty());
        let started: Vec<u32> = events
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                QueueEvent::JobStarted { index, count } => Some(index * 10 + count),
                _ => None,
            })
            .collect();
        assert_eq!(started, [2, 12], "job 0 of 2, then job 1 of 2");
        assert_eq!(state.queue_job(1).unwrap().summary().unwrap().files, 1);
    }

    #[test]
    fn a_job_that_cant_start_fails_and_the_queue_goes_on() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1), ("B", 1)]);
        fs::remove_dir_all(dir.path().join("A")).unwrap();
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(summary.results[0].result, QueueResult::Failed);
        assert!(
            summary.results[0]
                .reason
                .as_deref()
                .unwrap()
                .ends_with("isn't there any more.")
        );
        assert_eq!(summary.results[1].result, QueueResult::Complete);
        let left = state.queue_view().jobs;
        assert_eq!(left.len(), 1, "the failed job stays");
        assert!(left[0].last_error.is_some());
    }

    /// Review focus 2.
    #[test]
    fn stop_on_failure_leaves_the_rest_not_run() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1), ("B", 1)]);
        lock(&state.queue).on_failure = OnFailure::Stop;
        fs::remove_dir_all(dir.path().join("A")).unwrap();
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(summary.results[1].result, QueueResult::NotRun);
        assert!(!dir.path().join("dest/B").exists(), "B never started");
        assert_eq!(state.queue_view().jobs.len(), 2);
    }

    /// Review focus 5.
    #[test]
    fn an_unknown_job_fails_and_the_queue_goes_on() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        lock(&state.queue).jobs.insert(
            0,
            crate::queue::Entry {
                job: QueuedJob::Unknown(serde_json::json!({"kind": "mirror", "preset": "p"})),
                last_error: None,
            },
        );
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(
            summary.results[0].reason.as_deref(),
            Some("Needs a newer Secopy.")
        );
        assert_eq!(summary.results[1].result, QueueResult::Complete);
    }

    /// Review focus 3.
    #[test]
    fn cancel_stops_the_queue_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 400), ("B", 1)]);
        state.cancel(); // before the run: flag is reset by the run, so cancel during it
        std::thread::scope(|s| {
            let run = s.spawn(|| state.run_queue(Events::default()).unwrap());
            // Cancel as soon as the first job is running.
            while !state.jobs.is_running() && !run.is_finished() {
                std::thread::yield_now();
            }
            state.cancel();
            let summary = run.join().unwrap();
            assert_ne!(summary.results[1].result, QueueResult::Complete);
        });
        assert!(!state.busy());
        let left = state.queue_view().jobs;
        assert_eq!(
            left.len(),
            2,
            "the cancelled job and the one not run both stay"
        );
    }

    /// Review focus 4.
    #[test]
    fn one_run_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        lock(&state.queue_run).running = true;
        assert_eq!(
            state.run_queue(Events::default()).unwrap_err(),
            "A copy or the queue is already running."
        );
        assert_eq!(
            state.start(true, Sink::default()).unwrap_err(),
            "The queue is running."
        );
        lock(&state.queue_run).running = false;
    }

    #[test]
    fn add_to_queue_takes_the_setup_and_saves_it() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[]);
        assert!(
            state
                .add_to_queue(true)
                .unwrap_err()
                .contains("Set up a copy first")
        );
        let src = dir.path().join("S");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.mov"), b"a").unwrap();
        state.rescan(Change::Pick(vec![src]));
        session(&state).set_destination(Some(dir.path().join("dest")));
        let view = state.add_to_queue(false).unwrap();
        assert_eq!(view.jobs.len(), 1);
        assert!(!view.jobs[0].verify);
        assert_eq!(
            state.store.load::<Queue>(crate::queue::QUEUE).0.jobs.len(),
            1
        );
    }

    /// A cancel that lands while the next job is being scanned stops it before it starts.
    #[test]
    fn a_cancel_between_jobs_stops_the_next_one_before_it_starts() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        let entry = lock(&state.queue).jobs[0].clone();
        {
            let mut run = lock(&state.queue_run);
            run.running = true;
            run.cancelled = true;
        }
        let (result, _, handle) = state.run_one(&entry, &Events::default());
        assert_eq!(result, QueueResult::Cancelled);
        assert!(handle.is_none());
        assert!(!dir.path().join("dest/A").exists(), "nothing copied");
        lock(&state.queue_run).running = false;
    }
}
