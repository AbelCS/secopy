//! Runs one copy job at a time on its own thread and keeps what the UI asks for: progress
//! twice a second, pages of finished files, the summary and the report (RFD §5.3, §5.4,
//! FR-35).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use secopy_core::checksum_file;
use secopy_core::control::JobControl;
use secopy_core::job::{
    Event, FileOutcome, FileStatus, JobOptions, JobReport, Progress, SkipReason, Undone, run_job,
    undo,
};
use secopy_core::mirror::{self, Change, Deleted, MirrorPlan, Removal};
use secopy_core::plan::Plan;
use secopy_core::report::{JobMeta, Report};
use secopy_core::scan::Selection;
use secopy_core::source::Source;

use crate::dto::{
    ActiveFileView, FinishedRow, JobOutcome, JobPhase, MirrorSummaryView, ProgressView, RowStatus,
    SmallFilesView, SummaryView, UndoneView, count, show,
};
use crate::mirrors::MirrorJob;
use crate::session::Ready;
use crate::store::Settings;

/// Progress reaches the UI twice a second (NFR-5).
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
/// Files at least this big get their own row in the active list (RFD §5.3).
const OWN_ROW: u64 = 8 << 20;
/// Failures listed in the summary; the finished list has all of them.
const FAILURES_SHOWN: usize = 1000;

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

