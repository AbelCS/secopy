//! Runs one copy job at a time on its own thread and keeps what the UI asks for: progress
//! twice a second, pages of finished files, the summary and the report (RFD §5.3, §5.4,
//! FR-35).

use std::fs;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use secopy_core::check::{CheckOptions, CheckPlan, CheckReport};
use secopy_core::checksum_file;
use secopy_core::control::JobControl;
use secopy_core::error::FileError;
use secopy_core::job::{
    Event, FileOutcome, FileStatus, JobOptions, JobReport, Progress, SkipReason, Undone, run_job,
    undo,
};
use secopy_core::mirror::{self, Change, Deleted, MirrorPlan};
use secopy_core::plan::Plan;
use secopy_core::report::{JobMeta, Report};
use secopy_core::scan::Selection;
use secopy_core::source::Source;

use crate::dto::{
    ActiveFileView, CheckSummaryView, FinishedRow, JobOutcome, JobPhase, MirrorSummaryView,
    ProgressView, RowStatus, SmallFilesView, SummaryView, UndoneView, count, show,
};
use crate::lock;
use crate::message::Message;
use crate::mirrors::MirrorJob;
use crate::msg;
use crate::say;
use crate::session::Ready;
use crate::store::Settings;

/// Progress reaches the UI twice a second (NFR-5).
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
/// Files at least this big get their own row in the active list (RFD §5.3).
const OWN_ROW: u64 = 8 << 20;
/// Failures listed in the summary; the finished list has all of them.
const FAILURES_SHOWN: usize = 1000;
/// A job whose thread panicked (#69) has no report: one would read as if it had run to the end.
fn no_report() -> Message {
    msg!("errors.report.none")
}

/// The settings a job starts with (RFD §5.5); changing them later doesn't affect it.
#[derive(Debug, Clone)]
pub struct JobSettings {
    pub write_checksum_file: bool,
    pub report_next_to_checksum: bool,
    /// A mirror's removals, done after its copy phase (plan 7); `None` for a copy.
    pub mirror: Option<MirrorRun>,
}

/// What a mirror job does once its files are copied and verified.
#[derive(Debug, Clone)]
pub struct MirrorRun {
    pub plan: Arc<MirrorPlan>,
    /// This run's archive directory (archive mode); `None` deletes.
    pub archive: Option<PathBuf>,
    /// Archived files older than this many days go first, in either mode (#101).
    pub archive_days: u32,
    /// What deleting the whole archive did as this run was started, when the user asked (#101).
    pub archive_deleted: Option<mirror::ArchiveDeleted>,
}

impl JobSettings {
    /// A mirror job: no checksum file (it would be part of the mirror), its removals after.
    pub fn for_mirror(job: &MirrorJob, now: DateTime<Local>) -> Self {
        let archive = match job.plan.options.deleted {
            Deleted::Archive { .. } => Some(mirror::archive_dir(&job.plan.copy.dest, now)),
            Deleted::Delete => None,
        };
        Self {
            write_checksum_file: false,
            report_next_to_checksum: false,
            mirror: Some(MirrorRun {
                plan: job.plan.clone(),
                archive,
                archive_days: job.archive_days,
                archive_deleted: None,
            }),
        }
    }
}

impl Default for JobSettings {
    fn default() -> Self {
        (&Settings::default()).into()
    }
}

impl From<&Settings> for JobSettings {
    fn from(s: &Settings) -> Self {
        Self {
            write_checksum_file: s.write_checksum_file,
            report_next_to_checksum: s.report_next_to_checksum,
            mirror: None,
        }
    }
}

/// Where progress goes: a Tauri channel in the app, a collector in tests.
pub trait ProgressSink: Send + Sync + 'static {
    fn send(&self, view: ProgressView);
}

/// What a job does: copy (a copy or a mirror), or check a directory (plan 8).
pub enum Work {
    Copy {
        ready: Box<Ready>,
        verify: bool,
        settings: JobSettings,
    },
    Check(Arc<CheckPlan>),
}

/// The one job the app runs at a time.
pub struct Jobs {
    reports_dir: PathBuf,
    current: Mutex<Option<Arc<Job>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

struct Job {
    work: Work,
    control: JobControl,
    started: DateTime<Local>,
    clock: Instant,
    /// In the order files finished.
    outcomes: Mutex<Vec<FileOutcome>>,
    /// The engine's latest progress; the final view is built from it.
    last: Mutex<Progress>,
    failed: AtomicU32,
    /// The files under `OWN_ROW` this job writes, and how many of them finished.
    small_total: u32,
    small_done: AtomicU32,
    /// Cancel asked to remove the files already copied (#54).
    remove_copied: AtomicBool,
    done: Mutex<Option<Done>>,
}

struct Done {
    report: JobReport,
    finished: DateTime<Local>,
    /// The report saved in the reports folder, or why it couldn't be.
    report_file: Result<PathBuf, Message>,
    /// Why the report couldn't also be written next to the checksum file.
    next_to_error: Option<Message>,
    /// Why a mirror's own checksum file couldn't be written (the report has it too, with
    /// "the mirror's checksum file:" before it).
    mirror_checksum_error: Option<secopy_core::error::IoFailure>,
    /// What deleting the whole archive did, when the user asked for it (#101).
    archive_deleted: Option<mirror::ArchiveDeleted>,
    /// A mirror's removals, or why nothing was removed (plan 7).
    removals: Option<Result<mirror::Finished, mirror::NotRemoved>>,
    /// What a cancel with "Also remove the files already copied" removed (#54).
    undone: Option<Undone>,
    /// A check's report beyond the files: what nothing lists, checksum file problems.
    check: Option<CheckReport>,
    /// The job's thread panicked: it stopped part way, with no report (#69).
    panicked: bool,
}

impl Jobs {
    /// `reports_dir` is where every job's report is saved (FR-35).
    pub fn new(reports_dir: PathBuf) -> Self {
        Self {
            reports_dir,
            current: Mutex::new(None),
            thread: Mutex::new(None),
        }
    }

    /// Starts copying `ready`. Fails if a job is already running.
    pub fn start(
        &self,
        ready: Ready,
        verify: bool,
        settings: JobSettings,
        sink: impl ProgressSink,
    ) -> Result<(), Message> {
        self.start_work(
            Work::Copy {
                ready: Box::new(ready),
                verify,
                settings,
            },
            sink,
        )
    }

    /// Starts `work`. Fails if a job is already running.
    pub fn start_work(&self, work: Work, sink: impl ProgressSink) -> Result<(), Message> {
        // Held until the job is in place, so two starts can't both get past the check.
        let mut current = lock(&self.current);
        if current.as_ref().is_some_and(|job| job.running()) {
            return Err(msg!("errors.job.alreadyRunning"));
        }
        let small_total = count(match &work {
            Work::Copy { ready, .. } => ready
                .plan
                .files
                .iter()
                .filter(|f| f.action.writes() && f.entry.size < OWN_ROW)
                .count(),
            Work::Check(plan) => plan.files.iter().filter(|f| f.size < OWN_ROW).count(),
        });
        let job = Arc::new(Job {
            work,
            control: JobControl::new(),
            started: Local::now(),
            clock: Instant::now(),
            outcomes: Mutex::new(Vec::new()),
            last: Mutex::new(Progress::default()),
            failed: AtomicU32::new(0),
            small_total,
            small_done: AtomicU32::new(0),
            remove_copied: AtomicBool::new(false),
            done: Mutex::new(None),
        });
        *current = Some(job.clone());
        let reports_dir = self.reports_dir.clone();
        let handle = std::thread::spawn(move || {
            // A bug mustn't leave the window on the Copying screen: the job ends, stopped.
            let ran = std::panic::catch_unwind(AssertUnwindSafe(|| job.run(&sink, &reports_dir)));
            if ran.is_err() {
                job.end_after_panic(&sink);
            }
        });
        *lock(&self.thread) = Some(handle);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.job().is_some_and(|job| job.running())
    }

    pub fn pause(&self) {
        if let Some(job) = self.job() {
            job.control.pause();
        }
    }

    pub fn resume(&self) {
        if let Some(job) = self.job() {
            job.control.resume();
        }
    }

    /// Stops the job: the file in progress is removed; finished files stay (FR-23) unless
    /// `remove_copied` (#54).
    pub fn cancel(&self, remove_copied: bool) {
        if let Some(job) = self.job() {
            job.remove_copied.store(remove_copied, Relaxed);
            job.control.cancel();
        }
    }

