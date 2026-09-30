//! The commands the UI calls (see `ui/src/lib/bindings.ts`, generated from these). Thin
//! wrappers: the work is in `session` and `jobs`, run off the main thread so the window
//! never freezes (NFR-5).

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::lock;
use crate::message::Message;
use crate::msg;
use crate::say;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::dto::{ArchiveDeletedView, ArchiveView, ExportWhat, ImportDone};
use crate::dto::{CheckView, ComparedView, MirrorPreviewView, PreviewKind, PreviewRow, count};
use crate::dto::{
    ConflictPolicy, CopyPresetsView, ExtensionKey, FinishedRow, JobOutcome, ProgressView,
    QueueEvent, QueueResult, QueueResultView, QueueSummaryView, QueueView, QueuedJobView,
    SessionView, StartView, SummaryView, show,
};
use crate::jobs::{JobHandle, JobSettings, Jobs, ProgressSink, Work};
use crate::mirrors::MirrorJob;
use crate::queue::{Entry, OnFailure, QUEUE, Queue, QueuedJob};
use crate::session::{Change, Session, scan_source as scan};
use crate::store::{
    COPY_PRESETS, CopyPreset, CopyPresetInput, CopyPresets, MIRRORS, MirrorPreset,
    MirrorPresetInput, MirrorPresets, REMEMBERED, Remembered, SETTINGS, Settings, Store,
    WindowSize,
};
#[cfg(test)]
use crate::transfer::PresetChoice;
use crate::transfer::{self, Contents, ImportChoices, ImportView};
use secopy_core::check::CheckPlan;
use secopy_core::control::JobControl;

/// Everything the app keeps between commands.
pub struct AppState {
    pub session: Mutex<Session>,
    pub jobs: Jobs,
    pub store: Store,
    pub settings: Mutex<Settings>,
    pub copy_presets: Mutex<CopyPresets>,
    pub remembered: Mutex<Remembered>,
    /// Saved files that couldn't be read, handed to the UI once.
    warnings: Mutex<Vec<Message>>,
    /// The saved queue (plan 6).
    pub(crate) queue: Mutex<Queue>,
    /// Saved mirror presets (plan 7).
    pub(crate) mirrors: Mutex<MirrorPresets>,
    /// The mirror last previewed, with its preset as it was: the preview's Start runs exactly this, once
    /// (FR-47).
    preview: Mutex<Option<(MirrorPreset, MirrorJob)>>,
    /// Stops a mirror being planned (its deep check), for Preview's Cancel and the queue's.
    planning: Mutex<Vec<Arc<JobControl>>>,
    /// The directory last chosen on Verify, planned: Verify's Start runs exactly this, once.
    checking: Mutex<Option<(PathBuf, Arc<CheckPlan>)>>,
    pub(crate) queue_run: Mutex<QueueRun>,
    /// The file on the Import screen, as read when it opened: Import applies exactly this.
    importing: Mutex<Option<Pending>>,
    /// A `.secopy` file opened from Finder that the window hasn't shown yet.
    opened: Mutex<Option<PathBuf>>,
    /// ⌘Q or the menu bar's Quit: the next close asks instead of hiding (#80).
    pub quitting: std::sync::atomic::AtomicBool,
}

/// The queue run in progress, and the jobs of the last one.
#[derive(Default)]
pub(crate) struct QueueRun {
    pub running: bool,
    pub cancelled: bool,
    pub handles: Vec<Option<JobHandle>>,
    /// Each job's result, recorded as soon as it ends, for a run that panics (#69).
    pub done: Vec<(QueueResult, Option<Message>)>,
    /// The job after the last in `done` was started.
    pub job_started: bool,
    /// The thread running the queue, joined at quit.
    pub thread: Option<std::thread::JoinHandle<()>>,
}

impl AppState {
    /// Loads what was saved in `data_dir`; reports go to `data_dir/reports`.
    pub fn new(data_dir: PathBuf) -> Self {
        let store = Store::new(data_dir.clone());
        let (settings, w1) = store.load::<Settings>(SETTINGS);
        let (loaded, w2) = store.load::<CopyPresets>(COPY_PRESETS);
        let (copy_presets, repairs) = loaded.clone().repaired();
        // Saved as put right, so its messages show once.
        if copy_presets != loaded
            && let Err(e) = store.save(COPY_PRESETS, &copy_presets)
        {
            eprintln!("Secopy: couldn't save {COPY_PRESETS}: {e:?}");
        }
        let (remembered, w3) = store.load::<Remembered>(REMEMBERED);
        let (queue, w4) = store.load::<Queue>(QUEUE);
        let (mirrors, w5) = store.load::<MirrorPresets>(MIRRORS);
        Self {
            session: Mutex::new(Session::new()),
            jobs: Jobs::new(data_dir.join("reports")),
            store,
            settings: Mutex::new(settings),
            copy_presets: Mutex::new(copy_presets),
            remembered: Mutex::new(remembered),
            warnings: Mutex::new(
                [w1, w2, w3, w4, w5]
                    .into_iter()
                    .flatten()
                    .chain(repairs)
                    .collect(),
            ),
            mirrors: Mutex::new(mirrors),
            preview: Mutex::new(None),
            planning: Mutex::new(Vec::new()),
            checking: Mutex::new(None),
            queue: Mutex::new(queue),
            queue_run: Mutex::new(QueueRun::default()),
            importing: Mutex::new(None),
            opened: Mutex::new(None),
            quitting: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn start_view(&self) -> StartView {
        // One lock at a time: a guard in a struct literal lives to the end of it.
        let session = session(self).view();
        let settings = lock(&self.settings).clone();
        let copy_presets = lock(&self.copy_presets).presets.clone();
        let verify = lock(&self.remembered).verify;
        let warnings = std::mem::take(&mut *lock(&self.warnings));
        // Loaded again only when its source is there: no error at launch for a card that
        // isn't inserted.
        let last = lock(&self.remembered).last_preset.clone();
        let last_preset = last.filter(|id| {
            copy_presets
                .iter()
                .any(|p| &p.id == id && (p.source.is_empty() || Path::new(&p.source).exists()))
        });
        StartView {
            session,
            settings,
            copy_presets,
            verify,
            recent_destinations: self.recent(),
            warnings,
            last_preset,
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
            eprintln!("Secopy: couldn't save {REMEMBERED}: {e:?}");
        }
    }

    /// Kept in memory; written when the app quits.
    pub fn window_resized(&self, size: WindowSize) {
        lock(&self.remembered).window = Some(size);
    }

    pub fn saved_window(&self) -> Option<WindowSize> {
        lock(&self.remembered).window
    }

    pub fn select_copy_preset(&self, id: Option<String>) -> Result<SessionView, Message> {
        let preset = match &id {
            Some(id) => Some(
                lock(&self.copy_presets)
                    .get(id)
                    .cloned()
                    .ok_or_else(|| msg!("errors.preset.gone"))?,
            ),
            None => None,
        };
        self.remember(|r| r.last_preset = id);
        Ok(self.rescan(Change::CopyPreset(preset)))
    }

    /// Changes the copy presets and saves them, holding their lock throughout so saves at the
    /// same time can't undo each other; memory changes only when the file is written.
    fn change_copy_presets<T>(
        &self,
        change: impl FnOnce(&mut CopyPresets) -> Result<T, Message>,
    ) -> Result<T, Message> {
        let mut presets = lock(&self.copy_presets);
        let mut next = presets.clone();
        let result = change(&mut next)?;
        self.store
            .save(COPY_PRESETS, &next)
            .map_err(|e| msg!("errors.save.preset", why = e))?;
        *presets = next;
        Ok(result)
    }

    fn copy_presets_view(&self, session: SessionView) -> CopyPresetsView {
        CopyPresetsView {
            presets: lock(&self.copy_presets).presets.clone(),
            session,
        }
    }

    /// Update: this run's choices go into the selected copy preset.
    pub fn update_copy_preset(&self) -> Result<CopyPresetsView, Message> {
        let updated = {
            let s = session(self);
            if s.scan_pending() {
                return Err(msg!("errors.preset.scanning"));
            }
            s.updated_preset()
                .ok_or_else(|| msg!("errors.preset.noneSelected"))?
        };
        self.change_copy_presets(|p| {
            p.replace(updated.clone());
            Ok(())
        })?;
        let view = session(self).preset_saved(updated);
        Ok(self.copy_presets_view(view))
    }

    /// Save as…: this run's source and choices under a new name, then selected.
    pub fn save_copy_preset_as(&self, name: String) -> Result<CopyPresetsView, Message> {
        let (source, (include_folder, extensions)) = {
            let s = session(self);
            let choices = s.choices().ok_or_else(|| msg!("errors.preset.scanning"))?;
            let source = s
                .picked_source()
                .ok_or_else(|| msg!("errors.preset.needsDirectory"))?;
            (source, choices)
        };
        let preset = self.change_copy_presets(|p| {
            p.add(CopyPresetInput {
                name,
                source,
                include_folder,
                extensions,
            })
        })?;
        self.remember(|r| r.last_preset = Some(preset.id.clone()));
        let view = self.rescan(Change::CopyPreset(Some(preset)));
        Ok(self.copy_presets_view(view))
    }

    /// Copy presets → New.
    pub fn create_copy_preset(&self, input: CopyPresetInput) -> Result<Vec<CopyPreset>, Message> {
        self.change_copy_presets(|p| p.add(input))?;
        Ok(lock(&self.copy_presets).presets.clone())
    }

    /// Copy presets → Edit; the selected preset is applied again.
    pub fn edit_copy_preset(
        &self,
        id: &str,
        input: CopyPresetInput,
    ) -> Result<CopyPresetsView, Message> {
        let edited = self.change_copy_presets(|p| p.edit(id, input))?;
        let selected = session(self).preset().is_some_and(|p| p.id == id);
        let view = if selected {
            self.rescan(Change::CopyPreset(Some(edited)))
        } else {
            session(self).view()
        };
        Ok(self.copy_presets_view(view))
    }

    /// Copy presets → Delete; a selected preset becomes None.
    pub fn delete_copy_preset(&self, id: &str) -> Result<CopyPresetsView, Message> {
        self.change_copy_presets(|p| {
            p.delete(id);
            Ok(())
        })?;
        let selected = session(self).preset().is_some_and(|p| p.id == id);
        let view = if selected {
            self.remember(|r| r.last_preset = None);
            self.rescan(Change::CopyPreset(None))
        } else {
            session(self).view()
        };
        Ok(self.copy_presets_view(view))
    }

    /// Saves, then applies (the next job uses it): settings that can't be saved aren't used
    /// either (#116).
    pub fn set_settings(&self, settings: Settings) -> Result<Settings, Message> {
        // Held across the save, so the last change in memory is also the last one on disk.
        let mut current = lock(&self.settings);
        self.store
            .save(SETTINGS, &settings)
            .map_err(|e| msg!("errors.save.settings", why = e))?;
        *current = settings.clone();
        Ok(settings)
    }

    /// New copy as it is now: after Start refused because the destination changed.
    pub fn session_view(&self) -> SessionView {
        session(self).view()
    }

    /// Starts the job with the current settings and remembers the destination (B7).
    pub fn start(&self, verify: bool, sink: impl ProgressSink) -> Result<(), Message> {
        if lock(&self.queue_run).running {
            return Err(msg!("errors.queue.running"));
        }
        let (ready, dest) = {
            let mut s = session(self);
            if !s.still_as_shown() {
                return Err(msg!("errors.job.changed"));
            }
            let ready = s.ready().ok_or_else(|| msg!("errors.job.cantStart"))?;
            (ready, s.destination().map(show))
        };
        let settings = JobSettings::from(&*lock(&self.settings));
        self.jobs.start(ready, verify, settings, sink)?;
        if let Some(dest) = dest {
            self.remember(|r| r.used_destination(&dest));
        }
        Ok(())
    }

    /// "Retry": the failed files of the last job become the source.
    pub fn retry_failed(&self) -> Result<SessionView, Message> {
        let (source, selection) = self
            .jobs
            .retry()
            .ok_or_else(|| msg!("errors.job.noneFailed"))?;
        // With the source's drive pulled out: say so instead of failing every file.
        if let Some(path) = crate::jobs::source_path(&source)
            && !path.exists()
        {
            return Err(msg!("errors.job.retrySourceGone", path = &path));
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
                [one] => Message::raw(show(one)),
                many => msg!("copy.picked.files", count = many.len()),
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
                source: Message::raw(p.origin.clone()),
                destination: p.destination.clone(),
                last_error: entry.last_error.clone(),
                supported: true,
                name: Some(p.name.clone()),
            },
            None => QueuedJobView {
                kind: "mirror".into(),
                verify: true,
                source: Message::raw(""),
                destination: String::new(),
                last_error: Some(preset_gone()),
                supported: false,
                name: None,
            },
        },
        QueuedJob::Check { directory } => QueuedJobView {
            kind: "check".into(),
            verify: true,
            source: Message::raw(show(directory)),
            destination: String::new(),
            last_error: entry.last_error.clone(),
            supported: true,
            name: None,
        },
        QueuedJob::Unknown(_) => QueuedJobView {
            kind: "unknown".into(),
            verify: false,
            source: Message::raw(""),
            destination: String::new(),
            last_error: Some(msg!("queue.reason.newer")),
            supported: false,
            name: None,
        },
    }
}

fn preset_gone() -> Message {
    msg!("errors.mirror.presetGone")
}

/// Why the jobs left in a queue whose thread panicked didn't run (#69).
fn queue_stopped() -> Message {
    msg!("queue.reason.internal")
}

impl AppState {
    pub fn queue_view(&self) -> QueueView {
        let q = lock(&self.queue);
        QueueView {
            jobs: {
                let mirrors = lock(&self.mirrors);
                q.jobs.iter().map(|e| job_view(e, &mirrors)).collect()
            },
            on_failure: q.on_failure,
        }
    }