/// The one job the app runs at a time.
pub struct Jobs {
    reports_dir: PathBuf,
    current: Mutex<Option<Arc<Job>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

struct Job {
    ready: Ready,
    verify: bool,
    settings: JobSettings,
    control: JobControl,
    started: DateTime<Local>,
    clock: Instant,
    /// In the order files finished.
    outcomes: Mutex<Vec<FileOutcome>>,
    /// The engine's latest progress; the final view is built from it.
    last: Mutex<Progress>,
    failed: AtomicU32,
    /// Cancel asked to remove the files already copied (#54).
    remove_copied: AtomicBool,
    done: Mutex<Option<Done>>,
}

struct Done {
    report: JobReport,
    finished: DateTime<Local>,
    /// The report saved in the reports folder, or why it couldn't be.
    report_file: Result<PathBuf, String>,
    /// Why the report couldn't also be written next to the checksum file.
    next_to_error: Option<String>,
    /// A mirror's removals, or why nothing was removed (plan 7).
    removals: Option<Result<mirror::Finished, String>>,
    /// What a cancel with "Also remove the files already copied" removed (#54).
    undone: Option<Undone>,
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
    ) -> Result<(), String> {
        // Held until the job is in place, so two starts can't both get past the check.
        let mut current = self.current.lock().expect("jobs lock poisoned");
        if current.as_ref().is_some_and(|job| job.running()) {
            return Err("A copy is already running.".into());
        }
        let job = Arc::new(Job {
            ready,
            verify,
            settings,
            control: JobControl::new(),
            started: Local::now(),
            clock: Instant::now(),
            outcomes: Mutex::new(Vec::new()),
            last: Mutex::new(Progress::default()),
            failed: AtomicU32::new(0),
            remove_copied: AtomicBool::new(false),
            done: Mutex::new(None),
        });
        *current = Some(job.clone());
        let reports_dir = self.reports_dir.clone();
        let handle = std::thread::spawn(move || job.run(&sink, &reports_dir));
        *self.thread.lock().expect("jobs lock poisoned") = Some(handle);
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
        let handle = self.thread.lock().expect("jobs lock poisoned").take();
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

    /// "Save report…": the text report at `path` and the JSON next to it (FR-35).
    pub fn save_report(&self, path: &Path) -> Result<(), String> {
        self.job()
            .ok_or("There is no report yet.")?
            .save_report(path)
    }

    /// The current (or last) job, kept for the queue summary.
    pub fn current_handle(&self) -> Option<JobHandle> {
        self.job().map(JobHandle)
    }

    /// The failed files of the last job, for "Retry failed" (RFD §5.4).
    pub fn retry(&self) -> Option<(Source, Selection)> {
        let job = self.job()?;
        let ids: Vec<usize> = job
            .outcomes
            .lock()
            .expect("job lock poisoned")
            .iter()
            .filter(|o| matches!(o.status, FileStatus::Failed(_)))
            .map(|o| o.id)
            .collect();
        if ids.is_empty() {
            return None;
        }
        let plan = &job.ready.plan;
        let all = Selection {
            files: plan.files.iter().map(|f| f.entry.clone()).collect(),
            dirs: plan.dirs.clone(),
            total_bytes: plan.total_bytes(),
        };
        Some((job.ready.source.clone(), all.subset(&ids)))
    }

    fn job(&self) -> Option<Arc<Job>> {
        self.current.lock().expect("jobs lock poisoned").clone()
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

    pub fn save_report(&self, path: &Path) -> Result<(), String> {
        self.0.save_report(path)
    }
}

impl Job {
    /// Rows of the finished list, in the order files finished.
    fn finished_page(&self, offset: u32, limit: u32, failed_only: bool) -> Vec<FinishedRow> {
        let outcomes = self.outcomes.lock().expect("job lock poisoned");
        outcomes
            .iter()
            .filter(|o| !failed_only || matches!(o.status, FileStatus::Failed(_)))
            .skip(offset as usize)
            .take(limit as usize)
            .map(row)
            .collect()
    }

    /// The summary once the job has ended; `None` while it runs.
    fn summary(&self) -> Option<SummaryView> {
        let job = self;
        let done = job.done.lock().expect("job lock poisoned");
        let done = done.as_ref()?;
        let report = job.report(done);
        let outcomes = job.outcomes.lock().expect("job lock poisoned");
        let c = &report.counts;
        let mirror = self
            .settings
            .mirror
            .as_ref()
            .map(|m| mirror_summary(m, done, &outcomes));
        let removal_failed = mirror
            .as_ref()
            .is_some_and(|m| !m.removal_failures.is_empty());
        Some(SummaryView {
            outcome: if done.report.fatal.is_some() {
                JobOutcome::Stopped
            } else if done.report.cancelled {
                JobOutcome::Cancelled
            } else if c.failed > 0 || removal_failed {
                JobOutcome::Failures
            } else {
                JobOutcome::Complete
            },
            stopped_because: done.report.fatal.as_ref().map(|f| sentence(&f.to_string())),
            verify: job.verify,
            files: count(c.files),
            copied: count(c.copied),
            verified: count(c.verified),
            skipped_identical: count(c.skipped_identical),
            skipped_different: count(c.skipped_different),
            failed: count(c.failed),
            not_started: count(c.not_started),
            bytes_written: c.bytes_written,
            millis: done.report.elapsed.as_millis() as u64,
            failures: outcomes
                .iter()
                .filter(|o| matches!(o.status, FileStatus::Failed(_)))
                .take(FAILURES_SHOWN)
                .map(row)
                .collect(),
            finished: count(outcomes.len()),
            copy_root: show(&job.ready.copy_root),
            checksum_file: done.report.checksum_file.as_deref().map(show),
            checksum_error: done.report.checksum_error.clone(),
            checksum_off: !job.settings.write_checksum_file,
            report_file: done.report_file.as_deref().ok().map(show),
            report_error: {
                let errors: Vec<String> = done
                    .report_file
                    .as_ref()
                    .err()
                    .cloned()
                    .into_iter()
                    .chain(done.next_to_error.clone())
                    .collect();
                (!errors.is_empty()).then(|| errors.join("; "))
            },
            mirror,
            undone: done.undone.as_ref().map(|u| UndoneView {
                removed: count(u.removed),
                restored: count(u.restored),
                not_restored: count(u.not_restored),
                failed: count(u.failed.len()),
            }),
        })
    }

    /// "Save report…": the text report at `path` and the JSON next to it (FR-35).
    fn save_report(&self, path: &Path) -> Result<(), String> {
        let job = self;
        let done = job.done.lock().expect("job lock poisoned");
        let done = done.as_ref().ok_or("The copy is still running.")?;
        let report = job.report(done);
        let write = |p: &Path, body: String| fs::write(p, body).map_err(|e| e.to_string());
        write(path, report.to_text())?;
        write(&path.with_extension("json"), report.to_json())
    }

    fn running(&self) -> bool {
        self.done.lock().expect("job lock poisoned").is_none()
    }

    fn run(&self, sink: &impl ProgressSink, reports_dir: &Path) {
        let mirroring = self.settings.mirror.as_ref();
        // Archive runs older than the preset keeps them go first (FR-49).
        if let Some(m) = mirroring
            && let Deleted::Archive { days } = m.plan.options.deleted
        {
            mirror::clean_archives(&m.plan.copy.dest, days, Local::now());
        }
        let opts = JobOptions {
            verify: self.verify,
            write_checksum_file: self.settings.write_checksum_file,
            progress_interval: PROGRESS_INTERVAL,
            archive_replaced: mirroring.and_then(|m| m.archive.clone()),
            ..JobOptions::default()
        };
        let plan: &Plan = &self.ready.plan;
        let mut report = run_job(plan, &opts, &self.control, &|event| match event {
            Event::Progress(p) => {
                sink.send(self.progress(&p, false, None));
                *self.last.lock().expect("job lock poisoned") = p;
            }
            Event::FileFinished(o) => {
                if matches!(o.status, FileStatus::Failed(_)) {
                    self.failed.fetch_add(1, Relaxed);
                }
                self.outcomes.lock().expect("job lock poisoned").push(o);
            }
        });
        // Cancel with "Also remove the files already copied": the destination as it was.
        let undone = (report.cancelled && self.remove_copied.load(Relaxed)).then(|| {
            let undone = undo(plan, &report, mirroring.and_then(|m| m.archive.as_deref()));
            report.checksum_file = None;
            undone
        });
        // A mirror removes what's gone from its origin, only after a clean copy phase.
        let removals = mirroring.map(|m| {
            if !report.cancelled {
                let last = self.last.lock().expect("job lock poisoned").clone();
                let mut view = self.progress(&last, false, None);
                view.phase = JobPhase::Removing;
                view.removing = count(m.plan.removals.len());
                view.archiving = m.archive.is_some();
                sink.send(view);
            }
            mirror::finish(&m.plan, &report, m.archive.as_deref())
        });
        let mut done = Done {
            finished: Local::now(),
            report_file: Err(String::new()),
            next_to_error: None,
            report,
            removals,
            undone,
        };
        done.report_file = self.save(&done, reports_dir);
        if self.settings.report_next_to_checksum
            && let Some(checksum) = &done.report.checksum_file
        {
            done.next_to_error = self
                .report(&done)
                .write_next_to(checksum)
                .err()
                .map(|e| format!("next to the checksum file: {e}"));
        }
        let fatal = done.report.fatal.as_ref().map(|f| sentence(&f.to_string()));
        // The engine's last progress, so a stopped job's bars stay where it stopped.
        let last = self.last.lock().expect("job lock poisoned").clone();
        let mut view = self.progress(&last, true, fatal);
        view.files_done = count(done.report.outcomes.len());
        view.files_skipped = count(done.report.skipped().count());
        *self.done.lock().expect("job lock poisoned") = Some(done);
        sink.send(view);
    }

    fn progress(&self, p: &Progress, finished: bool, fatal: Option<String>) -> ProgressView {
        let total_bytes = self.ready.plan.bytes_to_write();
        let mut small = SmallFilesView {
            count: 0,
            size: 0,
            bytes_done: 0,
        };
        let mut active = Vec::new();
        for f in &p.active {
            if f.size >= OWN_ROW {
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
            } else {
                small.count += 1;
                small.size += f.size;
                small.bytes_done += f.bytes_done;
            }
        }
        let copying = p
            .active
            .iter()
            .any(|f| f.phase == secopy_core::job::Phase::Copying);
        ProgressView {
            phase: if finished {
                JobPhase::Done
            } else if self.verify && !copying && p.copied_bytes >= total_bytes {
                JobPhase::Verifying
            } else {
                JobPhase::Copying
            },
            elapsed_ms: self.clock.elapsed().as_millis() as u64,
            paused: p.paused,
            verify: self.verify,
            total_files: count(self.ready.plan.files.len()),
            total_bytes,
            copied_bytes: p.copied_bytes,
            verified_bytes: p.verified_bytes,
            files_done: count(p.files_done),
            files_skipped: count(p.files_skipped),
            files_failed: self.failed.load(Relaxed),
            active,
            small_files: (small.count > 0).then_some(small),
            fatal,
            removing: 0,
            archiving: false,
        }
    }

    fn report(&self, done: &Done) -> Report {
        let meta = JobMeta {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            source: self.ready.label.clone(),
            verify: self.verify,
            started: self.started,
            finished: done.finished,
        };
        let mut job_report = done.report.clone();
        job_report.outcomes = self.outcomes.lock().expect("job lock poisoned").clone();
        Report::new(&self.ready.plan, &job_report, &meta)
    }

    /// Saves the report in the app's data folder, named like the checksum file (FR-35).
    fn save(&self, done: &Done, reports_dir: &Path) -> Result<PathBuf, String> {
        let failed = |e: std::io::Error| format!("{}: {e}", show(reports_dir));
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
        if let Some(removals) = &done.removals {
            let archived = self
                .settings
                .mirror
                .as_ref()
                .is_some_and(|m| m.archive.is_some());
            append_removals(&text, removals, archived).map_err(failed)?;
        }
        if let Some(undone) = &done.undone {
            append_undone(&text, undone).map_err(failed)?;
        }
        Ok(text)
    }
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
                        reason: Some(format!("Not removed: {e}")),
                    })
                })
                .collect(),
            None,
        ),
        Some(Err(why)) => (0, Vec::new(), Some(why.clone())),
        None => (0, Vec::new(), None),
    };
    MirrorSummaryView {
        new: count(done_as(true)),
        updated: count(done_as(false)),
        removed: count(removed),
        archived: m.archive.is_some(),
        removal_failures,
        nothing_removed,
    }
}