    /// Waits for the job's thread to end (after a cancel, or at quit).
    pub fn wait(&self) {
        let handle = lock(&self.thread).take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }

    /// Rows of the finished list, in the order files finished.
    pub fn finished_page(&self, offset: u32, limit: u32, failed_only: bool) -> Vec<FinishedRow> {
        self.job().map_or_else(Vec::new, |job| {
            job.finished_page(offset, limit, failed_only)
        })
    }

    /// The summary once the job has ended; `None` while it runs.
    pub fn summary(&self) -> Option<SummaryView> {
        self.job()?.summary()
    }

    /// The current (or last) job's latest figures, as the progress screen shows them.
    pub fn progress_view(&self) -> Option<ProgressView> {
        self.job().map(|job| {
            let last = lock(&job.last).clone();
            job.progress(&last, false, None)
        })
    }

    /// For the menu bar panel: what the current (or last) job does, from where, and to where.
    pub fn describe(&self) -> Option<(Message, Message, Option<String>)> {
        self.job().map(|job| match &job.work {
            Work::Check(plan) => (
                msg!("menubar.heading.verifying"),
                Message::raw(show(&plan.dir)),
                None,
            ),
            Work::Copy { ready, verify, .. } => {
                let heading = match &ready.mirror {
                    Some(name) => msg!("menubar.heading.mirroring", name = name),
                    None if *verify => msg!("menubar.heading.copyingVerifying"),
                    None => msg!("menubar.heading.copying"),
                };
                (
                    heading,
                    panel_from(&ready.source, &ready.shown),
                    Some(show(&ready.copy_root)),
                )
            }
        })
    }

    /// The current (or last) job is a check (Verify).
    pub fn is_check(&self) -> bool {
        self.job().is_some_and(|job| job.check_plan().is_some())
    }

    /// "Save report…": the text report at `path` and the JSON next to it (FR-35).
    pub fn save_report(&self, path: &Path) -> Result<(), Message> {
        self.job()
            .ok_or_else(|| msg!("errors.report.noneYet"))?
            .save_report(path)
    }

    /// The current (or last) job, kept for the queue summary.
    pub fn current_handle(&self) -> Option<JobHandle> {
        self.job().map(JobHandle)
    }

    /// The failed files of the last job, for "Retry" (RFD §5.4).
    pub fn retry(&self) -> Option<(Source, Selection)> {
        let job = self.job()?;
        let ids: Vec<usize> = lock(&job.outcomes)
            .iter()
            .filter(|o| matches!(o.status, FileStatus::Failed(_)))
            .map(|o| o.id)
            .collect();
        if ids.is_empty() {
            return None;
        }
        let (ready, _, _) = job.copy()?;
        let plan = &ready.plan;
        let all = Selection {
            files: plan.files.iter().map(|f| f.entry.clone()).collect(),
            dirs: plan.dirs.clone(),
            total_bytes: plan.total_bytes(),
            unread: Vec::new(),
        };
        Some((ready.source.clone(), all.subset(&ids)))
    }

    fn job(&self) -> Option<Arc<Job>> {
        lock(&self.current).clone()
    }
}

/// A job the app ran, kept for the queue summary (plan 6).
#[derive(Clone)]
pub struct JobHandle(Arc<Job>);

impl JobHandle {
    pub fn summary(&self) -> Option<SummaryView> {
        self.0.summary()
    }

    pub fn finished_page(&self, offset: u32, limit: u32, failed_only: bool) -> Vec<FinishedRow> {
        self.0.finished_page(offset, limit, failed_only)
    }

    pub fn save_report(&self, path: &Path) -> Result<(), Message> {
        self.0.save_report(path)
    }
}

impl Job {
    /// A copy's setup; `None` for a check.
    fn copy(&self) -> Option<(&Ready, bool, &JobSettings)> {
        match &self.work {
            Work::Copy {
                ready,
                verify,
                settings,
            } => Some((ready, *verify, settings)),
            Work::Check(_) => None,
        }
    }

    fn check_plan(&self) -> Option<&CheckPlan> {
        match &self.work {
            Work::Check(plan) => Some(plan),
            Work::Copy { .. } => None,
        }
    }

    fn settings(&self) -> Option<&JobSettings> {
        self.copy().map(|(_, _, settings)| settings)
    }

    fn mirror(&self) -> Option<&MirrorRun> {
        self.settings().and_then(|s| s.mirror.as_ref())
    }

    /// A check always reads every file back, like Copy & Verify.
    fn verify(&self) -> bool {
        self.copy().is_none_or(|(_, verify, _)| verify)
    }

    fn label(&self) -> String {
        match &self.work {
            Work::Copy { ready, .. } => ready.label.clone(),
            Work::Check(plan) => format!("Verify · {}", show(&plan.dir)),
        }
    }

    fn root(&self) -> &Path {
        match &self.work {
            Work::Copy { ready, .. } => &ready.copy_root,
            Work::Check(plan) => &plan.dir,
        }
    }

    /// The files and bytes the job works through.
    fn totals(&self) -> (usize, u64) {
        match &self.work {
            Work::Copy { ready, .. } => (ready.plan.files.len(), ready.plan.bytes_to_write()),
            Work::Check(plan) => (plan.files.len(), plan.total_bytes),
        }
    }

    /// Rows of the finished list, in the order files finished.
    fn finished_page(&self, offset: u32, limit: u32, failed_only: bool) -> Vec<FinishedRow> {
        let outcomes = lock(&self.outcomes);
        outcomes
            .iter()
            .filter(|o| !failed_only || matches!(o.status, FileStatus::Failed(_)))
            .skip(offset as usize)
            .take(limit as usize)
            .map(|o| row(o, self.check_plan()))
            .collect()
    }

    /// The summary once the job has ended; `None` while it runs.
    fn summary(&self) -> Option<SummaryView> {
        let job = self;
        let done = lock(&job.done);
        let done = done.as_ref()?;
        let report = job.report(done);
        let outcomes = lock(&job.outcomes);
        let c = &report.counts;
        let mirror = self.mirror().map(|m| mirror_summary(m, done, &outcomes));
        let check = match (self.check_plan(), &done.check) {
            (Some(plan), Some(checked)) => {
                let n = checked.counts();
                Some(CheckSummaryView {
                    intact: count(n.intact),
                    changed: count(n.changed),
                    missing: count(n.missing),
                    failed: count(n.failed),
                    not_checked: count(checked.not_checked.len()),
                    checksum_files: count(plan.checksum_files.len()),
                    problems: checked
                        .problems
                        .iter()
                        .take(FAILURES_SHOWN)
                        .map(say::check_problem)
                        .collect(),
                    more_problems: (checked.problems.len() > FAILURES_SHOWN)
                        .then(|| count(checked.problems.len() - FAILURES_SHOWN)),
                })
            }
            _ => None,
        };
        let removal_failed = mirror
            .as_ref()
            .is_some_and(|m| !m.removal_failures.is_empty());
        Some(SummaryView {
            outcome: if done.report.fatal.is_some() || done.panicked {
                JobOutcome::Stopped
            } else if done.report.cancelled {
                JobOutcome::Cancelled
            } else if c.failed > 0
                || removal_failed
                || mirror
                    .as_ref()
                    .is_some_and(|m| m.archive_not_deleted.is_some())
                || !done.report.unread.is_empty()
                || done.report.checksum_error.is_some()
                || done.report.durability_error.is_some()
                || !done.report.dir_errors.is_empty()
                || check.as_ref().is_some_and(|c| !c.problems.is_empty())
            {
                JobOutcome::Failures
            } else {
                JobOutcome::Complete
            },
            stopped_because: match &done.report.fatal {
                _ if done.panicked => Some(say::internal()),
                fatal => fatal.as_ref().map(say::fatal),
            },
            verify: job.verify(),
            files: count(c.files),
            copied: count(c.copied),
            verified: count(c.verified),
            skipped_identical: count(c.skipped_identical),
            skipped_different: count(c.skipped_different),
            failed: count(c.failed),
            unread: count(done.report.unread.len()),
            durability_error: done.report.durability_error.as_ref().map(say::io_failure),
            dir_errors: count(done.report.dir_errors.len()),
            not_started: count(c.not_started),
            bytes_written: c.bytes_written,
            millis: done.report.elapsed.as_millis() as u64,
            // What couldn't be read first: it's the source, before any file.
            failures: done
                .report
                .unread
                .iter()
                .enumerate()
                .map(|(i, p)| FinishedRow {
                    id: count(i),
                    path: show(&p.path),
                    final_path: show(&p.path),
                    size: 0,
                    millis: 0,
                    hash: None,
                    status: RowStatus::Failed,
                    reason: Some(msg!("summary.reason.unread", why = say::scan_why(&p.kind))),
                })
                .chain(done.report.dir_errors.iter().map(|(rel, why)| FinishedRow {
                    id: 0,
                    path: show(rel),
                    final_path: show(rel),
                    size: 0,
                    millis: 0,
                    hash: None,
                    status: RowStatus::Failed,
                    reason: Some(msg!(
                        "summary.reason.dirNotCreated",
                        why = say::io_failure(why)
                    )),
                }))
                .chain(
                    outcomes
                        .iter()
                        .filter(|o| matches!(o.status, FileStatus::Failed(_)))
                        .map(|o| row(o, self.check_plan())),
                )
                .take(FAILURES_SHOWN)
                .collect(),
            finished: count(outcomes.len()),
            copy_root: show(job.root()),
            checksum_file: done.report.checksum_file.as_deref().map(show),
            checksum_error: checksum_error(
                done.mirror_checksum_error.as_ref(),
                done.report.checksum_error.as_ref(),
            ),
            // A check writes no checksum file: none was turned off.
            checksum_off: job.copy().is_some()
                && !job.settings().is_some_and(|s| s.write_checksum_file),
            report_file: done.report_file.as_deref().ok().map(show),
            report_errors: done
                .report_file
                .as_ref()
                .err()
                .cloned()
                .into_iter()
                .chain(done.next_to_error.clone())
                .collect(),
            mirror,
            undone: done.undone.as_ref().map(|u| UndoneView {
                removed: count(u.removed),
                not_restored: count(u.not_restored),
                failed: count(u.failed.len()),
            }),
            check,
        })
    }