    /// Changes the queue and saves it; `change` returns false for a bad index.
    pub fn change_queue(
        &self,
        change: impl FnOnce(&mut Queue) -> bool,
    ) -> Result<QueueView, Message> {
        // The run saves what's left after each job; a change now would be lost or duplicated.
        if lock(&self.queue_run).running {
            return Err(msg!("errors.queue.running"));
        }
        {
            let mut q = lock(&self.queue);
            let mut next = q.clone();
            if !change(&mut next) {
                return Err(msg!("errors.queue.jobGone"));
            }
            self.store
                .save(QUEUE, &next)
                .map_err(|e| msg!("errors.save.queue", why = e))?;
            *q = next;
        }
        Ok(self.queue_view())
    }

    pub fn add_to_queue(&self, verify: bool) -> Result<QueueView, Message> {
        let job = session(self)
            .copy_job(verify)
            .ok_or_else(|| msg!("errors.queue.setUpFirst"))?;
        self.change_queue(|q| {
            q.add(job);
            true
        })
    }

    /// A copy or the queue is running.
    pub fn busy(&self) -> bool {
        self.jobs.is_running() || lock(&self.queue_run).running
    }

    /// Whether closing the window should hide it behind the menu bar icon (#80). Takes the
    /// quit flag: a quit asks once.
    pub fn wants_hide(&self) -> bool {
        let quitting = self
            .quitting
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        let keep = lock(&self.settings).keep_in_menu_bar;
        crate::menubar::should_hide(keep, self.busy(), quitting)
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
    ) -> Result<MirrorJob, Message> {
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
    pub fn run_queue(&self, sink: impl QueueSink) -> Result<QueueSummaryView, Message> {
        self.claim_queue_run()?;
        Ok(self.run_claimed(sink))
    }

    /// Marks the queue as running, so starting it again is refused before any thread starts.
    pub fn claim_queue_run(&self) -> Result<(), Message> {
        let mut run = lock(&self.queue_run);
        if run.running || self.jobs.is_running() {
            return Err(msg!("errors.queue.busy"));
        }
        run.running = true;
        run.cancelled = false;
        run.handles.clear();
        run.done.clear();
        run.job_started = false;
        Ok(())
    }

    /// The queue's thread: runs the claimed queue, and however that ends, a panic included,
    /// the window gets its `Done` (#69).
    pub fn run_claimed_to_done(&self, sink: impl QueueSink) {
        let clock = std::time::Instant::now();
        let entries = lock(&self.queue).jobs.clone();
        let ran = std::panic::catch_unwind(AssertUnwindSafe(|| self.run_claimed(sink.clone())));
        if ran.is_ok() {
            return;
        }
        let summary =
            std::panic::catch_unwind(AssertUnwindSafe(|| self.after_panic(&entries, clock)))
                .unwrap_or_else(|_| {
                    // The recovery hit the same bug: only what is recorded.
                    let done = lock(&self.queue_run).done.clone();
                    QueueSummaryView {
                        complete: done.iter().filter(|d| d.0 == QueueResult::Complete).count()
                            as u32,
                        count: entries.len() as u32,
                        millis: clock.elapsed().as_millis() as u64,
                        results: Vec::new(),
                        save_error: Some(queue_stopped()),
                    }
                });
        sink.send(QueueEvent::Done { summary });
    }

    /// The summary of a queue run that panicked: the results recorded, the job that was
    /// running as its own summary says, the rest failed; the queue keeps all but the
    /// complete ones, so none runs twice.
    fn after_panic(&self, entries: &[Entry], clock: std::time::Instant) -> QueueSummaryView {
        let (done, job_started, handles) = {
            let run = lock(&self.queue_run);
            (run.done.clone(), run.job_started, run.handles.clone())
        };
        let mut results = Vec::new();
        let mut keep = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let running = index == done.len() && job_started;
            let handle = match handles.get(index) {
                Some(handle) => handle.clone(),
                None if running => self.jobs.current_handle(),
                None => None,
            };
            let summary = handle.as_ref().and_then(JobHandle::summary);
            let (result, reason) = match done.get(index) {
                Some(recorded) => recorded.clone(),
                None if running
                    && summary.as_ref().map(|s| s.outcome) == Some(JobOutcome::Complete) =>
                {
                    (QueueResult::Complete, None)
                }
                None => (QueueResult::Failed, Some(queue_stopped())),
            };
            if result != QueueResult::Complete {
                let last_error = match result {
                    QueueResult::Failed => reason.clone(),
                    QueueResult::Cancelled => Some(msg!("queue.reason.stopped")),
                    _ => None,
                };
                keep.push(Entry {
                    last_error,
                    ..entry.clone()
                });
            }
            results.push(QueueResultView {
                job: job_view(entry, &lock(&self.mirrors)),
                result,
                reason,
                summary,
            });
        }
        let save_error = self.leave_in_queue(keep).err();
        QueueSummaryView {
            complete: count_of(&results, QueueResult::Complete),
            count: entries.len() as u32,
            millis: clock.elapsed().as_millis() as u64,
            results,
            save_error,
        }
    }

    /// Runs the queue claimed by `claim_queue_run`; it is released however this ends.
    pub fn run_claimed(&self, sink: impl QueueSink) -> QueueSummaryView {
        /// Releases the run on the way out, a panic included.
        struct Release<'a>(&'a Mutex<QueueRun>);
        impl Drop for Release<'_> {
            fn drop(&mut self) {
                lock(self.0).running = false;
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
                {
                    let mut run = lock(&self.queue_run);
                    run.handles.push(None);
                    run.done.push((QueueResult::NotRun, None));
                }
                results.push(QueueResultView {
                    job: job_view(entry, &lock(&self.mirrors)),
                    result: QueueResult::NotRun,
                    reason: Some(msg!("queue.reason.notRun")),
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
            {
                let mut run = lock(&self.queue_run);
                run.done.push((result, reason.clone()));
                run.job_started = false;
                run.handles.push(handle.clone());
            }
            let failed = matches!(result, QueueResult::Failed);
            if result != QueueResult::Complete {
                keep.push(Entry {
                    last_error: if failed {
                        reason.clone()
                    } else {
                        Some(msg!("queue.reason.stopped"))
                    },
                    ..entry.clone()
                });
            }
            stop |= result == QueueResult::Cancelled || (failed && on_failure == OnFailure::Stop);
            // Saved after every job, first thing: completed ones gone, the rest kept (Review
            // focus 3).
            let remaining: Vec<Entry> = keep
                .iter()
                .cloned()
                .chain(entries[index + 1..].iter().cloned())
                .collect();
            if let Err(e) = self.leave_in_queue(remaining) {
                save_error = Some(e);
            }
            results.push(QueueResultView {
                job: job_view(entry, &lock(&self.mirrors)),
                result,
                reason,
                summary: handle.as_ref().and_then(JobHandle::summary),
            });
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
    fn leave_in_queue(&self, jobs: Vec<Entry>) -> Result<(), Message> {
        let mut q = lock(&self.queue);
        q.jobs = jobs;
        self.store
            .save(QUEUE, &*q)
            .map_err(|e| msg!("errors.save.queue", why = e))
    }

    fn run_one(
        &self,
        entry: &Entry,
        (index, jobs): (u32, u32),
        sink: &impl QueueSink,
    ) -> (QueueResult, Option<Message>, Option<JobHandle>) {
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
                let work = Work::Copy {
                    ready: Box::new(ready),
                    verify: job.verify,
                    settings,
                };
                let ran = self.start_and_wait(work, started, sink, None);
                if ran.2.is_some() {
                    self.remember(|r| r.used_destination(&show(&job.destination)));
                }
                ran
            }
            QueuedJob::Mirror { preset } => {
                let Some(preset) = lock(&self.mirrors).get(preset).cloned() else {
                    return (QueueResult::Failed, Some(preset_gone()), None);
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
                        return (
                            QueueResult::Cancelled,
                            Some(msg!("queue.reason.cancelled")),
                            None,
                        );
                    }
                    Err(reason) => return (QueueResult::Failed, Some(reason), None),
                };
                // Nobody is there to confirm: a run that looks wrong doesn't start (FR-50).
                if let Some(guard) = &job.plan.guard {
                    return (QueueResult::Failed, Some(say::guard(guard)), None);
                }
                let settings = JobSettings::for_mirror(&job, chrono::Local::now());
                let work = Work::Copy {
                    ready: Box::new(job.ready()),
                    verify: true,
                    settings,
                };
                self.start_and_wait(work, started, sink, Some(&preset.id))
            }
            QueuedJob::Check { directory } => {
                let plan = match plan_check(directory) {
                    Ok(plan) => plan,
                    Err(reason) => return (QueueResult::Failed, Some(reason), None),
                };
                if plan.files.is_empty() {
                    return (QueueResult::Failed, Some(nothing_to_verify()), None);
                }
                self.start_and_wait(Work::Check(Arc::new(plan)), started, sink, None)
            }
            QueuedJob::Unknown(_) => (QueueResult::Failed, Some(msg!("queue.reason.newer")), None),
        }
    }