/// Adds a mirror's removals to its saved text report (FR-52).
fn append_removals(
    text: &Path,
    removals: &Result<mirror::Finished, String>,
    archived: bool,
) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = fs::OpenOptions::new().append(true).open(text)?;
    let finished = match removals {
        Ok(finished) => finished,
        Err(why) => return writeln!(out, "\n{why}"),
    };
    let (ok, failed): (Vec<&Removal>, Vec<&Removal>) =
        finished.removals.iter().partition(|r| r.result.is_ok());
    let how = if archived { "archived" } else { "deleted" };
    writeln!(out, "\nRemoved from the destination ({how}): {}", ok.len())?;
    for r in ok {
        writeln!(out, "  {}", r.rel.display())?;
    }
    if !failed.is_empty() {
        writeln!(out, "Not removed: {}", failed.len())?;
        for r in failed {
            let why = r.result.as_ref().err().map_or("", String::as_str);
            writeln!(out, "  {}: {why}", r.rel.display())?;
        }
    }
    if !finished.renamed.is_empty() {
        writeln!(
            out,
            "Renamed to match the origin: {}",
            finished.renamed.len()
        )?;
        for (from, to) in &finished.renamed {
            writeln!(out, "  {} → {}", from.display(), to.display())?;
        }
    }
    Ok(())
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