    /// "Save report…": the text report at `path` and the JSON next to it (FR-35).
    fn save_report(&self, path: &Path) -> Result<(), Message> {
        let job = self;
        let done = lock(&job.done);
        let done = done.as_ref().ok_or_else(|| msg!("errors.report.running"))?;
        if done.panicked {
            return Err(no_report());
        }
        let report = job.report(done);
        // The JSON goes next to the text the save panel named: never over a file of the
        // user's (#116), only over a Secopy report.
        let json = path.with_extension("json");
        match fs::read(&json) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            // Only the pair of the text the panel asked to replace.
            Ok(bytes) if path.is_file() && is_report(&bytes) => {}
            _ => return Err(msg!("errors.report.jsonTaken", path = &json)),
        }
        // Each through a temporary name, so a failure never leaves half a report.
        crate::transfer::write_file(path, &report.to_text())?;
        crate::transfer::write_file(&json, &report.to_json())
    }

    fn running(&self) -> bool {
        lock(&self.done).is_none()
    }

    fn run(&self, sink: &impl ProgressSink, reports_dir: &Path) {
        match &self.work {
            Work::Check(plan) => self.run_check(plan, sink, reports_dir),
            Work::Copy { ready, .. } => self.run_copy(ready, sink, reports_dir),
        }
    }