    /// Starts a queued job unless the queue was cancelled while it was being checked, waits
    /// for it, and says how it ended.
    /// `mirror`: the preset of a mirror job, whose pending archive deletion (#101) happens
    /// here, after the cancel check and under its lock: a cancelled job deletes nothing.
    fn start_and_wait(
        &self,
        mut work: Work,
        started: QueueEvent,
        sink: &impl QueueSink,
        mirror: Option<&str>,
    ) -> (QueueResult, Option<Message>, Option<JobHandle>) {
        {
            // A cancel during the checks stops the job before it starts: the check and the
            // start happen under the lock `cancel` takes.
            let mut run = lock(&self.queue_run);
            if run.cancelled {
                return (
                    QueueResult::Cancelled,
                    Some(msg!("queue.reason.cancelled")),
                    None,
                );
            }
            if let (Some(id), Work::Copy { settings, .. }) = (mirror, &mut work)
                && let Some(m) = settings.mirror.as_mut()
            {
                m.archive_deleted = self.pending_archive_deletion(id);
            }
            // The checks passed: the window shows this job's Copying screen from here.
            sink.send(started);
            if let Err(reason) = self.jobs.start_work(work, Forward(sink.clone())) {
                return (QueueResult::Failed, Some(reason), None);
            }
            run.job_started = true;
        }
        self.jobs.wait();
        let handle = self.jobs.current_handle();
        let summary = handle.as_ref().and_then(JobHandle::summary);
        let (result, reason) = match summary.as_ref().map(|s| s.outcome) {
            Some(JobOutcome::Complete) => (QueueResult::Complete, None),
            Some(JobOutcome::Cancelled) => {
                (QueueResult::Cancelled, Some(msg!("queue.reason.cancelled")))
            }
            Some(JobOutcome::Failures) => {
                (QueueResult::Failed, summary.as_ref().map(failure_reason))
            }
            Some(JobOutcome::Stopped) | None => (
                QueueResult::Failed,
                Some(stopped_reason(summary.and_then(|s| s.stopped_because))),
            ),
        };
        (result, reason, handle)
    }
}

/// Why a queued job stopped, as one sentence: a reason that is already a question keeps its
/// own ending (chosen by its key, not its words).
fn stopped_reason(why: Option<Message>) -> Message {
    match why {
        Some(why)
            if matches!(
                why.key.as_str(),
                "errors.fatal.destinationGone" | "errors.fatal.sourceGone"
            ) =>
        {
            why
        }
        Some(why) => msg!("queue.reason.stoppedBecause", why = why),
        None => msg!("queue.reason.stopped"),
    }
}

/// Why a job ended with failures: its failed files, or a mirror's files not removed.
fn failure_reason(s: &SummaryView) -> Message {
    if let Some(c) = &s.check {
        let mut parts = Vec::new();
        if c.changed > 0 {
            parts.push(msg!("queue.reason.part.changed", count = c.changed));
        }
        if c.missing > 0 {
            parts.push(msg!("queue.reason.part.missing", count = c.missing));
        }
        if c.failed > 0 {
            parts.push(msg!("queue.reason.part.unreadable", count = c.failed));
        }
        if c.problem_count() > 0 {
            parts.push(msg!(
                "queue.reason.part.problems",
                count = c.problem_count()
            ));
        }
        return msg!("queue.reason.check", parts = parts);
    }
    if s.failed > 0 {
        return msg!("queue.reason.filesFailed", count = s.failed);
    }
    if s.unread > 0 {
        return msg!("queue.reason.unread", count = s.unread);
    }
    let n = s.mirror.as_ref().map_or(0, |m| m.removal_failures.len());
    if n > 0 {
        return msg!("queue.reason.notRemoved", count = n);
    }
    if s.dir_errors > 0 {
        return msg!("queue.reason.dirs", count = s.dir_errors);
    }
    if let Some(note) = s
        .mirror
        .as_ref()
        .and_then(|m| m.archive_not_deleted.clone())
    {
        return note;
    }
    if let Some(e) = &s.checksum_error {
        return msg!("queue.reason.checksum", why = e);
    }
    match &s.durability_error {
        Some(e) => msg!("queue.reason.durability", why = e),
        None => msg!("queue.reason.incomplete"),
    }
}

fn nothing_to_verify() -> Message {
    msg!("errors.verify.nothing")
}

/// Plans a check of `dir`: a directory that is there.
fn plan_check(dir: &Path) -> Result<CheckPlan, Message> {
    if !dir.exists() {
        return Err(crate::session::gone(dir));
    }
    if !dir.is_dir() {
        return Err(msg!("errors.verify.notADirectory"));
    }
    secopy_core::check::plan(dir).map_err(|e| {
        msg!(
            "errors.verify.cantRead",
            path = dir,
            why = say::io_error(&e)
        )
    })
}

fn count_of(results: &[QueueResultView], kind: QueueResult) -> u32 {
    results.iter().filter(|r| r.result == kind).count() as u32
}

/// A progress or queue channel that also keeps the menu bar icon up to date (#80).
#[derive(Clone)]
pub struct Watched<C> {
    pub to: C,
    pub app: AppHandle,
}

impl ProgressSink for Watched<Channel<ProgressView>> {
    fn send(&self, view: ProgressView) {
        let shown = view.clone();
        crate::menubar::deliver(
            || {
                let _ = Channel::send(&self.to, shown);
            },
            || crate::menubar::progress(&self.app, &view),
        );
    }
}

impl QueueSink for Watched<Channel<QueueEvent>> {
    fn send(&self, event: QueueEvent) {
        let shown = event.clone();
        crate::menubar::deliver(
            || {
                let _ = Channel::send(&self.to, shown);
            },
            || crate::menubar::queue_event(&self.app, &event),
        );
    }
}

/// The window is closing: hides it behind the menu bar icon when that applies (#80); `false`
/// and nothing changed otherwise, and the window asks or quits as before.
#[tauri::command]
#[specta::specta]
pub fn hide_to_menu_bar(app: AppHandle) -> bool {
    app.state::<AppState>().wants_hide() && crate::menubar::hide(&app)
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
) -> Result<T, Message> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| {
            eprintln!("Secopy: a command's thread failed: {e}");
            say::internal()
        })
}

/// The session, started afresh after a panic while it was held (#69): a change may have
/// been left half made (a new source with the old plan), and a copy must never start from
/// something the window didn't show.
fn session(state: &AppState) -> MutexGuard<'_, Session> {
    state.session.lock().unwrap_or_else(|poisoned| {
        let mut session = poisoned.into_inner();
        session.restart();
        state.session.clear_poison();
        session
    })
}

/// FROM's Choose…: a folder or files, in one panel (FR-1, FR-2). `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn pick_source(app: AppHandle) -> Result<Option<Vec<String>>, Message> {
    let failed = |e: &dyn std::fmt::Display| {
        eprintln!("Secopy: the source panel failed: {e}");
        say::internal()
    };
    let (done, picked) = std::sync::mpsc::channel();
    crate::picker::pick_source(&app, done).map_err(|e| failed(&e))?;
    tauri::async_runtime::spawn_blocking(move || picked.recv().ok().flatten())
        .await
        .map_err(|e| failed(&e))
}

/// Scans a picked, dropped or chosen source (FR-1..FR-3). A newer scan replaces
/// an older one.
#[tauri::command]
#[specta::specta]
pub async fn scan_source(app: AppHandle, paths: Vec<String>) -> Result<SessionView, Message> {
    blocking(app, move |state| {
        state.rescan(Change::Pick(paths.into_iter().map(PathBuf::from).collect()))
    })
    .await
}

/// The "Include the folder" checkbox (FR-4); this run's file types stay.
#[tauri::command]
#[specta::specta]
pub async fn set_include_folder(app: AppHandle, include: bool) -> Result<SessionView, Message> {
    blocking(app, move |state| {
        state.rescan(Change::IncludeFolder(include))
    })
    .await
}

/// Clears the source; the destination stays ("New copy", RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn clear_source(app: AppHandle) -> Result<SessionView, Message> {
    blocking(app, |state| session(state).clear_source()).await
}