fn row(o: &FileOutcome) -> FinishedRow {
    let (status, reason) = match &o.status {
        FileStatus::Copied => (RowStatus::Copied, None),
        FileStatus::Verified => (RowStatus::Verified, None),
        FileStatus::Skipped(SkipReason::Identical) => (
            RowStatus::Skipped,
            Some("Already at the destination (not checked)".to_string()),
        ),
        FileStatus::Skipped(SkipReason::Differs) => (
            RowStatus::Skipped,
            Some("A different file with this name was kept".to_string()),
        ),
        FileStatus::Failed(e) => (RowStatus::Failed, Some(sentence(&e.to_string()))),
        FileStatus::Cancelled => (RowStatus::Cancelled, None),
    };
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

fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
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
    use crate::session::{Change, Session, scan_source};

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

    /// #57: the report counts only what was removed, lists what wasn't, and the renames.
    #[test]
    fn the_report_counts_removals_that_worked_and_lists_renames() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("r.txt");
        fs::write(&text, "Secopy\n").unwrap();
        let finished = mirror::Finished {
            removals: vec![
                Removal {
                    rel: "a.mov".into(),
                    result: Ok(()),
                },
                Removal {
                    rel: "b.mov".into(),
                    result: Err("Permission denied".into()),
                },
            ],
            renamed: vec![("IMG.jpg".into(), "img.jpg".into())],
        };
        append_removals(&text, &Ok(finished), true).unwrap();
        let out = fs::read_to_string(&text).unwrap();
        assert!(
            out.contains("Removed from the destination (archived): 1\n  a.mov\n"),
            "{out}"
        );
        assert!(
            out.contains("Not removed: 1\n  b.mov: Permission denied\n"),
            "{out}"
        );
        assert!(
            out.contains("Renamed to match the origin: 1\n  IMG.jpg → img.jpg\n"),
            "{out}"
        );
    }

    /// #54: Cancel with "Also remove the files already copied".
    #[test]
    fn cancelling_can_remove_what_was_copied() {
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
        f.jobs.cancel(true);
        f.jobs.wait();
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Cancelled);
        let undone = s.undone.expect("what was removed");
        assert!(undone.removed >= 1, "{undone:?}");
        assert_eq!((undone.not_restored, undone.failed), (0, 0));
        assert_eq!(s.checksum_file, None);
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
        assert!(s.report_error.is_some());
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

    #[cfg(unix)]
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
        assert!(!s.checksum_off && s.report_error.is_none());
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
        }
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
            m.nothing_removed.as_deref(),
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
}