    /// Ends a job whose thread panicked: stopped, with the files that finished and no report,
    /// then the Done view the window waits for (#69).
    fn end_after_panic(&self, sink: &impl ProgressSink) {
        let finished = lock(&self.outcomes).len();
        let report = JobReport {
            outcomes: Vec::new(),
            not_started: (self.totals().0.saturating_sub(finished)) as u64,
            checksum_file: None,
            checksum_error: None,
            // A check writes no checksum file: none was turned off.
            checksum_off: self.copy().is_some()
                && !self.settings().is_some_and(|s| s.write_checksum_file),
            cache_bypass: None,
            removed_partials: 0,
            fatal: None,
            cancelled: false,
            elapsed: self.clock.elapsed(),
            created_dirs: Vec::new(),
            unread: Vec::new(),
            durability_error: None,
            dir_errors: Vec::new(),
        };
        // A check stays a check: what was read so far, counted.
        let check = self.check_plan().map(|plan| CheckReport {
            job: JobReport {
                outcomes: lock(&self.outcomes).clone(),
                ..report.clone()
            },
            not_checked: plan.not_checked.clone(),
            problems: plan.problems.clone(),
        });
        let done = Done {
            report,
            finished: Local::now(),
            report_file: Err(no_report()),
            next_to_error: None,
            mirror_checksum_error: None,
            archive_deleted: None,
            removals: None,
            undone: None,
            check,
            panicked: true,
        };
        let last = lock(&self.last).clone();
        let mut ended = lock(&self.done);
        // Past its end (sending the last view), the job stands as it ended.
        let fatal = ended.is_none().then(say::internal);
        ended.get_or_insert(done);
        drop(ended);
        let full = AssertUnwindSafe(|| {
            let mut view = self.progress(&last, true, fatal.clone());
            view.files_done = count(finished);
            sink.send(view);
        });
        if std::panic::catch_unwind(full).is_err() {
            // The same bug again: a Done view built from nothing it could have broken.
            let bare = ProgressView {
                phase: JobPhase::Done,
                files_done: count(finished),
                fatal,
                ..ProgressView::default()
            };
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| sink.send(bare)));
        }
    }

    fn run_check(&self, plan: &CheckPlan, sink: &impl ProgressSink, reports_dir: &Path) {
        let opts = CheckOptions {
            progress_interval: PROGRESS_INTERVAL,
            ..CheckOptions::default()
        };
        let checked = secopy_core::check::run(plan, &opts, &self.control, &|event| match event {
            Event::Progress(p) => {
                sink.send(self.progress(&p, false, None));
                *lock(&self.last) = p;
            }
            Event::FileFinished(o) => {
                if matches!(o.status, FileStatus::Failed(_)) {
                    self.failed.fetch_add(1, Relaxed);
                }
                if plan.files[o.id].size < OWN_ROW {
                    self.small_done.fetch_add(1, Relaxed);
                }
                lock(&self.outcomes).push(o);
            }
        });
        let mut done = Done {
            finished: Local::now(),
            report_file: Err(no_report()),
            next_to_error: None,
            mirror_checksum_error: None,
            archive_deleted: None,
            report: checked.job.clone(),
            removals: None,
            undone: None,
            check: Some(checked),
            panicked: false,
        };
        done.report_file = self.save(&done, reports_dir);
        let last = lock(&self.last).clone();
        let mut view = self.progress(&last, true, None);
        view.files_done = count(done.report.outcomes.len());
        *lock(&self.done) = Some(done);
        sink.send(view);
    }

    fn run_copy(&self, ready: &Ready, sink: &impl ProgressSink, reports_dir: &Path) {
        let settings = self.settings().expect("a copy has settings");
        let mirroring = settings.mirror.as_ref();
        // Archive runs older than the preset keeps them go first (FR-49), in Delete mode too:
        // what was archived before a switch still goes when due (#101).
        let archive_deleted = mirroring.and_then(|m| m.archive_deleted.clone());
        if let Some(m) = mirroring {
            mirror::clean_archives(&m.plan.copy.dest, m.archive_days, Local::now());
        }
        let opts = JobOptions {
            verify: self.verify(),
            write_checksum_file: settings.write_checksum_file,
            progress_interval: PROGRESS_INTERVAL,
            archive_replaced: mirroring.and_then(|m| m.archive.clone()),
            ..JobOptions::default()
        };
        let plan: &Plan = &ready.plan;
        let mut report = run_job(plan, &opts, &self.control, &|event| match event {
            Event::Progress(p) => {
                sink.send(self.progress(&p, false, None));
                *lock(&self.last) = p;
            }
            Event::FileFinished(o) => {
                if matches!(o.status, FileStatus::Failed(_)) {
                    self.failed.fetch_add(1, Relaxed);
                }
                let file = &plan.files[o.id];
                if file.action.writes() && file.entry.size < OWN_ROW {
                    self.small_done.fetch_add(1, Relaxed);
                }
                lock(&self.outcomes).push(o);
            }
        });
        // Cancel with "Also remove the files already copied": the destination as it was.
        let undone = (report.cancelled && self.remove_copied.load(Relaxed)).then(|| {
            let last = lock(&self.last).clone();
            let mut view = self.progress(&last, false, None);
            view.phase = JobPhase::Removing;
            view.undoing = true;
            sink.send(view);
            let undone = undo(plan, &report, mirroring.and_then(|m| m.archive.as_deref()));
            report.checksum_file = None;
            undone
        });
        // A mirror removes what's gone from its origin, only after a clean copy phase.
        let mut mirror_checksum = None;
        let removals = mirroring.map(|m| {
            if !report.cancelled {
                let last = lock(&self.last).clone();
                let mut view = self.progress(&last, false, None);
                view.phase = JobPhase::Removing;
                view.removing = count(m.plan.removals.len());
                view.archiving = m.archive.is_some();
                sink.send(view);
            }
            let finished = mirror::finish(&m.plan, &report, m.archive.as_deref());
            // The mirror's checksum file: with the removals after a clean run (plan 8), with what
            // was verified after any other (#114), and not after an undo.
            if undone.is_none()
                && let Err(e) = mirror::write_checksums(&m.plan, &report, finished.as_ref().ok())
            {
                // The report says whose checksum file it was, in English; the UI in its words.
                report.checksum_error = Some(secopy_core::error::IoFailure {
                    kind: e.kind(),
                    message: format!("the mirror's checksum file: {e}"),
                });
                mirror_checksum = Some(e.into());
            }
            finished
        });
        let mut done = Done {
            finished: Local::now(),
            report_file: Err(no_report()),
            next_to_error: None,
            mirror_checksum_error: None,
            archive_deleted: None,
            report,
            removals,
            undone,
            check: None,
            panicked: false,
        };
        done.mirror_checksum_error = mirror_checksum;
        done.archive_deleted = archive_deleted;
        done.report_file = self.save(&done, reports_dir);
        if settings.report_next_to_checksum
            && let Some(checksum) = &done.report.checksum_file
        {
            done.next_to_error = self
                .report(&done)
                .write_next_to(checksum)
                .err()
                .map(|e| msg!("errors.report.nextToChecksum", why = say::io_error(&e)));
        }
        let fatal = done.report.fatal.as_ref().map(say::fatal);
        // The engine's last progress, so a stopped job's bars stay where it stopped.
        let last = lock(&self.last).clone();
        let mut view = self.progress(&last, true, fatal);
        view.files_done = count(done.report.outcomes.len());
        view.files_skipped = count(done.report.skipped().count());
        *lock(&self.done) = Some(done);
        sink.send(view);
    }

    fn progress(&self, p: &Progress, finished: bool, fatal: Option<Message>) -> ProgressView {
        let (total_files, total_bytes) = self.totals();
        let mut active = Vec::new();
        // Small files are one steady row below; only big ones are worth a bar each.
        for f in p.active.iter().filter(|f| f.size >= OWN_ROW) {
            active.push(ActiveFileView {
                id: count(f.id),
                name: f
                    .rel
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: show(&f.rel),
                verifying: f.phase == secopy_core::job::Phase::Verifying,
                size: f.size,
                bytes_done: f.bytes_done,
            });
        }
        let copying = p
            .active
            .iter()
            .any(|f| f.phase == secopy_core::job::Phase::Copying);
        ProgressView {
            phase: if finished {
                JobPhase::Done
            } else if self.check_plan().is_some()
                || (self.verify() && !copying && p.copied_bytes >= total_bytes)
            {
                JobPhase::Verifying
            } else {
                JobPhase::Copying
            },
            elapsed_ms: self.clock.elapsed().as_millis() as u64,
            paused: p.paused,
            verify: self.verify(),
            total_files: count(total_files),
            total_bytes,
            copied_bytes: p.copied_bytes,
            verified_bytes: p.verified_bytes,
            files_done: count(p.files_done),
            files_skipped: count(p.files_skipped),
            files_failed: self.failed.load(Relaxed),
            active,
            small_files: (self.small_total > 0).then(|| SmallFilesView {
                done: self.small_done.load(Relaxed).min(self.small_total),
                total: self.small_total,
            }),
            fatal,
            removing: 0,
            archiving: false,
            undoing: false,
        }
    }

    fn report(&self, done: &Done) -> Report {
        let meta = JobMeta {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            source: self.label(),
            verify: self.verify(),
            started: self.started,
            finished: done.finished,
        };
        let mut job_report = done.report.clone();
        job_report.outcomes = lock(&self.outcomes).clone();
        let ready = match (&self.work, &done.check) {
            (Work::Check(plan), Some(checked)) => return Report::for_check(plan, checked, &meta),
            (Work::Check(plan), None) => {
                let checked = CheckReport {
                    job: job_report,
                    not_checked: plan.not_checked.clone(),
                    problems: plan.problems.clone(),
                };
                return Report::for_check(plan, &checked, &meta);
            }
            (Work::Copy { ready, .. }, _) => ready,
        };
        let report = Report::new(&ready.plan, &job_report, &meta);
        match (&done.removals, self.mirror()) {
            (Some(removals), Some(m)) => {
                report.with_mirror(mirror::report_part(removals, m.archive.is_some()))
            }
            _ => report,
        }
    }

    /// Saves the report in the app's data folder, named like the checksum file (FR-35).
    fn save(&self, done: &Done, reports_dir: &Path) -> Result<PathBuf, Message> {
        let failed = |e: std::io::Error| {
            msg!(
                "errors.report.dir",
                path = reports_dir,
                why = say::io_error(&e)
            )
        };
        fs::create_dir_all(reports_dir).map_err(failed)?;
        let stem = match done
            .report
            .checksum_file
            .as_deref()
            .and_then(Path::file_stem)
        {
            Some(stem) => stem.to_string_lossy().into_owned(),
            None => checksum_file::file_name(self.started).replace(".xxh64", ""),
        };
        let text = self
            .report(done)
            .write(reports_dir, &stem)
            .map(|(text, _)| text)
            .map_err(failed)?;
        if let Some(undone) = &done.undone {
            append_undone(&text, undone).map_err(failed)?;
        }
        Ok(text)
    }
}

/// `bytes` is a report Secopy saved: its JSON, with the app's version and a result.
fn is_report(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .is_ok_and(|v| v.get("app_version").is_some() && v.get("result").is_some())
}

/// A mirror's figures: files copied as new or updated, and its removals.
fn mirror_summary(m: &MirrorRun, done: &Done, outcomes: &[FileOutcome]) -> MirrorSummaryView {
    let written: std::collections::HashSet<usize> = outcomes
        .iter()
        .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
        .map(|o| o.id)
        .collect();
    let done_as = |new: bool| {
        m.plan
            .changes
            .iter()
            .filter(|(i, c)| (*c == Change::New) == new && written.contains(i))
            .count()
    };
    let (removed, removal_failures, nothing_removed) = match &done.removals {
        Some(Ok(finished)) => (
            finished
                .removals
                .iter()
                .filter(|r| r.result.is_ok())
                .count(),
            finished
                .removals
                .iter()
                .enumerate()
                .filter_map(|(i, r)| {
                    r.result.as_ref().err().map(|e| FinishedRow {
                        id: count(i),
                        path: show(&r.rel),
                        final_path: show(&r.rel),
                        size: 0,
                        millis: 0,
                        hash: None,
                        status: RowStatus::Failed,
                        reason: Some(msg!("summary.reason.notRemoved", why = say::removal(e))),
                    })
                })
                .collect(),
            None,
        ),
        Some(Err(why)) => (0, Vec::new(), Some(say::not_removed(why))),
        None => (0, Vec::new(), None),
    };
    let archive_not_deleted = done
        .archive_deleted
        .as_ref()
        .and_then(|a| say::archive_not_deleted(a, m.archive_days));
    MirrorSummaryView {
        archive_not_deleted,
        new: count(done_as(true)),
        updated: count(done_as(false)),
        removed: count(removed),
        archived: m.archive.is_some(),
        removal_failures,
        nothing_removed,
    }
}

/// What Cancel's "Also remove the files already copied" did, at the end of the text report.
fn append_undone(text: &Path, u: &Undone) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = fs::OpenOptions::new().append(true).open(text)?;
    writeln!(
        out,
        "\nRemoved after cancelling: {} copied, {} put back from the archive, {} replaced files not put back",
        u.removed, u.restored, u.not_restored
    )?;
    for (path, why) in &u.failed {
        writeln!(out, "  {} — NOT REMOVED: {why}", path.display())?;
    }
    Ok(())
}

