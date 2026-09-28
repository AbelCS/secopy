//! The commands the UI calls (see `ui/src/lib/bindings.ts`, generated from these). Thin
//! wrappers: the work is in `session` and `jobs`, run off the main thread so the window
//! never freezes (NFR-5).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::dto::{ComparedView, MirrorPreviewView, PreviewKind, PreviewRow, count};
use crate::dto::{
    ConflictPolicy, ExtensionKey, FinishedRow, JobOutcome, ProfilesView, ProgressView, QueueEvent,
    QueueResult, QueueResultView, QueueSummaryView, QueueView, QueuedJobView, SessionView,
    StartView, SummaryView, show,
};
use crate::jobs::{JobHandle, JobSettings, Jobs, ProgressSink};
use crate::mirrors::MirrorJob;
use crate::queue::{Entry, OnFailure, QUEUE, Queue, QueuedJob};
use crate::session::{Change, Ready, Session, scan_source as scan};
use crate::store::{
    MIRRORS, MirrorPreset, MirrorPresetInput, MirrorPresets, PROFILES, Profile, ProfileInput,
    Profiles, REMEMBERED, Remembered, SETTINGS, Settings, Store, WindowSize,
};
use secopy_core::control::JobControl;

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
    /// Saved mirror presets (plan 7).
    pub(crate) mirrors: Mutex<MirrorPresets>,
    /// The mirror last previewed, with its preset as it was: Run mirror runs exactly this, once
    /// (FR-47).
    preview: Mutex<Option<(MirrorPreset, MirrorJob)>>,
    /// Stops a mirror being planned (its deep check), for Preview's Cancel and the queue's.
    planning: Mutex<Vec<Arc<JobControl>>>,
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
        let (mirrors, w5) = store.load::<MirrorPresets>(MIRRORS);
        Self {
            session: Mutex::new(Session::new()),
            jobs: Jobs::new(data_dir.join("reports")),
            store,
            settings: Mutex::new(settings),
            profiles: Mutex::new(profiles),
            remembered: Mutex::new(remembered),
            warnings: Mutex::new([w1, w2, w3, w4, w5].into_iter().flatten().collect()),
            mirrors: Mutex::new(mirrors),
            preview: Mutex::new(None),
            planning: Mutex::new(Vec::new()),
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

fn job_view(entry: &Entry, mirrors: &MirrorPresets) -> QueuedJobView {
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
            name: None,
        },
        QueuedJob::Mirror { preset } => match mirrors.get(preset) {
            Some(p) => QueuedJobView {
                kind: "mirror".into(),
                verify: true,
                source: p.origin.clone(),
                destination: p.destination.clone(),
                last_error: entry.last_error.clone(),
                supported: true,
                name: Some(p.name.clone()),
            },
            None => QueuedJobView {
                kind: "mirror".into(),
                verify: true,
                source: String::new(),
                destination: String::new(),
                last_error: Some(PRESET_GONE.into()),
                supported: false,
                name: None,
            },
        },
        QueuedJob::Unknown(_) => QueuedJobView {
            kind: "unknown".into(),
            verify: false,
            source: String::new(),
            destination: String::new(),
            last_error: Some("Needs a newer Secopy.".into()),
            supported: false,
            name: None,
        },
    }
}

const PRESET_GONE: &str = "The mirror preset no longer exists.";

impl AppState {
    pub fn queue_view(&self) -> QueueView {
        let q = lock(&self.queue);
        QueueView {
            jobs: {
                let mirrors = lock(&self.mirrors);
                q.jobs.iter().map(|e| job_view(e, &mirrors)).collect()
            },
            on_failure: q.on_failure,
            running: lock(&self.queue_run).running,
        }
    }