/// `None` selects every extension (FR-8, FR-10).
#[tauri::command]
#[specta::specta]
pub async fn set_filter(
    app: AppHandle,
    selected: Option<Vec<ExtensionKey>>,
) -> Result<SessionView, Message> {
    blocking(app, move |state| session(state).set_filter(selected)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_destination(app: AppHandle, path: Option<String>) -> Result<SessionView, Message> {
    blocking(app, move |state| {
        session(state).set_destination(path.map(PathBuf::from))
    })
    .await
}

/// New copy as it is now (#112).
#[tauri::command]
#[specta::specta]
pub async fn session_view(app: AppHandle) -> Result<SessionView, Message> {
    blocking(app, |state| state.session_view()).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_conflicts(app: AppHandle, policy: ConflictPolicy) -> Result<SessionView, Message> {
    blocking(app, move |state| session(state).set_policy(policy)).await
}

/// Starts copying what the main window shows; progress arrives on `on_progress`.
#[tauri::command]
#[specta::specta]
pub async fn start_job(
    app: AppHandle,
    verify: bool,
    on_progress: Channel<ProgressView>,
) -> Result<(), Message> {
    let sink = Watched {
        to: on_progress,
        app: app.clone(),
    };
    blocking(app, move |state| state.start(verify, sink)).await?
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

/// The menu bar panel's content now (#80); it asks when it opens.
#[tauri::command]
#[specta::specta]
pub fn menubar_view(app: AppHandle) -> Option<crate::menubar::PanelView> {
    crate::menubar::panel_view(&app)
}

/// The panel's Open Secopy: the window again, back in the Dock (#80).
#[tauri::command]
#[specta::specta]
pub fn open_main_window(app: AppHandle) {
    crate::menubar::show(&app);
}

/// The panel's Quit Secopy: asks first while a job runs, as ⌘Q does (#80).
#[tauri::command]
#[specta::specta]
pub fn quit_app(app: AppHandle) {
    crate::quit(&app);
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
) -> Result<Vec<FinishedRow>, Message> {
    blocking(app, move |state| {
        state.jobs.finished_page(offset, limit, failed_only)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn job_summary(app: AppHandle) -> Result<Option<SummaryView>, Message> {
    blocking(app, |state| state.jobs.summary()).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_report(app: AppHandle, path: String) -> Result<(), Message> {
    blocking(app, move |state| {
        state.jobs.save_report(&PathBuf::from(path))
    })
    .await?
}

/// The UI says which File menu items apply.
#[tauri::command]
#[specta::specta]
pub fn set_menu_state(app: AppHandle, setup: bool, can_start: bool, copying: bool, busy: bool) {
    if let Some(menu) = app.try_state::<crate::FileMenu<tauri::Wry>>() {
        menu.update(setup, can_start, copying, busy);
    }
}

impl AppState {
    pub fn mirror_presets(&self) -> Vec<MirrorPreset> {
        lock(&self.mirrors).presets.clone()
    }

    /// Changes the mirror presets and saves them; memory changes only once saved.
    fn change_mirrors<T>(
        &self,
        change: impl FnOnce(&mut MirrorPresets) -> Result<T, Message>,
    ) -> Result<Vec<MirrorPreset>, Message> {
        let mut presets = lock(&self.mirrors);
        let mut next = presets.clone();
        change(&mut next)?;
        self.store
            .save(MIRRORS, &next)
            .map_err(|e| msg!("errors.save.mirror", why = e))?;
        *presets = next;
        Ok(presets.presets.clone())
    }

    pub fn create_mirror_preset(
        &self,
        input: MirrorPresetInput,
    ) -> Result<Vec<MirrorPreset>, Message> {
        self.change_mirrors(|m| m.add(input))
    }

    pub fn edit_mirror_preset(
        &self,
        id: &str,
        input: MirrorPresetInput,
    ) -> Result<Vec<MirrorPreset>, Message> {
        self.change_mirrors(|m| m.edit(id, input))
    }

    pub fn delete_mirror_preset(&self, id: &str) -> Result<Vec<MirrorPreset>, Message> {
        self.change_mirrors(|m| {
            m.delete(id);
            Ok(())
        })
    }

    /// What mirror `id`'s archive holds now (#101): asked before it's switched to Delete.
    pub fn mirror_archive(&self, id: &str) -> Result<ArchiveView, Message> {
        let preset = lock(&self.mirrors)
            .get(id)
            .cloned()
            .ok_or_else(preset_gone)?;
        let summary = secopy_core::mirror::archive_summary(Path::new(&preset.destination));
        let held = summary.as_ref().ok().copied().flatten();
        Ok(ArchiveView {
            destination: preset.destination.clone(),
            files: held.map_or(0, |s| count(s.files)),
            bytes: held.map_or(0, |s| s.bytes),
            oldest: held.and_then(|s| s.oldest).map(|t| t.to_rfc3339()),
            connected: summary.is_ok(),
            busy: self.busy(),
        })
    }

    /// "Delete them now" (#101): the whole archive of mirror `id`'s destination; never while a
    /// job or the queue runs. What can't be deleted goes once it reaches the preset's days.
    /// `shown` is the destination the user saw: if the preset's changed since, nothing is
    /// deleted (#113).
    pub fn delete_mirror_archive(
        &self,
        id: &str,
        shown: &str,
    ) -> Result<ArchiveDeletedView, Message> {
        let preset = lock(&self.mirrors)
            .get(id)
            .cloned()
            .ok_or_else(preset_gone)?;
        if preset.destination != shown {
            return Err(msg!("errors.mirror.archiveMoved"));
        }
        // Held throughout, so the queue can't start in the middle (`claim_queue_run`).
        let run = lock(&self.queue_run);
        if run.running || self.jobs.is_running() {
            return Err(msg!("errors.mirror.archiveBusy"));
        }
        let destination = Path::new(&preset.destination);
        if !destination.is_dir() {
            return Err(crate::session::gone(destination));
        }
        let done = secopy_core::mirror::delete_archive(destination);
        drop(run);
        Ok(ArchiveDeletedView {
            removed: count(done.removed),
            not_deleted: crate::say::archive_not_deleted(&done, preset.deleted.days),
        })
    }

    /// "Delete it at the next run" (#101): kept with the destination it applies to.
    pub fn clear_mirror_archive_next_run(&self, id: &str) -> Result<Vec<MirrorPreset>, Message> {
        self.change_mirrors(|m| {
            let p = m
                .presets
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or_else(preset_gone)?;
            p.clear_archive = Some(p.destination.clone());
            Ok(())
        })
    }

    /// "Delete it at the next run" (#101), as that run is started, before its job exists: only
    /// for the destination it was asked for, and once. The preset is saved without it first,
    /// so a save that fails deletes nothing and it stays pending.
    fn pending_archive_deletion(&self, id: &str) -> Option<secopy_core::mirror::ArchiveDeleted> {
        // Taken in one step with the save, so an edit can't slip between reading and clearing.
        let mut target = None;
        let saved = self.change_mirrors(|m| {
            if let Some(p) = m.presets.iter_mut().find(|p| p.id == id)
                && let Some(path) = p.clear_archive.take()
            {
                target = (path == p.destination).then_some(path);
            }
            Ok(())
        });
        if let Err(e) = saved {
            eprintln!("Secopy: the archive wasn't deleted: couldn't save {MIRRORS}: {e:?}");
            return None;
        }
        target.map(|path| secopy_core::mirror::delete_archive(Path::new(&path)))
    }

    /// Works out what the preset would do now and keeps it for the preview's Start (FR-47).
    pub fn preview_mirror(
        &self,
        id: &str,
        on_compared: &(dyn Fn(u64, u64) + Sync),
    ) -> Result<MirrorPreviewView, Message> {
        let preset = lock(&self.mirrors)
            .get(id)
            .cloned()
            .ok_or_else(preset_gone)?;
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
        let failing = plan
            .copy
            .files
            .iter()
            .filter(|f| matches!(f.action, secopy_core::plan::Action::Fail(_)))
            .count();
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
            failing: count(failing),
            unchanged: count(plan.copy.files.len() - plan.changes.len() - failing),
            guard: plan.guard.as_ref().map(say::guard),
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
                Change::New => (PreviewKind::New, msg!("mirror.preview.reason.new")),
                Change::Changed => (PreviewKind::Changed, msg!("mirror.preview.reason.changed")),
                Change::ContentsDiffer => (
                    PreviewKind::Changed,
                    msg!("mirror.preview.reason.contentsDiffer"),
                ),
            };
            PreviewRow {
                path: show(&entry.rel),
                size: entry.size,
                kind,
                reason,
            }
        });
        let removals = plan.removals.iter().map(|rel| PreviewRow {
            path: show(rel),
            size: std::fs::metadata(plan.copy.dest.join(rel)).map_or(0, |m| m.len()),
            kind: PreviewKind::Removed,
            reason: msg!("mirror.preview.reason.deleted"),
        });
        changes
            .chain(removals)
            .filter(|r| kind.is_none_or(|k| r.kind == k))
            .skip(offset as usize)
            .take(limit as usize)
            .collect()
    }

    /// Start on the preview (FR-47): preset `id`'s plan as it was previewed, through the job
    /// runner; a preview runs once.
    pub fn run_mirror(&self, id: &str, sink: impl ProgressSink) -> Result<(), Message> {
        // Held to the start, so the queue can't start between the archive deletion and the job.
        let run = lock(&self.queue_run);
        if run.running {
            return Err(msg!("errors.queue.busy"));
        }
        let mut preview = lock(&self.preview);
        let (previewed, job) = match preview.as_ref() {
            Some((preset, job)) if preset.id == id => (preset.clone(), job.clone()),
            _ => return Err(msg!("errors.mirror.previewFirst")),
        };
        if lock(&self.mirrors).get(id) != Some(&previewed) {
            *preview = None;
            return Err(msg!("errors.mirror.changedSincePreview"));
        }
        if self.jobs.is_running() {
            return Err(msg!("errors.job.alreadyRunning"));
        }
        let mut settings = JobSettings::for_mirror(&job, chrono::Local::now());
        if let Some(m) = settings.mirror.as_mut() {
            m.archive_deleted = self.pending_archive_deletion(id);
        }
        self.jobs.start(job.ready(), true, settings, sink)?;
        *preview = None;
        drop(run);
        Ok(())
    }

    /// Verify's Choose…: plans a check of `path` and keeps it for Verify's Start.
    pub fn check_directory(&self, path: &Path) -> Result<CheckView, Message> {
        let plan = plan_check(path)?;
        let view = CheckView {
            directory: show(path),
            checksum_files: count(plan.checksum_files.len()),
            files: count(plan.files.len()),
            bytes: plan.total_bytes,
            not_checked: count(plan.not_checked.len()),
            problems: plan.problems.iter().map(say::check_problem).collect(),
        };
        *lock(&self.checking) = Some((path.to_path_buf(), Arc::new(plan)));
        Ok(view)
    }

    /// Verify's Start: runs the plan made for `path`, once.
    pub fn start_check(&self, path: &str, sink: impl ProgressSink) -> Result<(), Message> {
        if lock(&self.queue_run).running {
            return Err(msg!("errors.queue.busy"));
        }
        let planned = lock(&self.checking).take();
        let Some((dir, plan)) = planned.filter(|(dir, _)| show(dir) == path) else {
            return Err(msg!("errors.verify.chooseAgain"));
        };
        if plan.files.is_empty() {
            *lock(&self.checking) = Some((dir, plan));
            return Err(nothing_to_verify());
        }
        self.jobs.start_work(Work::Check(plan), sink)
    }

    pub fn add_check_to_queue(&self, path: &str) -> Result<QueueView, Message> {
        self.change_queue(|q| {
            q.add_check(PathBuf::from(path));
            true
        })
    }

    pub fn add_mirror_to_queue(&self, id: &str) -> Result<QueueView, Message> {
        if lock(&self.mirrors).get(id).is_none() {
            return Err(preset_gone());
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
pub async fn queue(app: AppHandle) -> Result<QueueView, Message> {
    blocking(app, |state| state.queue_view()).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_to_queue(app: AppHandle, verify: bool) -> Result<QueueView, Message> {
    blocking(app, move |state| state.add_to_queue(verify)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn remove_from_queue(app: AppHandle, index: u32) -> Result<QueueView, Message> {
    blocking(app, move |state| {
        state.change_queue(|q| q.remove(index as usize))
    })
    .await?
}

#[tauri::command]
#[specta::specta]
pub async fn move_in_queue(app: AppHandle, from: u32, to: u32) -> Result<QueueView, Message> {
    blocking(app, move |state| {
        state.change_queue(|q| q.move_job(from as usize, to as usize))
    })
    .await?
}

#[tauri::command]
#[specta::specta]
pub async fn clear_queue(app: AppHandle) -> Result<QueueView, Message> {
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
) -> Result<QueueView, Message> {
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
pub fn run_queue(app: AppHandle, on_event: Channel<QueueEvent>) -> Result<(), Message> {
    let state = app.state::<AppState>();
    state.claim_queue_run()?;
    let runner = app.clone();
    let sink = Watched {
        to: on_event,
        app: app.clone(),
    };
    let thread = std::thread::spawn(move || {
        runner.state::<AppState>().run_claimed_to_done(sink);
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
) -> Result<Vec<FinishedRow>, Message> {
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
pub async fn queue_save_report(app: AppHandle, index: u32, path: String) -> Result<(), Message> {
    blocking(app, move |state| {
        state
            .queue_job(index as usize)
            .ok_or_else(|| msg!("errors.report.noJob"))?
            .save_report(&PathBuf::from(path))
    })
    .await?
}

#[tauri::command]
#[specta::specta]
pub async fn mirror_presets(app: AppHandle) -> Result<Vec<MirrorPreset>, Message> {
    blocking(app, |state| state.mirror_presets()).await
}

/// What a mirror's archive holds (#101).
#[tauri::command]
#[specta::specta]
pub async fn mirror_archive(app: AppHandle, id: String) -> Result<ArchiveView, Message> {
    blocking(app, move |state| state.mirror_archive(&id)).await?
}

/// Deletes a mirror's archive now (#101).
#[tauri::command]
#[specta::specta]
pub async fn delete_mirror_archive(
    app: AppHandle,
    id: String,
    destination: String,
) -> Result<ArchiveDeletedView, Message> {
    blocking(app, move |state| {
        state.delete_mirror_archive(&id, &destination)
    })
    .await?
}

/// Deletes a mirror's archive at its next run (#101).
#[tauri::command]
#[specta::specta]
pub async fn clear_mirror_archive_next_run(
    app: AppHandle,
    id: String,
) -> Result<Vec<MirrorPreset>, Message> {
    blocking(app, move |state| state.clear_mirror_archive_next_run(&id)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn create_mirror_preset(
    app: AppHandle,
    input: MirrorPresetInput,
) -> Result<Vec<MirrorPreset>, Message> {
    blocking(app, move |state| state.create_mirror_preset(input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn edit_mirror_preset(
    app: AppHandle,
    id: String,
    input: MirrorPresetInput,
) -> Result<Vec<MirrorPreset>, Message> {
    blocking(app, move |state| state.edit_mirror_preset(&id, input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn delete_mirror_preset(
    app: AppHandle,
    id: String,
) -> Result<Vec<MirrorPreset>, Message> {
    blocking(app, move |state| state.delete_mirror_preset(&id)).await?
}

/// A mirror's preview (FR-47); the preview's Start then runs it.
#[tauri::command]
#[specta::specta]
pub async fn preview_mirror(
    app: AppHandle,
    id: String,
    on_compared: Channel<ComparedView>,
) -> Result<MirrorPreviewView, Message> {
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
) -> Result<Vec<PreviewRow>, Message> {
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
) -> Result<(), Message> {
    let sink = Watched {
        to: on_progress,
        app: app.clone(),
    };
    blocking(app, move |state| state.run_mirror(&id, sink)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn add_mirror_to_queue(app: AppHandle, id: String) -> Result<QueueView, Message> {
    blocking(app, move |state| state.add_mirror_to_queue(&id)).await?
}

/// Verify's Choose…: what `path`'s checksum files list (plan 8).
#[tauri::command]
#[specta::specta]
pub async fn check_directory(app: AppHandle, path: String) -> Result<CheckView, Message> {
    blocking(app, move |state| state.check_directory(Path::new(&path))).await?
}

/// Verify's Start: checks the directory chosen last; progress arrives on `on_progress`.
#[tauri::command]
#[specta::specta]
pub async fn start_check(
    app: AppHandle,
    path: String,
    on_progress: Channel<ProgressView>,
) -> Result<(), Message> {
    let sink = Watched {
        to: on_progress,
        app: app.clone(),
    };
    blocking(app, move |state| state.start_check(&path, sink)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn add_check_to_queue(app: AppHandle, path: String) -> Result<QueueView, Message> {
    blocking(app, move |state| state.add_check_to_queue(&path)).await?
}

/// "Retry": only the failed files, checked again (RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn retry_failed(app: AppHandle) -> Result<SessionView, Message> {
    blocking(app, |state| state.retry_failed()).await?
}

/// Everything the window needs at start; load problems are handed out once.
#[tauri::command]
#[specta::specta]
pub async fn app_start(app: AppHandle) -> Result<StartView, Message> {
    blocking(app, |state| state.start_view()).await
}

#[tauri::command]
#[specta::specta]
pub async fn recent_destinations(app: AppHandle) -> Result<Vec<String>, Message> {
    blocking(app, |state| state.recent()).await
}

#[tauri::command]
#[specta::specta]
pub async fn select_copy_preset(
    app: AppHandle,
    id: Option<String>,
) -> Result<SessionView, Message> {
    blocking(app, move |state| state.select_copy_preset(id)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn update_copy_preset(app: AppHandle) -> Result<CopyPresetsView, Message> {
    blocking(app, |state| state.update_copy_preset()).await?
}

#[tauri::command]
#[specta::specta]
pub async fn save_copy_preset_as(app: AppHandle, name: String) -> Result<CopyPresetsView, Message> {
    blocking(app, move |state| state.save_copy_preset_as(name)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn create_copy_preset(
    app: AppHandle,
    input: CopyPresetInput,
) -> Result<Vec<CopyPreset>, Message> {
    blocking(app, move |state| state.create_copy_preset(input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn edit_copy_preset(
    app: AppHandle,
    id: String,
    input: CopyPresetInput,
) -> Result<CopyPresetsView, Message> {
    blocking(app, move |state| state.edit_copy_preset(&id, input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn delete_copy_preset(app: AppHandle, id: String) -> Result<CopyPresetsView, Message> {
    blocking(app, move |state| state.delete_copy_preset(&id)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn set_settings(app: AppHandle, settings: Settings) -> Result<Settings, Message> {
    blocking(app, move |state| state.set_settings(settings)).await?
}

/// Copy or Copy & Verify, remembered for the next launch (FR-36).
#[tauri::command]
#[specta::specta]
pub async fn set_mode(app: AppHandle, verify: bool) -> Result<(), Message> {
    blocking(app, move |state| state.remember(|r| r.verify = verify)).await
}

/// A file on the Import screen, and what the screen was worked out against.
struct Pending {
    contents: Contents,
    seen: (CopyPresets, MirrorPresets, Settings),
}

/// Why Import is refused while a job or the queue runs (#77).
pub fn import_waits() -> Message {
    msg!("errors.import.waits")
}

/// Why Import is refused when presets or settings changed after the file was opened.
pub fn import_changed() -> Message {
    msg!("errors.import.changed")
}

/// "1 copy preset, 2 mirror presets and the settings".
fn what_line(copies: usize, mirrors: usize, settings: bool) -> Message {
    let mut parts = Vec::new();
    if copies > 0 {
        parts.push(msg!("export.what.copyPresets", count = copies));
    }
    if mirrors > 0 {
        parts.push(msg!("export.what.mirrorPresets", count = mirrors));
    }
    if settings {
        parts.push(msg!("export.what.settings"));
    }
    match parts.as_slice() {
        [] => msg!("export.what.nothing"),
        [one] => one.clone(),
        [a, b] => msg!("format.list2", a = a, b = b),
        [a, b, c] => msg!("format.list3", a = a, b = b, c = c),
        _ => unreachable!("three kinds at most"),
    }
}

/// What an import saves, kind by kind, in this order.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    CopyPresets,
    MirrorPresets,
    Settings,
}

impl Kind {
    /// "The copy presets", to start a sentence.
    fn named(self) -> Message {
        match self {
            Kind::CopyPresets => msg!("import.kind.copyPresets"),
            Kind::MirrorPresets => msg!("import.kind.mirrorPresets"),
            Kind::Settings => msg!("import.kind.settings"),
        }
    }
}

/// Export and import (#77).
impl AppState {
    pub fn export_all(&self, path: &Path, what: &ExportWhat) -> Result<Message, Message> {
        let settings = what.settings.then(|| lock(&self.settings).clone());
        let copies = if what.copy_presets {
            lock(&self.copy_presets).presets.clone()
        } else {
            Vec::new()
        };
        let mirrors = if what.mirror_presets {
            lock(&self.mirrors).presets.clone()
        } else {
            Vec::new()
        };
        if settings.is_none() && copies.is_empty() && mirrors.is_empty() {
            return Err(msg!("errors.export.nothing"));
        }
        let text = transfer::export_text(
            settings.as_ref(),
            &copies,
            &mirrors,
            env!("CARGO_PKG_VERSION"),
            chrono::Local::now(),
        );
        transfer::write_file(path, &text)?;
        Ok(msg!(
            "export.done",
            what = what_line(copies.len(), mirrors.len(), settings.is_some()),
        ))
    }

    pub fn export_copy_preset(&self, id: &str, path: &Path) -> Result<Message, Message> {
        let preset = lock(&self.copy_presets)
            .get(id)
            .cloned()
            .ok_or_else(|| msg!("errors.preset.gone"))?;
        let text = transfer::export_text(
            None,
            std::slice::from_ref(&preset),
            &[],
            env!("CARGO_PKG_VERSION"),
            chrono::Local::now(),
        );
        transfer::write_file(path, &text)?;
        Ok(msg!("export.doneNamed", name = &preset.name))
    }

    pub fn export_mirror_preset(&self, id: &str, path: &Path) -> Result<Message, Message> {
        let preset = lock(&self.mirrors)
            .get(id)
            .cloned()
            .ok_or_else(|| msg!("errors.mirror.gone"))?;
        let text = transfer::export_text(
            None,
            &[],
            std::slice::from_ref(&preset),
            env!("CARGO_PKG_VERSION"),
            chrono::Local::now(),
        );
        transfer::write_file(path, &text)?;
        Ok(msg!("export.doneNamed", name = &preset.name))
    }

    /// Reads `path` for the Import screen. Changes nothing.
    pub fn open_import(&self, path: &Path) -> Result<ImportView, Message> {
        if self.busy() {
            return Err(import_waits());
        }
        let contents = transfer::read_file(path)?;
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        // One lock at a time: a guard in a tuple lives to the end of the statement.
        let copies = lock(&self.copy_presets).clone();
        let mirrors = lock(&self.mirrors).clone();
        let settings = lock(&self.settings).clone();
        let view = transfer::plan(&name, &contents, &copies, &mirrors, &settings, &|p| {
            Path::new(p).exists()
        });
        *lock(&self.importing) = Some(Pending {
            contents,
            seen: (copies, mirrors, settings),
        });
        Ok(view)
    }

    /// Imports what was ticked, kind by kind (copy presets, mirror presets, settings), each
    /// saved before the next; the first failure stops it, and the message says what went in
    /// and what didn't. The file is taken once, so a second press imports nothing; nothing is
    /// imported if a job runs or the presets or settings changed since the screen was shown.
    pub fn apply_import(&self, choices: &ImportChoices) -> Result<ImportDone, Message> {
        // Held throughout, so the queue can't start in the middle (`claim_queue_run`).
        let run = lock(&self.queue_run);
        if run.running || self.jobs.is_running() {
            return Err(import_waits());
        }
        let Pending { contents, seen } = lock(&self.importing)
            .take()
            .ok_or_else(|| msg!("errors.import.noFile"))?;
        // One lock at a time: a guard in a tuple lives to the end of the statement.
        let copies = lock(&self.copy_presets).clone();
        let mirrors = lock(&self.mirrors).clone();
        let settings = lock(&self.settings).clone();
        let now = (copies, mirrors, settings);
        if now != seen {
            return Err(import_changed());
        }
        let wants_settings = choices.settings && matches!(contents.settings, Some(Ok(_)));
        let mut done = (0usize, 0usize, false);
        let failure = (|| -> Result<(), (Kind, Message)> {
            if !choices.copy_presets.is_empty() {
                done.0 = self
                    .change_whole(&self.copy_presets, COPY_PRESETS, |p| {
                        transfer::apply_copy(&contents, &choices.copy_presets, p)
                    })
                    .map_err(|e| (Kind::CopyPresets, e))?;
            }
            if !choices.mirror_presets.is_empty() {
                done.1 = self
                    .change_whole(&self.mirrors, MIRRORS, |m| {
                        transfer::apply_mirrors(&contents, &choices.mirror_presets, m)
                    })
                    .map_err(|e| (Kind::MirrorPresets, e))?;
            }
            if wants_settings && let Some(Ok(theirs)) = &contents.settings {
                self.change_whole(&self.settings, SETTINGS, |_| Ok((theirs.clone(), ())))
                    .map_err(|e| (Kind::Settings, e))?;
                done.2 = true;
            }
            Ok(())
        })()
        .err();
        drop(run);
        let imported = what_line(done.0, done.1, done.2);
        let (message, failed) = match failure {
            None => (msg!("import.done", what = imported), false),
            Some((kind, why)) => {
                // The kinds after the one that failed were never tried.
                let mirrors_later = kind == Kind::CopyPresets && !choices.mirror_presets.is_empty();
                let settings_later = kind != Kind::Settings && wants_settings;
                let failed = if done == (0, 0, false) {
                    msg!("import.failed", what = kind.named(), why = why)
                } else {
                    msg!(
                        "import.partly",
                        done = imported,
                        what = kind.named(),
                        why = why
                    )
                };
                let message = match (mirrors_later, settings_later) {
                    (false, false) => failed,
                    (true, false) => msg!("import.andNot.mirrors", message = failed),
                    (false, true) => msg!("import.andNot.settings", message = failed),
                    (true, true) => msg!("import.andNot.both", message = failed),
                };
                (message, true)
            }
        };
        let settings = lock(&self.settings).clone();
        let copy_presets = lock(&self.copy_presets).presets.clone();
        let mirror_presets = lock(&self.mirrors).presets.clone();
        // The preset New copy has selected, replaced: loaded there as it is now, so a later
        // Update doesn't put the old one back (#116).
        let selected = session(self).preset().cloned();
        let session = selected.and_then(|old| {
            let new = copy_presets.iter().find(|p| p.id == old.id)?;
            (*new != old).then(|| self.rescan(Change::CopyPreset(Some(new.clone()))))
        });
        Ok(ImportDone {
            message,
            failed,
            settings,
            copy_presets,
            mirror_presets,
            session,
        })
    }

    /// Works out the next `current` under its lock and saves it as `name`; memory changes
    /// only once the file is written.
    fn change_whole<T: serde::Serialize, R>(
        &self,
        current: &Mutex<T>,
        name: &str,
        change: impl FnOnce(&T) -> Result<(T, R), Message>,
    ) -> Result<R, Message> {
        let mut current = lock(current);
        let (next, result) = change(&current)?;
        self.store.save(name, &next)?;
        *current = next;
        Ok(result)
    }

    pub fn set_opened(&self, path: PathBuf) {
        *lock(&self.opened) = Some(path);
    }

    pub fn take_opened(&self) -> Option<String> {
        lock(&self.opened).take().map(|p| show(&p))
    }
}

#[tauri::command]
#[specta::specta]
pub async fn export_all(
    app: AppHandle,
    path: String,
    what: ExportWhat,
) -> Result<Message, Message> {
    blocking(app, move |s| s.export_all(Path::new(&path), &what)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn export_copy_preset(
    app: AppHandle,
    id: String,
    path: String,
) -> Result<Message, Message> {
    blocking(app, move |s| s.export_copy_preset(&id, Path::new(&path))).await?
}

#[tauri::command]
#[specta::specta]
pub async fn export_mirror_preset(
    app: AppHandle,
    id: String,
    path: String,
) -> Result<Message, Message> {
    blocking(app, move |s| s.export_mirror_preset(&id, Path::new(&path))).await?
}

/// Reads a `.secopy` file for the Import screen; changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn open_import(app: AppHandle, path: String) -> Result<ImportView, Message> {
    blocking(app, move |s| s.open_import(Path::new(&path))).await?
}

#[tauri::command]
#[specta::specta]
pub async fn apply_import(app: AppHandle, choices: ImportChoices) -> Result<ImportDone, Message> {
    blocking(app, move |s| s.apply_import(&choices)).await?
}

/// A `.secopy` file opened from Finder, once.
#[tauri::command]
#[specta::specta]
pub fn take_opened_file(app: AppHandle) -> Option<String> {
    app.state::<AppState>().take_opened()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Mutex as StdMutex};

    use super::*;
    use crate::message::En;
    use crate::store::{COPY_PRESETS, REMEMBERED, SETTINGS};

    /// Review: a stop's reason is one sentence, whatever it ends with.
    #[test]
    fn a_stopped_jobs_reason_ends_once() {
        use secopy_core::error::FatalError;
        let gone = stopped_reason(Some(say::fatal(&FatalError::DestinationGone)));
        assert_eq!(
            gone,
            "The destination is no longer available; was it disconnected?"
        );
        assert_eq!(
            stopped_reason(Some(say::fatal(&FatalError::DiskFull))),
            "The destination drive is full."
        );
        assert_eq!(
            stopped_reason(Some(say::internal())),
            "Secopy hit an internal error."
        );
        assert_eq!(stopped_reason(None), "Stopped.");
    }

    #[derive(Clone, Default)]
    struct Sink(Arc<StdMutex<Vec<ProgressView>>>);

    impl ProgressSink for Sink {
        fn send(&self, view: ProgressView) {
            self.0.lock().unwrap().push(view);
        }
    }

    fn input(name: &str, source: &Path) -> CopyPresetInput {
        CopyPresetInput {
            name: name.into(),
            source: show(source),
            include_folder: true,
            extensions: None,
        }
    }

    #[test]
    fn a_missing_last_preset_starts_with_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(REMEMBERED),
            r#"{"version": 1, "lastProfile": "gone"}"#,
        )
        .unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let start = state.start_view();
        assert_eq!(start.last_preset, None);
        assert!(start.warnings.is_empty());
    }

    #[test]
    fn the_last_preset_and_mode_come_back() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        let state = AppState::new(dir.path().join("data"));
        let id = state.create_copy_preset(input("FX3", &card)).unwrap()[0]
            .id
            .clone();
        state.select_copy_preset(Some(id.clone())).unwrap();
        state.remember(|r| r.verify = false);
        let again = AppState::new(dir.path().join("data")).start_view();
        assert_eq!(again.last_preset, Some(id), "the UI loads it again");
        assert!(!again.verify);
        assert!(
            again.session.destination.is_none(),
            "the destination is never restored"
        );
        assert_eq!(again.copy_presets.len(), 1);
        fs::remove_dir(&card).unwrap();
        let later = AppState::new(dir.path().join("data")).start_view();
        assert_eq!(
            later.last_preset, None,
            "its card isn't there: nothing to load, and no error at launch"
        );
    }

    /// #72: `profiles.json` and `state.json` as Secopy 0.10 wrote them. Renaming profiles
    /// to copy presets must not change a byte of what is saved.
    const COPY_PRESETS_0_10: &str = r#"{
  "version": 1,
  "profiles": [
    {
      "id": "19a2f0c4e8b1d3",
      "name": "Sony FX3",
      "source": "/Volumes/FX3_A/PRIVATE/M4ROOT/CLIP",
      "includeFolder": true,
      "extensions": [
        null,
        "mp4",
        "xml"
      ]
    },
    {
      "id": "19a2f0c4e8b1d4",
      "name": "Photos",
      "source": "",
      "includeFolder": false,
      "extensions": null
    }
  ]
}"#;
    const REMEMBERED_0_10: &str = r#"{
  "version": 1,
  "verify": false,
  "window": {
    "width": 1120.0,
    "height": 760.0
  },
  "lastProfile": "19a2f0c4e8b1d4",
  "recentDestinations": [
    "/Volumes/SSD/Footage"
  ]
}"#;

    #[test]
    fn presets_and_the_last_one_saved_by_0_10_load() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(COPY_PRESETS), COPY_PRESETS_0_10).unwrap();
        fs::write(dir.path().join(REMEMBERED), REMEMBERED_0_10).unwrap();
        let start = AppState::new(dir.path().to_path_buf()).start_view();
        assert!(start.warnings.is_empty(), "{:?}", start.warnings);
        let p = &start.copy_presets;
        assert_eq!(p.len(), 2);
        assert_eq!(
            (p[0].id.as_str(), p[0].name.as_str(), p[0].source.as_str()),
            (
                "19a2f0c4e8b1d3",
                "Sony FX3",
                "/Volumes/FX3_A/PRIVATE/M4ROOT/CLIP"
            )
        );
        assert!(p[0].include_folder);
        assert_eq!(
            p[0].extensions,
            Some(vec![None, Some("mp4".into()), Some("xml".into())])
        );
        assert_eq!((p[1].name.as_str(), p[1].source.as_str()), ("Photos", ""));
        assert!(!p[1].include_folder && p[1].extensions.is_none());
        assert_eq!(start.last_preset.as_deref(), Some("19a2f0c4e8b1d4"));
        assert!(!start.verify);
        assert_eq!(
            fs::read_to_string(dir.path().join(COPY_PRESETS)).unwrap(),
            COPY_PRESETS_0_10,
            "nothing to put right, so the file isn't rewritten"
        );
    }

    #[test]
    fn saving_writes_the_keys_0_10_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let id = state
            .create_copy_preset(input("FX3", Path::new("")))
            .unwrap()[0]
            .id
            .clone();
        state.select_copy_preset(Some(id.clone())).unwrap();
        let read = |name: &str| -> serde_json::Value {
            serde_json::from_str(&fs::read_to_string(dir.path().join(name)).unwrap()).unwrap()
        };
        let keys = |v: &serde_json::Value| -> Vec<String> {
            let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            keys.sort();
            keys
        };
        let saved = read("profiles.json");
        assert_eq!(keys(&saved), ["profiles", "version"]);
        assert_eq!(
            keys(&saved["profiles"][0]),
            ["extensions", "id", "includeFolder", "name", "source"]
        );
        let remembered = read("state.json");
        assert_eq!(
            keys(&remembered),
            [
                "lastProfile",
                "recentDestinations",
                "verify",
                "version",
                "window"
            ]
        );
        assert_eq!(remembered["lastProfile"], serde_json::Value::String(id));
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

    /// #69: copy presets read from disk are put right like new ones; one that clashes with
    /// another or has no name is kept under a free name, never dropped.
    #[test]
    fn loaded_presets_are_normalized_and_none_is_lost() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(COPY_PRESETS),
            r#"{"version": 1, "profiles": [
                {"id": "a", "name": " FX3 ", "source": " /Volumes/CARD/DCIM/ ", "includeFolder": true, "extensions": ["MP4", ".XML", "mp4"]},
                {"id": "b", "name": "fx3", "source": "", "includeFolder": true, "extensions": null},
                {"id": "a", "name": "A7", "source": "", "includeFolder": false, "extensions": null},
                {"id": "c", "name": "  ", "source": "DCIM", "includeFolder": true, "extensions": null}
            ]}"#,
        )
        .unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let start = state.start_view();
        let p = &start.copy_presets;
        let names: Vec<&str> = p.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["FX3", "fx3 (2)", "A7", "Unnamed preset"]);
        assert_eq!(p[0].source, "/Volumes/CARD/DCIM");
        assert_eq!(
            p[0].extensions,
            Some(vec![Some("mp4".into()), Some("xml".into())])
        );
        assert_eq!(p[0].id, "a");
        let ids: std::collections::HashSet<&str> = p.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), 4, "every id its own");
        assert_eq!(p[3].source, "", "not a full path: no source");
        assert_eq!(start.warnings.len(), 3, "{:?}", start.warnings);
        assert!(start.warnings.iter().any(|w| w.contains("“fx3 (2)”")));
        assert!(start.warnings.iter().any(|w| w.contains("(DCIM)")));
        let saved = Store::new(dir.path().to_path_buf()).load::<CopyPresets>(COPY_PRESETS);
        assert_eq!(saved, (state.copy_presets.lock().unwrap().clone(), None));
        assert!(
            AppState::new(dir.path().to_path_buf())
                .start_view()
                .warnings
                .is_empty()
        );
    }

    #[test]
    fn deleting_the_selected_preset_selects_none() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let id = state
            .create_copy_preset(input("FX3", Path::new("")))
            .unwrap()[0]
            .id
            .clone();
        state.select_copy_preset(Some(id.clone())).unwrap();
        let after = state.delete_copy_preset(&id).unwrap();
        assert!(after.presets.is_empty());
        assert_eq!(after.session.preset_id, None);
        assert_eq!(state.remembered.lock().unwrap().last_preset, None);
        let saved = fs::read_to_string(dir.path().join(COPY_PRESETS)).unwrap();
        assert!(!saved.contains("FX3"));
    }

    #[test]
    fn editing_the_selected_preset_applies_it() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let clip = dir.path().join("CLIP");
        fs::create_dir_all(&clip).unwrap();
        fs::write(clip.join("a.mp4"), b"a").unwrap();
        let id = state
            .create_copy_preset(input("FX3", Path::new("")))
            .unwrap()[0]
            .id
            .clone();
        state.select_copy_preset(Some(id.clone())).unwrap();
        let after = state
            .edit_copy_preset(&id, input("FX3 A-cam", &clip))
            .unwrap();
        assert_eq!(after.presets[0].name, "FX3 A-cam");
        assert_eq!(after.session.preset_id, Some(id));
        assert_eq!(after.session.source.unwrap().label, show(&clip), "loaded");
    }

    #[test]
    fn save_as_new_selects_and_saves_the_new_preset() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(card.join("DCIM")).unwrap();
        fs::write(card.join("DCIM/a.jpg"), b"a").unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.rescan(Change::Pick(vec![card.join("DCIM")]));
        let after = state.save_copy_preset_as("Photos".into()).unwrap();
        let id = after.presets[0].id.clone();
        assert_eq!(after.presets[0].source, show(&card.join("DCIM")));
        assert_eq!(after.session.preset_id, Some(id.clone()));
        assert!(!after.session.preset_changed);
        assert_eq!(state.remembered.lock().unwrap().last_preset, Some(id));
        assert!(
            state
                .save_copy_preset_as("photos".into())
                .unwrap_err()
                .contains("already a preset")
        );
        state.rescan(Change::Pick(vec![card.join("DCIM/a.jpg")]));
        assert_eq!(
            state.save_copy_preset_as("One file".into()).unwrap_err(),
            "A preset saves a directory as its source; pick a directory first."
        );
    }

    #[test]
    fn start_checks_the_destination_again_and_refuses_if_it_changed() {
        // Copied, then the destination emptied (or another drive with its name): Start from the
        // same screen would "skip" everything as already there (#112).
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("a.mov"), b"a").unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir_all(dest.join("CARD")).unwrap();
        fs::write(dest.join("CARD/a.mov"), b"a").unwrap();
        let t = fs::metadata(card.join("a.mov"))
            .unwrap()
            .modified()
            .unwrap();
        fs::File::options()
            .write(true)
            .open(dest.join("CARD/a.mov"))
            .unwrap()
            .set_modified(t)
            .unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.rescan(Change::Pick(vec![card]));
        let shown = state
            .session
            .lock()
            .unwrap()
            .set_destination(Some(dest.clone()));
        assert_eq!(shown.destination.unwrap().identical, 1);
        fs::remove_file(dest.join("CARD/a.mov")).unwrap();
        assert_eq!(
            state.start(true, Sink::default()).unwrap_err().key,
            "errors.job.changed"
        );
        let fresh = state.session_view();
        assert_eq!(fresh.destination.unwrap().identical, 0);
        // Now what the screen shows is what Start does.
        state.start(true, Sink::default()).unwrap();
        state.jobs.wait();
        assert_eq!(fs::read(dest.join("CARD/a.mov")).unwrap(), b"a");
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
                            keep_in_menu_bar: i % 11 == 0,
                        };
                        let a = state.set_settings(settings).err();
                        let b = state
                            .create_copy_preset(input(&format!("P{i}"), Path::new("")))
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
        let saved = store.load::<CopyPresets>(COPY_PRESETS).0;
        assert_eq!(saved.presets.len(), 16, "no preset lost");
        assert_eq!(saved, *state.copy_presets.lock().unwrap());
    }

    /// QA review (#116): settings that can't be saved aren't used either: the next job runs
    /// with what's saved and shown.
    #[test]
    fn settings_that_cant_be_saved_arent_used() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let state = AppState::new(data.clone());
        state.set_settings(Settings::default()).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o500)).unwrap();
        let off = Settings {
            write_checksum_file: false,
            ..Settings::default()
        };
        let failed = state.set_settings(off);
        fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).unwrap();
        if failed.is_ok() {
            return; // running as root
        }
        assert!(lock(&state.settings).write_checksum_file, "the saved value");
    }

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
                overwrite: vec![],
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
                .en()
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
            summary.results[0].reason.en().as_deref(),
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

    /// #69: a panic in the queue's thread still ends the run with a `Done`, so the window
    /// doesn't stay on the Copying screen; the jobs not done stay queued.
    #[test]
    fn a_panic_in_the_queue_thread_still_sends_done() {
        /// Panics when the second job is checked, like a bug in the run would.
        #[derive(Clone, Default)]
        struct PanicsOnSecond(Events);
        impl QueueSink for PanicsOnSecond {
            fn send(&self, e: QueueEvent) {
                if matches!(e, QueueEvent::JobChecking { index: 1, .. }) {
                    panic!("a bug");
                }
                self.0.send(e);
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1), ("B", 1), ("C", 1)]);
        let sink = PanicsOnSecond::default();
        state.claim_queue_run().unwrap();
        state.run_claimed_to_done(sink.clone());
        assert!(!state.busy());
        let events = sink.0.0.lock().unwrap();
        let Some(QueueEvent::Done { summary }) = events.last() else {
            panic!("no Done: {events:?}");
        };
        assert_eq!((summary.complete, summary.count), (1, 3), "A was copied");
        let reasons: Vec<_> = summary
            .results
            .iter()
            .map(|r| (r.result, r.reason.en()))
            .collect();
        let stopped = Some("Secopy hit an internal error; the queue stopped.".to_string());
        assert_eq!(
            reasons,
            [
                (QueueResult::Complete, None),
                (QueueResult::Failed, stopped.clone()),
                (QueueResult::Failed, stopped),
            ],
            "every job, A's result included"
        );
        assert!(summary.results[0].summary.is_some(), "A's summary opens");
        assert_eq!(state.queue_view().jobs.len(), 2, "B and C stay queued");
    }

    /// #70: a job that stopped gives the queue a full sentence, like every other reason.
    #[test]
    fn a_stopped_jobs_reason_ends_with_a_full_stop() {
        /// Panics on the job's first progress view, in the job's thread.
        #[derive(Clone, Default)]
        struct PanicsOnProgress(Events, Arc<std::sync::atomic::AtomicBool>);
        impl QueueSink for PanicsOnProgress {
            fn send(&self, e: QueueEvent) {
                if matches!(e, QueueEvent::Progress { .. })
                    && !self.1.swap(true, std::sync::atomic::Ordering::Relaxed)
                {
                    panic!("a bug");
                }
                self.0.send(e);
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        state.claim_queue_run().unwrap();
        let summary = state.run_claimed(PanicsOnProgress::default());
        assert_eq!(summary.results[0].result, QueueResult::Failed);
        assert_eq!(
            summary.results[0].reason.en().as_deref(),
            Some("Secopy hit an internal error.")
        );
    }

    /// #69 review: a job that completed, with the panic before it left the queue, is
    /// complete, and isn't run again.
    #[test]
    fn a_job_complete_just_before_a_panic_leaves_the_queue() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        let entries = lock(&state.queue).jobs.clone();
        state.claim_queue_run().unwrap();
        state.run_claimed(Events::default());
        // As if the panic came right after A ended: still queued, its result not recorded.
        lock(&state.queue).jobs = entries.clone();
        {
            let mut run = lock(&state.queue_run);
            run.done.clear();
            run.handles.clear();
            run.job_started = true;
        }
        let summary = state.after_panic(&entries, std::time::Instant::now());
        assert_eq!(summary.results[0].result, QueueResult::Complete);
        assert_eq!(summary.complete, 1);
        assert!(state.queue_view().jobs.is_empty(), "A isn't run again");
    }

    /// #69: a panic while a lock was held doesn't fail every later command.
    #[test]
    fn a_panic_while_a_lock_was_held_doesnt_break_later_commands() {
        use crate::tests::poison;
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        poison(&state.queue);
        poison(&state.settings);
        poison(&state.copy_presets);
        poison(&state.queue_run);
        assert_eq!(state.queue_view().jobs.len(), 1, "the queue as it was");
        assert!(!state.busy());
        state.set_settings(Settings::default()).unwrap();
        state
            .create_copy_preset(input("FX3", Path::new("")))
            .unwrap();
        // The session may be half changed: it starts afresh rather than copy from that.
        state.rescan(Change::Pick(vec![dir.path().join("A")]));
        poison(&state.session);
        assert!(session(&state).view().source.is_none());
        assert!(!state.session.is_poisoned(), "afresh once, not every time");
        let view = state.rescan(Change::Pick(vec![dir.path().join("A")]));
        assert!(view.source.is_some());
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
            summary.results[0].reason.en().as_deref(),
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
            e.last_error = Some(Message::raw("an old reason"));
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
        assert_eq!(left[0].last_error.en().as_deref(), Some("Stopped."));
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

    /// An archived file in `d`'s archive, from a run two days ago.
    fn archived(d: &Path) -> PathBuf {
        let run =
            secopy_core::mirror::archive_dir(d, chrono::Local::now() - chrono::Duration::days(2));
        fs::create_dir_all(&run).unwrap();
        fs::write(run.join("old.mov"), b"12345").unwrap();
        run
    }

    /// #101: before switching to Delete, the editor asks what the archive holds.
    #[test]
    fn a_mirrors_archive_says_what_it_holds() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        let a = state.mirror_archive(&id).unwrap();
        assert_eq!((a.files, a.bytes, a.connected, a.busy), (0, 0, true, false));
        archived(&d);
        let a = state.mirror_archive(&id).unwrap();
        assert_eq!((a.files, a.bytes), (1, 5));
        assert!(a.oldest.is_some(), "when its oldest run was archived");
        lock(&state.queue_run).running = true;
        let a = state.mirror_archive(&id).unwrap();
        assert_eq!((a.files, a.busy), (1, true), "counted while a job runs");
        lock(&state.queue_run).running = false;
        fs::rename(&d, dir.path().join("unplugged")).unwrap();
        assert!(!state.mirror_archive(&id).unwrap().connected);
    }

    /// #101: "Delete them now" deletes the archive, and never while a job runs.
    #[test]
    fn deleting_a_mirrors_archive_now() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        archived(&d);
        lock(&state.queue_run).running = true;
        assert_eq!(
            state.delete_mirror_archive(&id, &show(&d)).unwrap_err(),
            "Delete the archive when the current job has finished."
        );
        lock(&state.queue_run).running = false;
        let done = state.delete_mirror_archive(&id, &show(&d)).unwrap();
        assert_eq!((done.removed, done.not_deleted), (1, None));
        assert!(!d.join(secopy_core::mirror::ARCHIVE_DIR).exists());
        assert!(d.join("x.mov").exists(), "only the archive");
    }

    /// QA review (#113): Delete archive… deletes the archive it showed; the preset's
    /// destination changed since, it refuses.
    #[test]
    fn deleting_an_archive_names_the_destination_shown() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        let run = archived(&d);
        let shown = state.mirror_archive(&id).unwrap();
        assert_eq!(shown.destination, show(&d));
        let b = dir.path().join("b");
        fs::create_dir_all(&b).unwrap();
        let b_run = archived(&b);
        let mut input = state.mirror_presets()[0].input();
        input.destination = show(&b);
        state.edit_mirror_preset(&id, input).unwrap();
        assert_eq!(
            state
                .delete_mirror_archive(&id, &shown.destination)
                .unwrap_err()
                .key,
            "errors.mirror.archiveMoved"
        );
        assert!(run.exists() && b_run.exists(), "nothing deleted");
    }

    /// Review of #101: a pending deletion goes with the destination it was asked for.
    #[test]
    fn a_pending_archive_deletion_is_dropped_when_the_destination_changes() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, o, d) = mirror_state(dir.path());
        state.clear_mirror_archive_next_run(&id).unwrap();
        let run = archived(&d);
        let b = dir.path().join("b");
        fs::create_dir_all(&b).unwrap();
        let mut input = state.mirror_presets()[0].input();
        input.destination = show(&b);
        let presets = state.edit_mirror_preset(&id, input).unwrap();
        assert_eq!(presets[0].clear_archive, None);
        state.preview_mirror(&id, &|_, _| {}).unwrap();
        state.run_mirror(&id, Sink::default()).unwrap();
        state.jobs.wait();
        assert!(run.exists(), "A's archive is no longer this mirror's");
        let _ = o;
    }

    /// QA review (#113): switching back to Archive keeps the archive, so a pending deletion
    /// goes.
    #[test]
    fn a_pending_archive_deletion_is_dropped_when_switched_back_to_archive() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, o, d) = mirror_state(dir.path());
        let mut input = state.mirror_presets()[0].input();
        input.deleted.mode = crate::store::DeletedMode::Delete;
        state.edit_mirror_preset(&id, input.clone()).unwrap();
        state.clear_mirror_archive_next_run(&id).unwrap();
        let run = archived(&d);
        input.deleted.mode = crate::store::DeletedMode::Archive;
        let presets = state.edit_mirror_preset(&id, input).unwrap();
        assert_eq!(presets[0].clear_archive, None);
        state.preview_mirror(&id, &|_, _| {}).unwrap();
        state.run_mirror(&id, Sink::default()).unwrap();
        state.jobs.wait();
        assert!(run.exists(), "the archive stays");
        let _ = o;
    }

    /// Review of #101: the pending deletion is done once: when its flag can't be saved as
    /// done, nothing is deleted and it stays pending.
    #[test]
    fn a_pending_archive_deletion_that_cant_be_saved_as_done_deletes_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        state.clear_mirror_archive_next_run(&id).unwrap();
        let run = archived(&d);
        state.preview_mirror(&id, &|_, _| {}).unwrap();
        let data = dir.path().join("data");
        fs::set_permissions(&data, fs::Permissions::from_mode(0o555)).unwrap();
        state.run_mirror(&id, Sink::default()).unwrap();
        state.jobs.wait();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(run.exists());
        assert_eq!(state.mirror_presets()[0].clear_archive, Some(show(&d)));
    }

    /// Review of #101: an archive that can't be read fails the run too, saying why.
    #[test]
    fn an_archive_that_cant_be_read_fails_the_run() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        state.clear_mirror_archive_next_run(&id).unwrap();
        archived(&d);
        let root = d.join(secopy_core::mirror::ARCHIVE_DIR);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
        state.preview_mirror(&id, &|_, _| {}).unwrap();
        state.run_mirror(&id, Sink::default()).unwrap();
        state.jobs.wait();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        let s = state.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Failures);
        assert_eq!(
            s.mirror.unwrap().archive_not_deleted.unwrap(),
            "The archive couldn't be deleted (Permission denied). What's in it is removed once it's 30 days old."
        );
    }

    /// Review of #101: a queued mirror deletes its pending archive as its job starts.
    #[test]
    fn a_queued_mirror_deletes_its_pending_archive_as_it_starts() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        fs::remove_file(d.join("x.mov")).unwrap();
        fs::remove_file(d.join("y.mov")).unwrap();
        fs::remove_file(d.join("z.mov")).unwrap();
        state.clear_mirror_archive_next_run(&id).unwrap();
        let run = archived(&d);
        state.add_mirror_to_queue(&id).unwrap();
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(summary.results[0].result, QueueResult::Complete);
        assert!(!run.exists());
        assert_eq!(state.mirror_presets()[0].clear_archive, None);
    }

    /// Review of #101: archived files that couldn't be deleted make the run a failure.
    #[test]
    fn archived_files_that_cant_be_deleted_fail_the_run() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        state.clear_mirror_archive_next_run(&id).unwrap();
        let run = archived(&d);
        fs::set_permissions(&run, fs::Permissions::from_mode(0o555)).unwrap();
        state.preview_mirror(&id, &|_, _| {}).unwrap();
        state.run_mirror(&id, Sink::default()).unwrap();
        state.jobs.wait();
        fs::set_permissions(&run, fs::Permissions::from_mode(0o755)).unwrap();
        let s = state.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Failures);
        let said = "1 archived file couldn't be deleted (Permission denied). It's removed once it's 30 days old.";
        assert_eq!(failure_reason(&s), said, "the queue says why");
        assert_eq!(s.mirror.unwrap().archive_not_deleted.unwrap(), said);
    }

    /// #101: "Delete it at the next run" is kept with its destination, and done once.
    #[test]
    fn a_pending_archive_deletion_is_kept_then_done_by_the_next_run() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        let presets = state.clear_mirror_archive_next_run(&id).unwrap();
        assert_eq!(presets[0].clear_archive, Some(show(&d)));
        let run = archived(&d);
        state.preview_mirror(&id, &|_, _| {}).unwrap();
        state.run_mirror(&id, Sink::default()).unwrap();
        state.jobs.wait();
        assert!(!run.exists());
        assert_eq!(state.mirror_presets()[0].clear_archive, None, "done once");
    }

    #[test]
    fn preview_counts_and_lists_the_changes() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, ..) = mirror_state(dir.path());
        let p = state.preview_mirror(&id, &|_, _| {}).unwrap();
        assert_eq!((p.new_files, p.removed_files, p.unchanged), (1, 3, 0));
        assert_eq!(
            p.guard.en().as_deref(),
            Some("3 of the destination's 3 files would be removed.")
        );
        let removed = state.mirror_preview_page(Some(PreviewKind::Removed), 0, 10);
        assert_eq!(removed.len(), 3);
    }

    /// #58: a file that will fail (a directory is in its way) isn't "unchanged".
    #[test]
    fn preview_counts_files_that_will_fail() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id, _, d) = mirror_state(dir.path());
        fs::create_dir_all(d.join("a.mov")).unwrap();
        let p = state.preview_mirror(&id, &|_, _| {}).unwrap();
        assert_eq!((p.failing, p.unchanged, p.new_files), (1, 0, 0));
    }

    /// #57: the preview's Start runs the preview of that preset, as it was previewed, once.
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
            state.queue_view().jobs[0].last_error.en().as_deref(),
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
            summary.results[0].reason.en().as_deref(),
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
            summary.results[0].reason.en().as_deref(),
            Some("The mirror preset no longer exists.")
        );
    }

    #[test]
    fn a_directory_is_checked_then_verified() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().join("data"));
        let root = dir.path().join("Day01");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.mov"), b"a").unwrap();
        secopy_core::checksum_file::write(
            &root,
            &[(PathBuf::from("a.mov"), secopy_core::hash::hash_bytes(b"a"))],
            chrono::Local::now(),
        )
        .unwrap();
        fs::write(root.join("extra.mov"), b"x").unwrap();
        let v = state.check_directory(&root).unwrap();
        assert_eq!((v.checksum_files, v.files, v.not_checked), (1, 1, 1));
        state.start_check(&show(&root), Sink::default()).unwrap();
        state.jobs.wait();
        assert_eq!(state.jobs.summary().unwrap().outcome, JobOutcome::Complete);
        assert_eq!(
            state
                .start_check(&show(&root), Sink::default())
                .unwrap_err(),
            "Choose the directory again."
        );
    }

    #[test]
    fn a_queued_check_that_finds_a_change_fails() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().join("data"));
        let root = dir.path().join("Day01");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.mov"), b"A").unwrap();
        secopy_core::checksum_file::write(
            &root,
            &[(PathBuf::from("a.mov"), secopy_core::hash::hash_bytes(b"a"))],
            chrono::Local::now(),
        )
        .unwrap();
        state.add_check_to_queue(&show(&root)).unwrap();
        assert_eq!(state.queue_view().jobs[0].kind, "check");
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(summary.results[0].result, QueueResult::Failed);
        assert_eq!(
            summary.results[0].reason.en().as_deref(),
            Some("1 changed.")
        );
    }

    /// #69: past the 1,000 checksum file problems a summary lists, the queue's reason still
    /// counts them all.
    #[test]
    fn a_queued_check_counts_every_checksum_file_problem() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().join("data"));
        let root = dir.path().join("Day01");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.mov"), b"a").unwrap();
        secopy_core::checksum_file::write(
            &root,
            &[(PathBuf::from("a.mov"), secopy_core::hash::hash_bytes(b"a"))],
            chrono::Local::now(),
        )
        .unwrap();
        fs::write(root.join("bad.xxh64"), "not a checksum line\n".repeat(1005)).unwrap();
        state.add_check_to_queue(&show(&root)).unwrap();
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(
            summary.results[0].reason.en().as_deref(),
            Some("1,005 checksum file problems.")
        );
        let check = summary.results[0].summary.clone().unwrap().check.unwrap();
        assert_eq!((check.problems.len(), check.more_problems), (1000, Some(5)));
    }

    #[test]
    fn a_directory_without_checksum_files_cant_be_verified() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.check_directory(dir.path()).unwrap();
        assert_eq!(
            state
                .start_check(&show(dir.path()), Sink::default())
                .unwrap_err(),
            "No checksum files here: there's nothing to verify."
        );
    }

    fn with_presets(dir: &Path) -> AppState {
        let state = AppState::new(dir.join("data"));
        state
            .change_copy_presets(|p| {
                p.add(crate::store::CopyPresetInput {
                    name: "Sony FX3".into(),
                    source: "/Volumes/CARD_A/CLIP".into(),
                    include_folder: true,
                    extensions: None,
                })
            })
            .unwrap();
        state
    }

    /// #77: export on one Mac, import on another: the same presets and settings.
    #[test]
    fn exported_presets_import_on_another_mac() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let file = dir.path().join("all.secopy");
        let line = a
            .export_all(
                &file,
                &ExportWhat {
                    settings: true,
                    copy_presets: true,
                    mirror_presets: true,
                },
            )
            .unwrap();
        assert_eq!(line, "Exported 1 copy preset and the settings.");
        let b = AppState::new(dir.path().join("other"));
        let view = b.open_import(&file).unwrap();
        assert_eq!(view.copy_presets[0].name, "Sony FX3");
        let done = b
            .apply_import(&ImportChoices {
                settings: true,
                copy_presets: vec![PresetChoice {
                    index: 0,
                    replace: false,
                }],
                mirror_presets: vec![],
            })
            .unwrap();
        assert!(!done.failed);
        assert_eq!(done.message, "Imported 1 copy preset and the settings.");
        assert_eq!(done.copy_presets[0].source, "/Volumes/CARD_A/CLIP");
        let reloaded = AppState::new(dir.path().join("other"));
        assert_eq!(
            lock(&reloaded.copy_presets).presets.len(),
            1,
            "saved to disk"
        );
    }

    /// QA review (#116): Replace on the preset New copy has selected loads the imported one
    /// there, so a later Update can't put the old one back.
    #[test]
    fn replacing_the_selected_preset_loads_the_imported_one() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("A"), dir.path().join("B"));
        for d in [&a, &b] {
            fs::create_dir_all(d).unwrap();
            fs::write(d.join("x.mov"), b"x").unwrap();
        }
        let state = AppState::new(dir.path().join("data"));
        state
            .create_copy_preset(CopyPresetInput {
                name: "FX3".into(),
                source: show(&a),
                include_folder: true,
                extensions: None,
            })
            .unwrap();
        let id = lock(&state.copy_presets).presets[0].id.clone();
        state.select_copy_preset(Some(id)).unwrap();
        let file = dir.path().join("x.secopy");
        fs::write(
            &file,
            format!(
                r#"{{"secopy":1,"copyPresets":[{{"name":"fx3","source":"{}","includeFolder":false,"extensions":null}}]}}"#,
                show(&b)
            ),
        )
        .unwrap();
        state.open_import(&file).unwrap();
        let done = state
            .apply_import(&ImportChoices {
                settings: false,
                copy_presets: vec![PresetChoice {
                    index: 0,
                    replace: true,
                }],
                mirror_presets: vec![],
            })
            .unwrap();
        let view = done.session.expect("New copy reloaded");
        let source = view.source.unwrap();
        assert_eq!(source.folder, Some(show(&b)));
        assert!(source.contents_only, "the imported preset's choice");
        assert!(!view.preset_changed);
    }

    /// #77: opening the Import screen changes nothing.
    #[test]
    fn opening_an_import_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let file = dir.path().join("one.secopy");
        let id = lock(&a.copy_presets).presets[0].id.clone();
        a.export_copy_preset(&id, &file).unwrap();
        let before = lock(&a.copy_presets).clone();
        a.open_import(&file).unwrap();
        assert_eq!(*lock(&a.copy_presets), before);
    }

    /// Review focus 5: nothing is imported while a job or the queue runs.
    #[test]
    fn importing_waits_while_a_job_runs() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let file = dir.path().join("one.secopy");
        let id = lock(&a.copy_presets).presets[0].id.clone();
        a.export_copy_preset(&id, &file).unwrap();
        a.open_import(&file).unwrap();
        lock(&a.queue_run).running = true;
        assert_eq!(a.open_import(&file).unwrap_err(), import_waits());
        let choices = ImportChoices {
            copy_presets: vec![PresetChoice {
                index: 0,
                replace: false,
            }],
            ..Default::default()
        };
        assert_eq!(a.apply_import(&choices).unwrap_err(), import_waits());
        assert_eq!(lock(&a.copy_presets).presets.len(), 1);
    }

    /// Review focus 4: a save that fails partway says what was imported.
    #[test]
    fn a_failed_save_says_what_was_imported() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let text = r#"{"secopy":1,"settings":{"writeChecksumFile":false},
            "copyPresets":[{"name":"DJI","source":"","includeFolder":true,"extensions":null}],
            "mirrorPresets":[{"name":"M","origin":"/a","destination":"/b","deleted":{"mode":"delete","days":30},"deepCheck":false}]}"#;
        let file = dir.path().join("mixed.secopy");
        std::fs::write(&file, text).unwrap();
        a.open_import(&file).unwrap();
        // mirrors.json can't be written: a directory is in its way.
        std::fs::create_dir_all(dir.path().join("data").join("mirrors.json.tmp")).unwrap();
        let done = a
            .apply_import(&ImportChoices {
                settings: true,
                copy_presets: vec![PresetChoice {
                    index: 0,
                    replace: false,
                }],
                mirror_presets: vec![PresetChoice {
                    index: 0,
                    replace: false,
                }],
            })
            .unwrap();
        assert!(done.failed);
        assert!(
            done.message
                .starts_with("Imported 1 copy preset. The mirror presets couldn't be saved:"),
            "{}",
            done.message
        );
        assert!(
            done.message.ends_with(" The settings weren't imported."),
            "{}",
            done.message
        );
        assert!(lock(&a.settings).write_checksum_file);
        assert_eq!(lock(&a.copy_presets).presets.len(), 2);
        assert!(lock(&a.mirrors).presets.is_empty());
    }

    #[test]
    fn a_file_opened_from_finder_is_taken_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = AppState::new(dir.path().join("data"));
        a.set_opened(PathBuf::from("/Users/me/Sony FX3.secopy"));
        assert_eq!(
            a.take_opened().as_deref(),
            Some("/Users/me/Sony FX3.secopy")
        );
        assert_eq!(a.take_opened(), None);
    }

    /// Review: presets changed after the file was opened aren't overwritten by what the
    /// screen showed before.
    #[test]
    fn a_stale_import_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let file = dir.path().join("one.secopy");
        let id = lock(&a.copy_presets).presets[0].id.clone();
        a.export_copy_preset(&id, &file).unwrap();
        a.open_import(&file).unwrap();
        a.change_copy_presets(|p| {
            p.edit(
                &id,
                crate::store::CopyPresetInput {
                    name: "Sony FX3".into(),
                    source: "/Volumes/CARD_B".into(),
                    include_folder: true,
                    extensions: None,
                },
            )
        })
        .unwrap();
        let replace = ImportChoices {
            copy_presets: vec![PresetChoice {
                index: 0,
                replace: true,
            }],
            ..Default::default()
        };
        assert_eq!(a.apply_import(&replace).unwrap_err(), import_changed());
        assert_eq!(
            lock(&a.copy_presets).get(&id).unwrap().source,
            "/Volumes/CARD_B"
        );
    }

    /// Review: pressing Import twice imports once.
    #[test]
    fn an_import_applies_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let file = dir.path().join("one.secopy");
        let id = lock(&a.copy_presets).presets[0].id.clone();
        a.export_copy_preset(&id, &file).unwrap();
        a.open_import(&file).unwrap();
        let keep = ImportChoices {
            copy_presets: vec![PresetChoice {
                index: 0,
                replace: false,
            }],
            ..Default::default()
        };
        a.apply_import(&keep).unwrap();
        assert_eq!(
            a.apply_import(&keep).unwrap_err(),
            "There is no file to import."
        );
        assert_eq!(lock(&a.copy_presets).presets.len(), 2);
    }

    /// #80: closing hides only with the setting on and something running, never on a quit.
    #[test]
    fn a_quit_is_never_a_hide() {
        let dir = tempfile::tempdir().unwrap();
        let state = queued(dir.path(), &[("A", 1)]);
        assert!(!state.wants_hide(), "nothing runs");
        lock(&state.queue_run).running = true;
        assert!(state.wants_hide());
        state
            .quitting
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(!state.wants_hide(), "⌘Q asks instead");
        assert!(state.wants_hide(), "the quit flag is taken once");
        lock(&state.settings).keep_in_menu_bar = false;
        assert!(!state.wants_hide(), "setting off");
    }
}