/// Where the menu bar panel says a copy is from: a directory by its path (a retry too), picked
/// files as the UI names them ("3 files").
fn panel_from(source: &Source, shown: &Message) -> Message {
    match source {
        Source::Directory { path, .. } => Message::raw(show(path)),
        Source::Files(_) => shown.clone(),
    }
}

/// The summary's checksum error: a mirror's own checksum file says so.
fn checksum_error(
    mirror: Option<&secopy_core::error::IoFailure>,
    report: Option<&secopy_core::error::IoFailure>,
) -> Option<Message> {
    match (mirror, report) {
        (Some(e), _) => Some(msg!("summary.mirrorChecksum", why = say::io_failure(e))),
        (None, e) => e.map(say::io_failure),
    }
}

/// One row of the finished list; `check` for a check's files (intact, changed, missing).
/// A finished file as a row; `check` is the plan when the job is a check.
fn row(o: &FileOutcome, check: Option<&CheckPlan>) -> FinishedRow {
    let (status, mut reason) = match &o.status {
        FileStatus::Copied => (RowStatus::Copied, None),
        FileStatus::Verified if check.is_some() => (RowStatus::Intact, None),
        FileStatus::Verified => (RowStatus::Verified, None),
        FileStatus::Failed(e @ FileError::Changed { .. }) => {
            (RowStatus::Changed, Some(say::file_error(e)))
        }
        FileStatus::Failed(FileError::Missing) => (RowStatus::Missing, None),
        FileStatus::Skipped(SkipReason::Identical) => {
            (RowStatus::Skipped, Some(msg!("summary.reason.identical")))
        }
        FileStatus::Skipped(SkipReason::Differs) => {
            (RowStatus::Skipped, Some(msg!("summary.reason.differs")))
        }
        FileStatus::Failed(e) => (RowStatus::Failed, Some(say::file_error(e))),
        FileStatus::Cancelled => (RowStatus::Cancelled, None),
    };
    // A check's problem file says which checksum file listed it (#69).
    if let (Some(plan), FileStatus::Failed(_)) = (check, &o.status)
        && let Some(file) = plan.files.get(o.id)
    {
        reason = Some(match reason {
            Some(why) => msg!("errors.check.listedIn", why = why, file = &file.from),
            None => msg!("errors.check.listedInOnly", file = &file.from),
        });
    }
    FinishedRow {
        id: count(o.id),
        path: show(&o.rel),
        final_path: show(&o.final_rel),
        size: o.size,
        millis: o.elapsed.as_millis() as u64,
        hash: o.hash.map(secopy_core::hash::to_hex),
        status,
        reason,
    }
}