    /// Changes the queue and saves it; `change` returns false for a bad index.
    pub fn change_queue(
        &self,
        change: impl FnOnce(&mut Queue) -> bool,
    ) -> Result<QueueView, String> {
        // The run saves what's left after each job; a change now would be lost or duplicated.
        if lock(&self.queue_run).running {
            return Err("The queue is running.".into());
        }
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

    /// Cancel: the current job, and the queue if it runs (spec Q5). `remove_copied` removes
    /// the files the current job already copied (#54).
    pub fn cancel(&self, remove_copied: bool) {
        // Under the queue's lock, so it can't slip between a queued job's check and its start.
        let mut run = lock(&self.queue_run);
        run.cancelled = true;
        self.jobs.cancel(remove_copied);
        self.cancel_preview();
    }

    /// Stops a mirror being planned: Preview's Cancel, or a queued mirror's deep check.
    pub fn cancel_preview(&self) {
        for control in lock(&self.planning).iter() {
            control.cancel();
        }
    }

    /// Plans a mirror with a control Cancel can reach.
    fn plan_mirror(
        &self,
        preset: &MirrorPreset,
        on_compared: &(dyn Fn(u64, u64) + Sync),
    ) -> Result<MirrorJob, String> {
        let control = Arc::new(JobControl::new());
        lock(&self.planning).push(control.clone());
        let job = crate::mirrors::prepare(preset, &control, on_compared);
        lock(&self.planning).retain(|c| !Arc::ptr_eq(c, &control));
        job
    }

    pub fn queue_job(&self, index: usize) -> Option<JobHandle> {
        lock(&self.queue_run).handles.get(index).cloned().flatten()
    }

    /// Runs the saved queue, one job after another (FR-40..FR-43). Blocking.
    pub fn run_queue(&self, sink: impl QueueSink) -> Result<QueueSummaryView, String> {
        self.claim_queue_run()?;
        Ok(self.run_claimed(sink))
    }

    /// Marks the queue as running, so a second Run queue is refused before any thread starts.
    pub fn claim_queue_run(&self) -> Result<(), String> {
        let mut run = lock(&self.queue_run);
        if run.running || self.jobs.is_running() {
            return Err("A copy or the queue is already running.".into());
        }
        run.running = true;
        run.cancelled = false;
        run.handles.clear();
        Ok(())
    }

    /// Runs the queue claimed by `claim_queue_run`; it is released however this ends.
    pub fn run_claimed(&self, sink: impl QueueSink) -> QueueSummaryView {
        /// Releases the run on the way out, a panic included.
        struct Release<'a>(&'a Mutex<QueueRun>);
        impl Drop for Release<'_> {
            fn drop(&mut self) {
                self.0.lock().unwrap_or_else(|e| e.into_inner()).running = false;
            }
        }
        let _release = Release(&self.queue_run);
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
        let mut save_error = None;
        for (index, entry) in entries.iter().enumerate() {
            if stop || lock(&self.queue_run).cancelled {
                lock(&self.queue_run).handles.push(None);
                results.push(QueueResultView {
                    job: job_view(entry, &lock(&self.mirrors)),
                    result: QueueResult::NotRun,
                    reason: Some("Not run: the queue stopped.".into()),
                    summary: None,
                });
                // Not run this time: an older run's reason no longer applies.
                keep.push(Entry {
                    last_error: None,
                    ..entry.clone()
                });
                continue;
            }
            sink.send(QueueEvent::JobChecking {
                index: index as u32,
                count,
            });
            let (result, reason, handle) = self.run_one(entry, (index as u32, count), &sink);
            let summary = handle.as_ref().and_then(JobHandle::summary);
            lock(&self.queue_run).handles.push(handle);
            let failed = matches!(result, QueueResult::Failed);
            if result != QueueResult::Complete {
                keep.push(Entry {
                    last_error: if failed {
                        reason.clone()
                    } else {
                        Some("Stopped.".into())
                    },
                    ..entry.clone()
                });
            }
            stop |= result == QueueResult::Cancelled || (failed && on_failure == OnFailure::Stop);
            results.push(QueueResultView {
                job: job_view(entry, &lock(&self.mirrors)),
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
            if let Err(e) = self.leave_in_queue(remaining) {
                save_error = Some(e);
            }
        }
        // Once more with the jobs not run, whose reasons were cleared.
        if let Err(e) = self.leave_in_queue(keep) {
            save_error = Some(e);
        }
        lock(&self.queue_run).running = false;
        let summary = QueueSummaryView {
            complete: count_of(&results, QueueResult::Complete),
            count,
            millis: clock.elapsed().as_millis() as u64,
            results,
            save_error,
        };
        sink.send(QueueEvent::Done {
            summary: summary.clone(),
        });
        summary
    }

    /// The jobs left after one ran: kept in memory even if they can't be saved, so a
    /// completed job never runs twice.
    fn leave_in_queue(&self, jobs: Vec<Entry>) -> Result<(), String> {
        let mut q = lock(&self.queue);
        q.jobs = jobs;
        self.store
            .save(QUEUE, &*q)
            .map_err(|e| format!("Couldn't save the queue: {e}"))
    }

    fn run_one(
        &self,
        entry: &Entry,
        (index, jobs): (u32, u32),
        sink: &impl QueueSink,
    ) -> (QueueResult, Option<String>, Option<JobHandle>) {
        let started = QueueEvent::JobStarted {
            index,
            count: jobs,
            job: job_view(entry, &lock(&self.mirrors)),
        };
        match &entry.job {
            QueuedJob::Copy(job) => {
                let ready = match crate::queue::prepare(job) {
                    Ok(ready) => ready,
                    Err(reason) => return (QueueResult::Failed, Some(reason), None),
                };
                let settings = JobSettings::from(&*lock(&self.settings));
                let ran = self.start_and_wait(ready, job.verify, settings, started, sink);
                if ran.2.is_some() {
                    self.remember(|r| r.used_destination(&show(&job.destination)));
                }
                ran
            }
            QueuedJob::Mirror { preset } => {
                let Some(preset) = lock(&self.mirrors).get(preset).cloned() else {
                    return (QueueResult::Failed, Some(PRESET_GONE.into()), None);
                };
                let compared = |done, total| {
                    sink.send(QueueEvent::Compared {
                        index,
                        done: count(done),
                        total: count(total),
                    })
                };
                let job = match self.plan_mirror(&preset, &compared) {
                    Ok(job) => job,
                    // Cancel during the deep check: the queue was cancelled, not the job failed.
                    Err(_) if lock(&self.queue_run).cancelled => {
                        return (QueueResult::Cancelled, Some("Cancelled.".into()), None);
                    }
                    Err(reason) => return (QueueResult::Failed, Some(reason), None),
                };
                // Nobody is there to confirm: a run that looks wrong doesn't start (FR-50).
                if let Some(guard) = &job.plan.guard {
                    return (QueueResult::Failed, Some(guard.clone()), None);
                }
                let settings = JobSettings::for_mirror(&job, chrono::Local::now());
                self.start_and_wait(job.ready(), true, settings, started, sink)
            }
            QueuedJob::Unknown(_) => (
                QueueResult::Failed,
                Some("Needs a newer Secopy.".into()),
                None,
            ),
        }
    }

    /// Starts a queued job unless the queue was cancelled while it was being checked, waits
    /// for it, and says how it ended.
    fn start_and_wait(
        &self,
        ready: Ready,
        verify: bool,
        settings: JobSettings,
        started: QueueEvent,
        sink: &impl QueueSink,
    ) -> (QueueResult, Option<String>, Option<JobHandle>) {
        {
            // A cancel during the checks stops the job before it starts: the check and the
            // start happen under the lock `cancel` takes.
            let run = lock(&self.queue_run);
            if run.cancelled {
                return (QueueResult::Cancelled, Some("Cancelled.".into()), None);
            }
            // The checks passed: the window shows this job's Copying screen from here.
            sink.send(started);
            if let Err(reason) = self
                .jobs
                .start(ready, verify, settings, Forward(sink.clone()))
            {
                return (QueueResult::Failed, Some(reason), None);
            }
        }
        self.jobs.wait();
        let handle = self.jobs.current_handle();
        let summary = handle.as_ref().and_then(JobHandle::summary);
        let (result, reason) = match summary.as_ref().map(|s| s.outcome) {
            Some(JobOutcome::Complete) => (QueueResult::Complete, None),
            Some(JobOutcome::Cancelled) => (QueueResult::Cancelled, Some("Cancelled.".into())),
            Some(JobOutcome::Failures) => {
                (QueueResult::Failed, summary.as_ref().map(failure_reason))
            }
            Some(JobOutcome::Stopped) | None => (
                QueueResult::Failed,
                summary
                    .and_then(|s| s.stopped_because)
                    .or(Some("Stopped.".into())),
            ),
        };
        (result, reason, handle)
    }
}

/// Why a job ended with failures: its failed files, or a mirror's files not removed.
fn failure_reason(s: &SummaryView) -> String {
    let files = |n: u32| if n == 1 { "file" } else { "files" };
    if s.failed > 0 {
        return format!("{} {} failed.", s.failed, files(s.failed));
    }
    if s.unread > 0 {
        let items = if s.unread == 1 { "item" } else { "items" };
        return format!("{} {items} couldn't be read.", s.unread);
    }
    let n = s
        .mirror
        .as_ref()
        .map_or(0, |m| m.removal_failures.len() as u32);
    if n > 0 {
        return format!("{n} {} couldn't be removed.", files(n));
    }
    if let Some(e) = &s.checksum_error {
        return format!("The checksum file couldn't be written: {e}");
    }
    match &s.durability_error {
        Some(e) => format!("The destination couldn't confirm the files are saved: {e}"),
        None => "It didn't complete.".into(),
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
pub fn cancel_job(app: AppHandle, remove_copied: bool) {
    app.state::<AppState>().cancel(remove_copied);
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

impl AppState {
    pub fn mirror_presets(&self) -> Vec<MirrorPreset> {
        lock(&self.mirrors).presets.clone()
    }

    /// Changes the mirror presets and saves them; memory changes only once saved.
    fn change_mirrors<T>(
        &self,
        change: impl FnOnce(&mut MirrorPresets) -> Result<T, String>,
    ) -> Result<Vec<MirrorPreset>, String> {
        let mut presets = lock(&self.mirrors);
        let mut next = presets.clone();
        change(&mut next)?;
        self.store
            .save(MIRRORS, &next)
            .map_err(|e| format!("Couldn't save the mirror: {e}"))?;
        *presets = next;
        Ok(presets.presets.clone())
    }

    pub fn create_mirror_preset(
        &self,
        input: MirrorPresetInput,
    ) -> Result<Vec<MirrorPreset>, String> {
        self.change_mirrors(|m| m.add(input))
    }

    pub fn edit_mirror_preset(
        &self,
        id: &str,
        input: MirrorPresetInput,
    ) -> Result<Vec<MirrorPreset>, String> {
        self.change_mirrors(|m| m.edit(id, input))
    }

    pub fn delete_mirror_preset(&self, id: &str) -> Result<Vec<MirrorPreset>, String> {
        self.change_mirrors(|m| {
            m.delete(id);
            Ok(())
        })
    }

    /// Works out what the preset would do now and keeps it for Run mirror (FR-47).
    pub fn preview_mirror(
        &self,
        id: &str,
        on_compared: &(dyn Fn(u64, u64) + Sync),
    ) -> Result<MirrorPreviewView, String> {
        let preset = lock(&self.mirrors).get(id).cloned().ok_or(PRESET_GONE)?;
        let job = self.plan_mirror(&preset, on_compared)?;
        let plan = &job.plan;
        let sum = |new: bool| {
            let files: Vec<u64> = plan
                .changes
                .iter()
                .filter(|(_, c)| (*c == secopy_core::mirror::Change::New) == new)
                .map(|(i, _)| plan.copy.files[*i].entry.size)
                .collect();
            (count(files.len()), files.iter().sum::<u64>())
        };
        let ((new_files, new_bytes), (changed_files, changed_bytes)) = (sum(true), sum(false));
        let view = MirrorPreviewView {
            preset_id: preset.id.clone(),
            name: preset.name.clone(),
            origin: preset.origin.clone(),
            destination: preset.destination.clone(),
            new_files,
            new_bytes,
            changed_files,
            changed_bytes,
            removed_files: count(plan.removals.len()),
            archive_days: match plan.options.deleted {
                secopy_core::mirror::Deleted::Archive { days } => Some(days),
                secopy_core::mirror::Deleted::Delete => None,
            },
            unchanged: count(plan.copy.files.len() - plan.changes.len()),
            guard: plan.guard.clone(),
        };
        *lock(&self.preview) = Some((preset, job));
        Ok(view)
    }

    /// Rows of the preview, of one kind or all of them.
    pub fn mirror_preview_page(
        &self,
        kind: Option<PreviewKind>,
        offset: u32,
        limit: u32,
    ) -> Vec<PreviewRow> {
        use secopy_core::mirror::Change;
        let preview = lock(&self.preview);
        let Some((_, job)) = preview.as_ref() else {
            return Vec::new();
        };
        let plan = &job.plan;
        let changes = plan.changes.iter().map(|(i, c)| {
            let entry = &plan.copy.files[*i].entry;
            let (kind, reason) = match c {
                Change::New => (PreviewKind::New, "New in the origin"),
                Change::Changed => (PreviewKind::Changed, "Changed in the origin"),
                Change::ContentsDiffer => (PreviewKind::Changed, "Contents differ"),
            };
            PreviewRow {
                path: show(&entry.rel),
                size: entry.size,
                kind,
                reason: reason.into(),
            }
        });
        let removals = plan.removals.iter().map(|rel| PreviewRow {
            path: show(rel),
            size: std::fs::metadata(plan.copy.dest.join(rel)).map_or(0, |m| m.len()),
            kind: PreviewKind::Removed,
            reason: "Deleted in the origin".into(),
        });
        changes
            .chain(removals)
            .filter(|r| kind.is_none_or(|k| r.kind == k))
            .skip(offset as usize)
            .take(limit as usize)
            .collect()
    }

    /// Run mirror: the previewed plan, through the job runner (FR-47).
    /// Runs the preview of preset `id`, as it was previewed; a preview runs once.
    pub fn run_mirror(&self, id: &str, sink: impl ProgressSink) -> Result<(), String> {
        if lock(&self.queue_run).running {
            return Err("A copy or the queue is already running.".into());
        }
        let mut preview = lock(&self.preview);
        let (previewed, job) = match preview.as_ref() {
            Some((preset, job)) if preset.id == id => (preset.clone(), job.clone()),
            _ => return Err("Preview the mirror first.".into()),
        };
        if lock(&self.mirrors).get(id) != Some(&previewed) {
            *preview = None;
            return Err("The mirror changed since its preview. Preview it again.".into());
        }
        let settings = JobSettings::for_mirror(&job, chrono::Local::now());
        self.jobs.start(job.ready(), true, settings, sink)?;
        *preview = None;
        Ok(())
    }

    pub fn add_mirror_to_queue(&self, id: &str) -> Result<QueueView, String> {
        if lock(&self.mirrors).get(id).is_none() {
            return Err(PRESET_GONE.into());
        }
        self.change_queue(|q| {
            q.add_mirror(id);
            true
        })
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
    state.claim_queue_run()?;
    let runner = app.clone();
    let thread = std::thread::spawn(move || {
        runner.state::<AppState>().run_claimed(on_event);
    });
    // Only one run is claimed at a time, so this is the previous (finished) run's thread.
    let old = lock(&state.queue_run).thread.replace(thread);
    if let Some(old) = old {
        let _ = old.join();
    }
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

#[tauri::command]
#[specta::specta]
pub async fn mirror_presets(app: AppHandle) -> Result<Vec<MirrorPreset>, String> {
    blocking(app, |state| state.mirror_presets()).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_mirror_preset(
    app: AppHandle,
    input: MirrorPresetInput,
) -> Result<Vec<MirrorPreset>, String> {
    blocking(app, move |state| state.create_mirror_preset(input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn edit_mirror_preset(
    app: AppHandle,
    id: String,
    input: MirrorPresetInput,
) -> Result<Vec<MirrorPreset>, String> {
    blocking(app, move |state| state.edit_mirror_preset(&id, input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn delete_mirror_preset(app: AppHandle, id: String) -> Result<Vec<MirrorPreset>, String> {
    blocking(app, move |state| state.delete_mirror_preset(&id)).await?
}

/// A mirror's preview (FR-47); Run mirror then runs it.
#[tauri::command]
#[specta::specta]
pub async fn preview_mirror(
    app: AppHandle,
    id: String,
    on_compared: Channel<ComparedView>,
) -> Result<MirrorPreviewView, String> {
    blocking(app, move |state| {
        state.preview_mirror(&id, &|done, total| {
            let _ = on_compared.send(ComparedView {
                done: count(done),
                total: count(total),
            });
        })
    })
    .await?
}

/// Stops a preview's deep check.
#[tauri::command]
#[specta::specta]
pub fn cancel_mirror_preview(app: AppHandle) {
    app.state::<AppState>().cancel_preview();
}

#[tauri::command]
#[specta::specta]
pub async fn mirror_preview_page(
    app: AppHandle,
    kind: Option<PreviewKind>,
    offset: u32,
    limit: u32,
) -> Result<Vec<PreviewRow>, String> {
    blocking(app, move |state| {
        state.mirror_preview_page(kind, offset, limit)
    })
    .await
}

/// Runs the previewed mirror; progress arrives on `on_progress`.
#[tauri::command]
#[specta::specta]
pub async fn run_mirror(
    app: AppHandle,
    id: String,
    on_progress: Channel<ProgressView>,
) -> Result<(), String> {
    blocking(app, move |state| state.run_mirror(&id, on_progress)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn add_mirror_to_queue(app: AppHandle, id: String) -> Result<QueueView, String> {
    blocking(app, move |state| state.add_mirror_to_queue(&id)).await?
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
        let kinds: Vec<String> = events
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                QueueEvent::JobChecking { index, count } => Some(format!("check {index}/{count}")),
                QueueEvent::JobStarted { index, count, job } => {
                    Some(format!("start {index}/{count} {}", job.kind))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            ["check 0/2", "start 0/2 copy", "check 1/2", "start 1/2 copy"],
            "each job is checked, then started"
        );
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

    /// #58: a queued job whose source couldn't all be read fails, stays queued and says why:
    /// nobody was watching when it was scanned.
    #[cfg(unix)]
    #[test]
    fn a_queued_job_with_unreadable_items_fails() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        let locked = dir.path().join("A/locked");
        fs::create_dir_all(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let summary = state.run_queue(Events::default()).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(summary.results[0].result, QueueResult::Failed);
        assert_eq!(
            summary.results[0].reason.as_deref(),
            Some("1 item couldn't be read.")
        );
        assert_eq!(state.queue_view().jobs.len(), 1, "it stays queued");
    }

    /// #57: a job that can't start never shows a Copying screen.
    #[test]
    fn a_job_that_cant_start_is_never_started() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        fs::remove_dir_all(dir.path().join("A")).unwrap();
        let events = Events::default();
        state.run_queue(events.clone()).unwrap();
        let events = events.0.lock().unwrap();
        assert!(matches!(
            events[0],
            QueueEvent::JobChecking { index: 0, .. }
        ));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, QueueEvent::JobStarted { .. })),
            "no Copying screen for it"
        );
    }

    /// #57: a panic mid-run doesn't leave the queue "running" until restart.
    #[test]
    fn a_panic_mid_run_doesnt_leave_the_queue_running() {
        #[derive(Clone)]
        struct Panics;
        impl QueueSink for Panics {
            fn send(&self, _: QueueEvent) {
                panic!("the window went away badly");
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        let ran =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.run_queue(Panics)));
        assert!(ran.is_err());
        assert!(!state.busy());
    }

    /// #57: the run is claimed before its thread starts, so two can't both start.
    #[test]
    fn a_run_is_claimed_before_it_starts() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        state.claim_queue_run().unwrap();
        assert_eq!(
            state.claim_queue_run().unwrap_err(),
            "A copy or the queue is already running."
        );
        let summary = state.run_claimed(Events::default());
        assert_eq!(summary.complete, 1);
        assert!(!state.busy());
    }

    /// #57: completed jobs leave the queue even when it can't be saved.
    #[cfg(unix)]
    #[test]
    fn a_queue_that_cant_be_saved_still_forgets_completed_jobs() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        let data = dir.path().join("data");
        fs::create_dir_all(&data).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o555)).unwrap();
        let summary = state.run_queue(Events::default()).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            state.queue_view().jobs.is_empty(),
            "no duplicate on a re-run"
        );
        let error = summary.save_error.expect("says the queue wasn't saved");
        assert!(error.starts_with("Couldn't save the queue"), "{error}");
    }

    /// #57: the queue can't be changed while it runs.
    #[test]
    fn the_queue_cant_be_changed_while_it_runs() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        lock(&state.queue_run).running = true;
        assert_eq!(
            state.change_queue(|q| q.remove(0)).unwrap_err(),
            "The queue is running."
        );
        lock(&state.queue_run).running = false;
        assert_eq!(state.queue_view().jobs.len(), 1);
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
                job: QueuedJob::Unknown(serde_json::json!({"kind": "sync", "preset": "p"})),
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
        for e in &mut lock(&state.queue).jobs {
            e.last_error = Some("an old reason".into());
        }
        state.cancel(false); // before the run: flag is reset by the run, so cancel during it
        std::thread::scope(|s| {
            let run = s.spawn(|| state.run_queue(Events::default()).unwrap());
            // Cancel as soon as the first job is running.
            while !state.jobs.is_running() && !run.is_finished() {
                std::thread::yield_now();
            }
            state.cancel(false);
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
        assert_eq!(left[0].last_error.as_deref(), Some("Stopped."));
        assert_eq!(left[1].last_error, None, "no stale reason on a job not run");
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
        let (result, _, handle) = state.run_one(&entry, (0, 1), &Events::default());
        assert_eq!(result, QueueResult::Cancelled);
        assert!(handle.is_none());
        assert!(!dir.path().join("dest/A").exists(), "nothing copied");
        lock(&state.queue_run).running = false;
    }
    fn mirror_state(dir: &Path) -> (AppState, String, PathBuf, PathBuf) {
        let state = AppState::new(dir.join("data"));
        let (o, d) = (dir.join("o"), dir.join("d"));
        fs::create_dir_all(&o).unwrap();
        fs::create_dir_all(&d).unwrap();
        fs::write(o.join("a.mov"), b"a").unwrap();
        fs::write(d.join("x.mov"), b"x").unwrap();
        fs::write(d.join("y.mov"), b"y").unwrap();
        fs::write(d.join("z.mov"), b"z").unwrap();
        let presets = state
            .create_mirror_preset(crate::store::MirrorPresetInput {
                name: "Footage".into(),
                origin: show(&o),
                destination: show(&d),
                deleted: crate::store::DeletedFiles {
                    mode: crate::store::DeletedMode::Archive,
                    days: 30,
                },
                deep_check: false,
            })
            .unwrap();
        (state, presets[0].id.clone(), o, d)
    }

    #[test]
    fn preview_counts_and_lists_the_changes() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, ..) = mirror_state(dir.path());
        let p = state.preview_mirror(&id, &|_, _| {}).unwrap();
        assert_eq!((p.new_files, p.removed_files, p.unchanged), (1, 3, 0));
        assert_eq!(
            p.guard.as_deref(),
            Some("3 of the destination's 3 files would be removed.")
        );
        let removed = state.mirror_preview_page(Some(PreviewKind::Removed), 0, 10);
        assert_eq!(removed.len(), 3);
    }

    /// #57: Run mirror runs the preview of that preset, as it was previewed, once.
    #[test]
    fn run_mirror_runs_only_the_preview_of_that_preset() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, ..) = mirror_state(dir.path());
        state.preview_mirror(&id, &|_, _| {}).unwrap();
        assert_eq!(
            state.run_mirror("another", Sink::default()).unwrap_err(),
            "Preview the mirror first."
        );
        let mut preset = state.mirror_presets()[0].clone();
        preset.name = "Renamed".into();
        state
            .edit_mirror_preset(
                &id,
                crate::store::MirrorPresetInput {
                    name: preset.name,
                    origin: preset.origin,
                    destination: preset.destination,
                    deleted: preset.deleted,
                    deep_check: preset.deep_check,
                },
            )
            .unwrap();
        assert_eq!(
            state.run_mirror(&id, Sink::default()).unwrap_err(),
            "The mirror changed since its preview. Preview it again."
        );
        state.preview_mirror(&id, &|_, _| {}).unwrap();
        state.run_mirror(&id, Sink::default()).unwrap();
        state.jobs.wait();
        assert_eq!(
            state.run_mirror(&id, Sink::default()).unwrap_err(),
            "Preview the mirror first.",
            "a preview runs once"
        );
    }

    /// #57: the deep check of a preview can be cancelled, also by Cancel of the queue.
    #[test]
    fn a_deep_check_can_be_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, o, d) = mirror_state(dir.path());
        fs::copy(o.join("a.mov"), d.join("a.mov")).unwrap();
        let t = fs::metadata(o.join("a.mov")).unwrap().modified().unwrap();
        fs::File::options()
            .write(true)
            .open(d.join("a.mov"))
            .unwrap()
            .set_modified(t)
            .unwrap();
        let mut preset = state.mirror_presets()[0].clone();
        preset.deep_check = true;
        state
            .edit_mirror_preset(
                &id,
                crate::store::MirrorPresetInput {
                    name: preset.name,
                    origin: preset.origin,
                    destination: preset.destination,
                    deleted: preset.deleted,
                    deep_check: true,
                },
            )
            .unwrap();
        let seen = StdMutex::new(Vec::new());
        let err = state
            .preview_mirror(&id, &|done, total| {
                seen.lock().unwrap().push((done, total));
                if done == 0 {
                    state.cancel_preview();
                }
            })
            .unwrap_err();
        assert_eq!(err, "Cancelled.");
        assert_eq!(seen.lock().unwrap()[0], (0, 1));
        let err = state
            .preview_mirror(&id, &|done, _| {
                if done == 0 {
                    state.cancel(false);
                }
            })
            .unwrap_err();
        assert_eq!(err, "Cancelled.");
    }

    /// A mirror state whose preset compares contents, with one unchanged file to compare, and
    /// a guard that lets it run.
    fn deep_mirror_state(dir: &Path) -> (AppState, String) {
        let (state, id, o, d) = mirror_state(dir);
        for f in ["a.mov", "x.mov", "y.mov"] {
            fs::write(o.join(f), f).unwrap();
            fs::write(d.join(f), f).unwrap();
            let t = fs::metadata(o.join(f)).unwrap().modified().unwrap();
            fs::File::options()
                .write(true)
                .open(d.join(f))
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        let p = state.mirror_presets()[0].clone();
        state
            .edit_mirror_preset(
                &id,
                crate::store::MirrorPresetInput {
                    name: p.name,
                    origin: p.origin,
                    destination: p.destination,
                    deleted: p.deleted,
                    deep_check: true,
                },
            )
            .unwrap();
        (state, id)
    }

    /// #57 review: a queued mirror's deep check shows how far it is, and Cancel there reads
    /// as cancelled, not failed.
    #[test]
    fn a_queued_deep_check_reports_progress_and_cancels_as_cancelled() {
        #[derive(Clone)]
        struct CancelOnCompare(Events, Arc<AppState>);
        impl QueueSink for CancelOnCompare {
            fn send(&self, e: QueueEvent) {
                if matches!(e, QueueEvent::Compared { done: 0, .. }) {
                    self.1.cancel(false);
                }
                self.0.send(e);
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let (state, id) = deep_mirror_state(dir.path());
        state.add_mirror_to_queue(&id).unwrap();
        let state = Arc::new(state);
        let events = Events::default();
        let summary = state
            .run_queue(CancelOnCompare(events.clone(), state.clone()))
            .unwrap();
        assert_eq!(summary.results[0].result, QueueResult::Cancelled);
        assert_eq!(
            state.queue_view().jobs[0].last_error.as_deref(),
            Some("Stopped.")
        );
        assert!(events.0.lock().unwrap().iter().any(|e| matches!(
            e,
            QueueEvent::Compared {
                index: 0,
                done: 0,
                total: 3
            }
        )));
    }

    /// #57 review: a job cancelled while it was checked never shows a Copying screen.
    #[test]
    fn a_job_cancelled_while_checked_is_never_started() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        let entry = lock(&state.queue).jobs[0].clone();
        lock(&state.queue_run).cancelled = true;
        let events = Events::default();
        let (result, ..) = state.run_one(&entry, (0, 1), &events);
        assert_eq!(result, QueueResult::Cancelled);
        assert!(events.0.lock().unwrap().is_empty(), "no JobStarted");
    }

    /// #57 review: a second plan finishing doesn't take Cancel away from the first.
    #[test]
    fn cancel_reaches_every_mirror_being_planned() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id) = deep_mirror_state(dir.path());
        let err = state
            .preview_mirror(&id, &|done, _| {
                if done == 0 {
                    // Another preview, start to end, while this one runs.
                    state.preview_mirror(&id, &|_, _| {}).unwrap();
                    state.cancel_preview();
                }
            })
            .unwrap_err();
        assert_eq!(err, "Cancelled.");
    }

    /// Review focus 4.
    #[test]
    fn a_tripped_guard_fails_a_queued_mirror() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        state.add_mirror_to_queue(&id).unwrap();
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(summary.results[0].result, QueueResult::Failed);
        assert_eq!(
            summary.results[0].reason.as_deref(),
            Some("3 of the destination's 3 files would be removed.")
        );
        assert!(d.join("x.mov").exists(), "nothing removed");
    }

    #[test]
    fn a_queued_mirror_runs_like_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, o, d) = mirror_state(dir.path());
        fs::write(o.join("x.mov"), b"x").unwrap(); // the guard now allows it: 2 of 3
        fs::write(o.join("y.mov"), b"y").unwrap();
        state.add_mirror_to_queue(&id).unwrap();
        assert_eq!(state.queue_view().jobs[0].name.as_deref(), Some("Footage"));
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(summary.results[0].result, QueueResult::Complete);
        assert!(!d.join("z.mov").exists() && d.join("a.mov").exists());
    }

    #[test]
    fn a_queued_mirror_whose_preset_is_gone_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, ..) = mirror_state(dir.path());
        state.add_mirror_to_queue(&id).unwrap();
        state.delete_mirror_preset(&id).unwrap();
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(
            summary.results[0].reason.as_deref(),
            Some("The mirror preset no longer exists.")
        );
    }
}