/// A path on the source's drive: the directory, or the first file.
pub(crate) fn source_path(s: &Source) -> Option<PathBuf> {
    match s {
        Source::Directory { path, .. } => Some(path.clone()),
        Source::Files(files) => files.first().cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::En;
    use crate::session::{Change, Session, scan_source};

    /// Review: the menu bar says where a retry copies from, not "Retry: 3 failed files".
    #[test]
    fn the_panel_names_a_directory_source_by_its_path() {
        let retry = msg!("copy.picked.retry", count = 3u32);
        let dir = Source::Directory {
            path: PathBuf::from("/Volumes/A/CLIP"),
            mode: secopy_core::source::DirMode::FolderItself,
        };
        assert_eq!(panel_from(&dir, &retry), "/Volumes/A/CLIP");
        let files = Source::Files(vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(panel_from(&files, &retry), "Retry: 3 failed files");
    }

    /// Review: a mirror's checksum file that couldn't be written says it was the mirror's.
    #[test]
    fn a_mirrors_checksum_error_says_whose() {
        let io = secopy_core::error::IoFailure {
            kind: std::io::ErrorKind::Other,
            message: "Input/output error (os error 5)".into(),
        };
        assert_eq!(
            checksum_error(Some(&io), None).unwrap(),
            "the mirror's checksum file: Input/output error (os error 5)"
        );
        assert_eq!(
            checksum_error(None, Some(&io)).unwrap(),
            "Input/output error (os error 5)"
        );
        assert!(checksum_error(None, None).is_none());
    }

    #[derive(Clone, Default)]
    struct Collect(Arc<Mutex<Vec<ProgressView>>>);

    impl ProgressSink for Collect {
        fn send(&self, view: ProgressView) {
            self.0.lock().unwrap().push(view);
        }
    }

    impl Collect {
        fn last(&self) -> ProgressView {
            self.0.lock().unwrap().last().cloned().unwrap()
        }
    }

    struct Fixture {
        dir: tempfile::TempDir,
        dest: PathBuf,
        session: Session,
        jobs: Jobs,
    }

    /// CARD/ with `n` files of `size` bytes, picked, and dest/ chosen.
    fn fixture(n: usize, size: usize) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        for i in 0..n {
            fs::write(card.join(format!("C{i:04}.mov")), vec![i as u8; size]).unwrap();
        }
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        let mut session = Session::new();
        let pending = session
            .begin(Change::Pick(vec![card.clone()]))
            .ok()
            .unwrap();
        let scanned = scan_source(&pending.source);
        session.finish_scan(pending, scanned);
        session.set_destination(Some(dest.clone()));
        let jobs = Jobs::new(dir.path().join("reports"));
        Fixture {
            dir,
            dest,
            session,
            jobs,
        }
    }

    fn run_with(f: &Fixture, verify: bool, settings: JobSettings) -> Collect {
        let sink = Collect::default();
        f.jobs
            .start(f.session.ready().unwrap(), verify, settings, sink.clone())
            .unwrap();
        f.jobs.wait();
        sink
    }

    fn run(f: &Fixture, verify: bool) -> Collect {
        run_with(f, verify, JobSettings::default())
    }

    #[test]
    fn a_job_copies_verifies_and_ends_with_a_done_view() {
        let f = fixture(5, 1000);
        let sink = run(&f, true);
        let last = sink.last();
        assert_eq!(last.phase, JobPhase::Done);
        assert_eq!(last.files_done, 5);
        assert_eq!(last.verified_bytes, 5000);
        assert!(!f.jobs.is_running());
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Complete);
        assert_eq!((s.files, s.verified, s.failed), (5, 5, 0));
        assert_eq!(s.finished, 5, "rows in the finished list");
        assert_eq!(s.copy_root, show(&f.dest.join("CARD")));
        assert!(s.checksum_file.is_some());
        assert!(f.dest.join("CARD/C0004.mov").is_file());
    }

    #[test]
    fn the_report_is_saved_in_the_reports_folder() {
        let f = fixture(2, 10);
        run(&f, false);
        let report = PathBuf::from(f.jobs.summary().unwrap().report_file.unwrap());
        assert!(report.starts_with(f.dir.path().join("reports")));
        assert!(
            fs::read_to_string(&report)
                .unwrap()
                .contains("Result:       complete")
        );
        assert!(report.with_extension("json").is_file());
    }

    #[test]
    fn finished_rows_come_in_pages() {
        let f = fixture(7, 10);
        run(&f, false);
        let first = f.jobs.finished_page(0, 5, false);
        let rest = f.jobs.finished_page(5, 5, false);
        assert_eq!((first.len(), rest.len()), (5, 2));
        assert!(
            first
                .iter()
                .all(|r| r.status == RowStatus::Copied && r.hash.is_some())
        );
        assert!(f.jobs.finished_page(0, 10, true).is_empty(), "no failures");
    }

    #[test]
    fn save_report_writes_text_and_json() {
        let f = fixture(1, 10);
        run(&f, true);
        let path = f.dir.path().join("mine.txt");
        f.jobs.save_report(&path).unwrap();
        assert!(fs::read_to_string(&path).unwrap().starts_with("Secopy "));
        assert!(path.with_extension("json").is_file());
        // Again over the same name: the panel asked about the text; the JSON next to it is
        // Secopy's report, replaced too.
        f.jobs.save_report(&path).unwrap();
        // A file of the user's where the JSON would go is never replaced (#116).
        let other = f.dir.path().join("shoot.txt");
        fs::write(other.with_extension("json"), b"{\"mine\": true}").unwrap();
        assert!(f.jobs.save_report(&other).is_err());
        assert_eq!(
            fs::read(other.with_extension("json")).unwrap(),
            b"{\"mine\": true}"
        );
        assert!(!other.exists(), "nothing written");
        // Another Secopy report where the JSON would go, with no text beside it: not the pair
        // the panel asked about, so it stays too.
        let lone = f.dir.path().join("lone.txt");
        fs::copy(path.with_extension("json"), lone.with_extension("json")).unwrap();
        assert!(f.jobs.save_report(&lone).is_err());
        assert!(!lone.exists());
    }

    #[test]
    fn cancelling_ends_the_job_as_cancelled() {
        let f = fixture(200, 50_000);
        let sink = Collect::default();
        f.jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                sink.clone(),
            )
            .unwrap();
        f.jobs.cancel(false);
        f.jobs.wait();
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Cancelled);
        let last = sink.last();
        assert_eq!(last.phase, JobPhase::Done);
        assert!(
            last.copied_bytes < last.total_bytes && last.verified_bytes < last.total_bytes,
            "a cancelled job doesn't show full bars: {} / {} / {}",
            last.copied_bytes,
            last.verified_bytes,
            last.total_bytes
        );
    }

    /// #58: what the scan couldn't read wasn't copied: the summary says so and isn't green.
    #[test]
    fn items_the_scan_couldnt_read_are_failures_in_the_summary() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(card.join("locked")).unwrap();
        fs::write(card.join("a.mov"), b"a").unwrap();
        fs::write(card.join("locked/b.mov"), b"b").unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        fs::set_permissions(card.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
        let mut session = Session::new();
        let pending = session
            .begin(Change::Pick(vec![card.clone()]))
            .ok()
            .unwrap();
        let scanned = scan_source(&pending.source);
        session.finish_scan(pending, scanned);
        fs::set_permissions(card.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
        session.set_destination(Some(dest.clone()));
        let jobs = Jobs::new(dir.path().join("reports"));
        jobs.start(
            session.ready().unwrap(),
            true,
            JobSettings::default(),
            Collect::default(),
        )
        .unwrap();
        jobs.wait();
        let s = jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Failures);
        assert_eq!((s.unread, s.failed), (1, 0));
        let row = &s.failures[0];
        assert!(row.path.ends_with("locked"), "{row:?}");
        assert!(
            row.reason
                .en()
                .as_deref()
                .unwrap()
                .starts_with("Couldn't be read"),
            "{row:?}"
        );
    }

    /// #58: a checksum file that couldn't be written isn't a complete job.
    #[test]
    fn a_checksum_file_that_couldnt_be_written_is_not_complete() {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture(2, 10);
        fs::create_dir_all(f.dest.join("CARD")).unwrap();
        fs::set_permissions(&f.dest, fs::Permissions::from_mode(0o555)).unwrap();
        run(&f, true);
        fs::set_permissions(&f.dest, fs::Permissions::from_mode(0o755)).unwrap();
        let s = f.jobs.summary().unwrap();
        assert!(s.checksum_error.is_some());
        assert_eq!(s.outcome, JobOutcome::Failures);
    }

    /// #57: small files are one steady row for the whole job: its count never goes back.
    #[test]
    fn small_files_are_one_steady_row() {
        let f = fixture(40, 1000);
        let sink = run(&f, true);
        let views = sink.0.lock().unwrap().clone();
        let small: Vec<(u32, u32)> = views
            .iter()
            .map(|v| {
                let s = v.small_files.as_ref().expect("shown the whole time");
                (s.done, s.total)
            })
            .collect();
        assert!(small.iter().all(|&(_, total)| total == 40), "{small:?}");
        assert!(
            small.windows(2).all(|w| w[0].0 <= w[1].0),
            "never goes back: {small:?}"
        );
        assert_eq!(small.last(), Some(&(40, 40)));
    }

    /// #54: Cancel with "Also remove the files already copied".
    #[test]
    fn cancelling_can_remove_what_was_copied() {
        let f = fixture(200, 50_000);
        let sink = Collect::default();
        f.jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                sink.clone(),
            )
            .unwrap();
        while f.jobs.finished_page(0, 1, false).is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        f.jobs.pause();
        f.jobs.cancel(true);
        f.jobs.wait();
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Cancelled);
        let undone = s.undone.expect("what was removed");
        assert!(undone.removed >= 1, "{undone:?}");
        assert_eq!((undone.not_restored, undone.failed), (0, 0));
        assert_eq!(s.checksum_file, None);
        // While it removes them, the window says so, with nothing left to pause or cancel.
        let views = sink.0.lock().unwrap().clone();
        let shown = |v: &ProgressView| v.phase == JobPhase::Removing && v.undoing;
        assert!(views.iter().any(shown), "the removal is shown");
        assert_eq!(views.last().unwrap().phase, JobPhase::Done);
        assert_eq!(
            fs::read_dir(&f.dest).unwrap().count(),
            0,
            "the destination is as it was"
        );
        let report = fs::read_to_string(s.report_file.unwrap()).unwrap();
        assert!(report.contains("Removed after cancelling"), "{report}");
    }

    #[test]
    fn cancelling_keeps_what_was_copied_by_default() {
        let f = fixture(200, 50_000);
        f.jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                Collect::default(),
            )
            .unwrap();
        while f.jobs.finished_page(0, 1, false).is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        f.jobs.pause();
        f.jobs.cancel(false);
        f.jobs.wait();
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.undone, None);
        assert!(f.dest.join("CARD").read_dir().unwrap().count() >= 1);
    }

    #[test]
    fn a_report_that_cant_be_saved_says_why() {
        let mut f = fixture(1, 10);
        let blocked = f.dir.path().join("not-a-folder");
        fs::write(&blocked, b"").unwrap();
        f.jobs = Jobs::new(blocked);
        run(&f, false);
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.report_file, None);
        assert!(!s.report_errors.is_empty());
    }

    #[test]
    fn two_starts_at_once_run_one_job() {
        for _ in 0..5 {
            let f = fixture(2, 10);
            let barrier = std::sync::Barrier::new(2);
            let started = std::thread::scope(|s| {
                let tries: Vec<_> = (0..2)
                    .map(|_| {
                        s.spawn(|| {
                            let ready = f.session.ready().unwrap();
                            barrier.wait();
                            f.jobs
                                .start(ready, true, JobSettings::default(), Collect::default())
                                .is_ok()
                        })
                    })
                    .collect();
                tries
                    .into_iter()
                    .map(|t| t.join().unwrap())
                    .filter(|ok| *ok)
                    .count()
            });
            f.jobs.wait();
            assert_eq!(started, 1);
        }
    }

    #[test]
    fn a_second_job_cannot_start_while_one_runs() {
        let f = fixture(200, 50_000);
        f.jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                Collect::default(),
            )
            .unwrap();
        f.jobs.pause();
        let err = f
            .jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                Collect::default(),
            )
            .unwrap_err();
        assert!(err.contains("already running"));
        f.jobs.cancel(false);
        f.jobs.wait();
    }

    #[test]
    fn a_paused_job_reports_paused_and_finishes_after_resume() {
        let f = fixture(50, 20_000);
        let sink = Collect::default();
        f.jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                sink.clone(),
            )
            .unwrap();
        f.jobs.pause();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !sink.0.lock().unwrap().iter().any(|v| v.paused) {
            assert!(Instant::now() < deadline, "no paused progress");
            std::thread::sleep(Duration::from_millis(20));
        }
        f.jobs.resume();
        f.jobs.wait();
        assert_eq!(f.jobs.summary().unwrap().outcome, JobOutcome::Complete);
    }

    #[test]
    fn retry_offers_only_the_failed_files() {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture(3, 10);
        let locked = f.dir.path().join("CARD/C0001.mov");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(&locked).is_ok() {
            return; // running as root: permissions are not enforced
        }
        run(&f, true);
        let s = f.jobs.summary().unwrap();
        assert_eq!((s.outcome, s.failed), (JobOutcome::Failures, 1));
        assert_eq!(
            s.failures[0].path,
            show(&Path::new("CARD").join("C0001.mov"))
        );
        assert!(
            s.failures[0]
                .reason
                .en()
                .as_deref()
                .unwrap()
                .starts_with("Cannot read source")
        );
        let (_, sel) = f.jobs.retry().unwrap();
        assert_eq!(sel.files.len(), 1);
        assert_eq!(f.jobs.finished_page(0, 10, true).len(), 1);
    }

    #[test]
    fn a_job_with_the_checksum_file_off_writes_none() {
        let f = fixture(2, 10);
        run_with(
            &f,
            true,
            JobSettings {
                write_checksum_file: false,
                ..JobSettings::default()
            },
        );
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Complete);
        assert!(s.checksum_off && s.checksum_file.is_none());
        let xxh: Vec<_> = fs::read_dir(&f.dest)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "xxh64")
            })
            .collect();
        assert!(xxh.is_empty());
    }

    #[test]
    fn the_report_can_also_go_next_to_the_checksum_file() {
        let f = fixture(2, 10);
        run_with(
            &f,
            true,
            JobSettings {
                report_next_to_checksum: true,
                ..JobSettings::default()
            },
        );
        let s = f.jobs.summary().unwrap();
        let checksum = PathBuf::from(s.checksum_file.unwrap());
        let stem = checksum.file_stem().unwrap().to_string_lossy().into_owned();
        assert!(f.dest.join(format!("{stem}_report.txt")).is_file());
        assert!(f.dest.join(format!("{stem}_report.json")).is_file());
        assert!(!s.checksum_off && s.report_errors.is_empty());
    }

    /// Two ready jobs over temp dirs: one file, then two files.
    fn two_small_jobs() -> (tempfile::TempDir, Jobs, Ready, Ready) {
        let dir = tempfile::tempdir().unwrap();
        let ready = |name: &str, files: &[&str]| {
            let src = dir.path().join(name);
            std::fs::create_dir_all(&src).unwrap();
            for f in files {
                std::fs::write(src.join(f), b"x").unwrap();
            }
            let dest = dir.path().join(format!("{name}-dest"));
            std::fs::create_dir_all(&dest).unwrap();
            crate::queue::prepare(&crate::queue::CopyJob {
                sources: vec![src],
                include_folder: true,
                extensions: None,
                destination: dest,
                conflicts: crate::dto::ConflictPolicy::KeepBoth,
                verify: false,
                overwrite: vec![],
            })
            .ok()
            .unwrap()
        };
        let one = ready("one", &["a"]);
        let two = ready("two", &["a", "b"]);
        let jobs = Jobs::new(dir.path().join("reports"));
        (dir, jobs, one, two)
    }

    #[test]
    fn a_finished_job_stays_readable_after_the_next_one() {
        let (_dir, jobs, first_ready, second_ready) = two_small_jobs();
        jobs.start(
            first_ready,
            false,
            JobSettings::default(),
            Collect::default(),
        )
        .unwrap();
        jobs.wait();
        let first = jobs.current_handle().unwrap();
        jobs.start(
            second_ready,
            false,
            JobSettings::default(),
            Collect::default(),
        )
        .unwrap();
        jobs.wait();
        let second = jobs.current_handle().unwrap();
        assert_eq!(first.summary().unwrap().files, 1);
        assert_eq!(second.summary().unwrap().files, 2);
        assert_eq!(first.finished_page(0, 10, false).len(), 1);
        assert_eq!(
            jobs.summary().unwrap().files,
            2,
            "the current job is the last one"
        );
    }
    fn preset(o: &Path, d: &Path, mode: crate::store::DeletedMode) -> crate::store::MirrorPreset {
        crate::store::MirrorPreset {
            id: "m".into(),
            name: "Footage".into(),
            origin: show(o),
            destination: show(d),
            deleted: crate::store::DeletedFiles { mode, days: 30 },
            deep_check: false,
            clear_archive: None,
        }
    }

    /// Runs `preset` as a mirror job to its end.
    fn run_mirror(dir: &Path, preset: &crate::store::MirrorPreset) -> SummaryView {
        let job = crate::mirrors::prepare(preset, &JobControl::new(), &|_, _| {}).unwrap();
        let jobs = Jobs::new(dir.join("reports"));
        jobs.start(
            job.ready(),
            true,
            JobSettings::for_mirror(&job, Local::now()),
            Collect::default(),
        )
        .unwrap();
        jobs.wait();
        jobs.summary().unwrap()
    }

    /// An archive run directory `days_ago` old, holding one file.
    fn archive_run(d: &Path, days_ago: i64) -> std::path::PathBuf {
        let run = mirror::archive_dir(d, Local::now() - chrono::Duration::days(days_ago));
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(run.join("old.mov"), b"o").unwrap();
        run
    }

    /// #101: after switching to Delete, archived files still go once they reach the preset's days.
    #[test]
    fn a_mirror_in_delete_mode_still_removes_expired_archives() {
        let dir = tempfile::tempdir().unwrap();
        let (o, d) = (dir.path().join("o"), dir.path().join("d"));
        std::fs::create_dir_all(&o).unwrap();
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(o.join("a.mov"), b"a").unwrap();
        let (expired, young) = (archive_run(&d, 40), archive_run(&d, 2));
        run_mirror(
            dir.path(),
            &preset(&o, &d, crate::store::DeletedMode::Delete),
        );
        assert!(!expired.exists(), "older than the preset's 30 days");
        assert!(young.exists());
    }

    #[test]
    fn a_mirror_job_copies_then_removes_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let (o, d) = (dir.path().join("o"), dir.path().join("d"));
        std::fs::create_dir_all(&o).unwrap();
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(o.join("a.mov"), b"a").unwrap();
        std::fs::write(d.join("x.mov"), b"x").unwrap();
        let job = crate::mirrors::prepare(
            &preset(&o, &d, crate::store::DeletedMode::Archive),
            &JobControl::new(),
            &|_, _| {},
        )
        .unwrap();
        let jobs = Jobs::new(dir.path().join("reports"));
        let sink = Collect::default();
        jobs.start(
            job.ready(),
            true,
            JobSettings::for_mirror(&job, Local::now()),
            sink.clone(),
        )
        .unwrap();
        jobs.wait();
        let s = jobs.summary().unwrap();
        let m = s.mirror.unwrap();
        assert_eq!((m.new, m.updated, m.removed, m.archived), (1, 0, 1, true));
        assert!(s.checksum_file.is_none());
        assert!(!d.join("x.mov").exists());
        assert!(
            sink.0
                .lock()
                .unwrap()
                .iter()
                .any(|v| v.phase == JobPhase::Removing)
        );
        assert_eq!(sink.last().phase, JobPhase::Done);
    }

    #[test]
    fn a_mirror_that_failed_says_nothing_was_removed() {
        let dir = tempfile::tempdir().unwrap();
        let (o, d) = (dir.path().join("o"), dir.path().join("d"));
        std::fs::create_dir_all(&o).unwrap();
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(o.join("a.mov"), b"a").unwrap();
        std::fs::write(d.join("x.mov"), b"x").unwrap();
        let job = crate::mirrors::prepare(
            &preset(&o, &d, crate::store::DeletedMode::Delete),
            &JobControl::new(),
            &|_, _| {},
        )
        .unwrap();
        std::fs::remove_file(o.join("a.mov")).unwrap();
        let jobs = Jobs::new(dir.path().join("reports"));
        jobs.start(
            job.ready(),
            true,
            JobSettings::for_mirror(&job, Local::now()),
            Collect::default(),
        )
        .unwrap();
        jobs.wait();
        let m = jobs.summary().unwrap().mirror.unwrap();
        assert_eq!(
            m.nothing_removed.en().as_deref(),
            Some("Files deleted in the origin were left in the destination: 1 file failed.")
        );
        assert!(d.join("x.mov").exists());
    }

    #[test]
    fn a_missing_origin_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let p = preset(
            Path::new("/Volumes/SECOPY_NO_SUCH/Footage"),
            dir.path(),
            crate::store::DeletedMode::Archive,
        );
        assert_eq!(
            crate::mirrors::prepare(&p, &JobControl::new(), &|_, _| {})
                .err()
                .unwrap(),
            "SECOPY_NO_SUCH isn't connected."
        );
    }

    fn check_fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Day01");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.mov"), b"a").unwrap();
        fs::write(root.join("b.mov"), b"b").unwrap();
        let entries = vec![
            (PathBuf::from("a.mov"), secopy_core::hash::hash_bytes(b"a")),
            (PathBuf::from("b.mov"), secopy_core::hash::hash_bytes(b"b")),
        ];
        secopy_core::checksum_file::write(&root, &entries, Local::now()).unwrap();
        (dir, root)
    }

    /// Plan 8: a check job ends with a check summary and rows that say intact or changed.
    #[test]
    fn a_check_job_reports_intact_and_changed_files() {
        let (dir, root) = check_fixture();
        fs::write(root.join("b.mov"), b"B").unwrap();
        let jobs = Jobs::new(dir.path().join("reports"));
        let plan = Arc::new(secopy_core::check::plan(&root).unwrap());
        jobs.start_work(Work::Check(plan), Collect::default())
            .unwrap();
        jobs.wait();
        let s = jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Failures);
        let c = s.check.expect("a check summary");
        assert_eq!((c.intact, c.changed, c.missing), (1, 1, 0));
        let rows = jobs.finished_page(0, 10, false);
        let mut statuses: Vec<RowStatus> = rows.iter().map(|r| r.status).collect();
        statuses.sort_by_key(|s| format!("{s:?}"));
        assert_eq!(statuses, [RowStatus::Changed, RowStatus::Intact]);
        let report = fs::read_to_string(s.report_file.unwrap()).unwrap();
        assert!(report.contains("Result:       1 changed"), "{report}");
    }

    /// #69: a problem file says which checksum file listed it.
    #[test]
    fn a_check_row_names_the_checksum_file_that_listed_it() {
        let (dir, root) = check_fixture();
        fs::write(root.join("b.mov"), b"B").unwrap();
        let plan = Arc::new(secopy_core::check::plan(&root).unwrap());
        fs::remove_file(root.join("a.mov")).unwrap();
        let sum = plan.checksum_files[0]
            .file_name()
            .unwrap()
            .to_string_lossy();
        let listed = format!("Listed in {sum}.");
        let jobs = Jobs::new(dir.path().join("reports"));
        jobs.start_work(Work::Check(plan.clone()), Collect::default())
            .unwrap();
        jobs.wait();
        let rows = jobs.finished_page(0, 10, false);
        let reason = |status| {
            let row = rows.iter().find(|r| r.status == status).unwrap();
            row.reason
                .as_ref()
                .map(Message::english)
                .unwrap_or_default()
        };
        assert_eq!(reason(RowStatus::Missing), listed);
        assert!(
            reason(RowStatus::Changed).ends_with(&format!(" {listed}")),
            "{rows:?}"
        );
    }

    #[test]
    fn an_intact_check_is_complete() {
        let (dir, root) = check_fixture();
        let jobs = Jobs::new(dir.path().join("reports"));
        let plan = Arc::new(secopy_core::check::plan(&root).unwrap());
        jobs.start_work(Work::Check(plan), Collect::default())
            .unwrap();
        jobs.wait();
        assert_eq!(jobs.summary().unwrap().outcome, JobOutcome::Complete);
    }

    /// Panics on its first view, like a bug in the job's thread would.
    #[derive(Clone, Default)]
    struct PanicsFirst(Collect, Arc<AtomicBool>);
    impl ProgressSink for PanicsFirst {
        fn send(&self, view: ProgressView) {
            if !self.1.swap(true, Relaxed) {
                panic!("a bug");
            }
            self.0.send(view);
        }
    }

    /// #69 review: a check whose thread panicked is still a check: its summary counts what
    /// was read, and says nothing about a checksum file.
    #[test]
    fn a_check_that_panicked_is_still_a_check() {
        let (dir, root) = check_fixture();
        let jobs = Jobs::new(dir.path().join("reports"));
        let plan = Arc::new(secopy_core::check::plan(&root).unwrap());
        jobs.start_work(Work::Check(plan), PanicsFirst::default())
            .unwrap();
        jobs.wait();
        let s = jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Stopped);
        assert!(s.check.is_some(), "a check summary");
        assert!(!s.checksum_off);
    }

    /// #70: the ending after a panic panics too (the same bug); the window still gets its
    /// Done view.
    #[test]
    fn a_second_panic_while_ending_still_sends_done() {
        /// Panics on its first two views: the job's, then the ending's.
        #[derive(Clone, Default)]
        struct PanicsTwice(Collect, Arc<AtomicU32>);
        impl ProgressSink for PanicsTwice {
            fn send(&self, view: ProgressView) {
                if self.1.fetch_add(1, Relaxed) < 2 {
                    panic!("a bug");
                }
                self.0.send(view);
            }
        }
        let f = fixture(3, 10);
        let sink = PanicsTwice::default();
        f.jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                sink.clone(),
            )
            .unwrap();
        f.jobs.wait();
        let views = sink.0.0.lock().unwrap().clone();
        let last = views.last().expect("a view after the panics");
        assert_eq!(last.phase, JobPhase::Done);
        assert_eq!(
            last.fatal.en().as_deref(),
            Some("Secopy hit an internal error")
        );
        assert_eq!(f.jobs.summary().unwrap().outcome, JobOutcome::Stopped);
    }

    /// #69: a panic in the job's thread still ends the job, as stopped and saying why, so the
    /// window doesn't stay on the Copying screen.
    #[test]
    fn a_panic_in_the_job_ends_it_as_stopped() {
        let f = fixture(3, 10);
        let sink = PanicsFirst::default();
        f.jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                sink.clone(),
            )
            .unwrap();
        f.jobs.wait();
        assert!(!f.jobs.is_running());
        let last = sink.0.last();
        assert_eq!(last.phase, JobPhase::Done);
        assert_eq!(
            last.fatal.en().as_deref(),
            Some("Secopy hit an internal error")
        );
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Stopped);
        assert_eq!(
            s.stopped_because.en().as_deref(),
            Some("Secopy hit an internal error")
        );
        assert!(
            !s.report_errors.is_empty(),
            "no report for a run that never ended"
        );
        assert!(
            f.jobs.save_report(&f.dir.path().join("mine.txt")).is_err(),
            "a report would call it complete"
        );
    }

    /// #69: a panic while a job's lock was held doesn't take its summary and rows with it.
    #[test]
    fn a_poisoned_job_lock_still_gives_the_summary() {
        let f = fixture(2, 10);
        run(&f, true);
        let job = f.jobs.job().unwrap();
        crate::tests::poison(&job.outcomes);
        crate::tests::poison(&job.done);
        crate::tests::poison(&f.jobs.current);
        assert_eq!(f.jobs.summary().unwrap().outcome, JobOutcome::Complete);
        assert_eq!(f.jobs.finished_page(0, 10, false).len(), 2);
        assert!(!f.jobs.is_running());
    }

    /// Plan 8: a clean mirror job writes the mirror's checksum file.
    #[test]
    fn a_mirror_job_writes_its_checksum_file() {
        let dir = tempfile::tempdir().unwrap();
        let (o, d) = (dir.path().join("o"), dir.path().join("d"));
        fs::create_dir_all(&o).unwrap();
        fs::create_dir_all(&d).unwrap();
        fs::write(o.join("a.mov"), b"a").unwrap();
        fs::write(o.join("b.mov"), b"b").unwrap();
        let job = crate::mirrors::prepare(
            &preset(&o, &d, crate::store::DeletedMode::Archive),
            &JobControl::new(),
            &|_, _| {},
        )
        .unwrap();
        let jobs = Jobs::new(dir.path().join("reports"));
        jobs.start(
            job.ready(),
            true,
            JobSettings::for_mirror(&job, Local::now()),
            Collect::default(),
        )
        .unwrap();
        jobs.wait();
        assert_eq!(jobs.summary().unwrap().outcome, JobOutcome::Complete);
        let text = fs::read_to_string(d.join(secopy_core::check::MIRROR_CHECKSUMS)).unwrap();
        let (entries, bad) = secopy_core::check::parse(&text);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(entries.len(), 2);
    }

    /// #80: the menu bar icon starts from the job's real figures.
    #[test]
    fn the_last_progress_can_be_read() {
        let f = fixture(3, 10);
        assert!(f.jobs.progress_view().is_none(), "no job yet");
        f.jobs
            .start(
                f.session.ready().unwrap(),
                true,
                JobSettings::default(),
                Collect::default(),
            )
            .unwrap();
        f.jobs.wait();
        let view = f.jobs.progress_view().expect("the last job's figures");
        assert_eq!(view.total_files, 3);
        assert_eq!(view.files_done, 3);
    }

    /// #80: the menu bar panel names the job, and where it copies from and to.
    #[test]
    fn a_job_describes_itself_for_the_menu_bar() {
        let f = fixture(3, 10);
        assert!(f.jobs.describe().is_none(), "no job yet");
        let ready = f.session.ready().unwrap();
        let root = show(&ready.copy_root);
        f.jobs
            .start(ready, true, JobSettings::default(), Collect::default())
            .unwrap();
        f.jobs.wait();
        let (heading, from, to) = f.jobs.describe().unwrap();
        assert_eq!(heading, "Copying & verifying");
        assert!(!from.is_empty());
        assert_eq!(to, Some(root));
    }
}
