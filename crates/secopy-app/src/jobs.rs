//! Runs one copy job at a time on its own thread and keeps what the UI asks for: progress
//! twice a second, pages of finished files, the summary and the report (RFD §5.3, §5.4,
//! FR-35).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use secopy_core::checksum_file;
use secopy_core::control::JobControl;
use secopy_core::job::{
    Event, FileOutcome, FileStatus, JobOptions, JobReport, Progress, SkipReason, run_job,
};
use secopy_core::plan::Plan;
use secopy_core::report::{JobMeta, Report};
use secopy_core::scan::Selection;
use secopy_core::source::Source;

use crate::dto::{
    ActiveFileView, FinishedRow, JobOutcome, JobPhase, ProgressView, RowStatus, SmallFilesView,
    SummaryView, bytes, count, show,
};
use crate::session::Ready;

/// Progress reaches the UI twice a second (NFR-5).
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
/// Files at least this big get their own row in the active list (RFD §5.3).
const OWN_ROW: u64 = 8 << 20;
/// Failures listed in the summary; the finished list has all of them.
const FAILURES_SHOWN: usize = 1000;

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
    control: JobControl,
    started: DateTime<Local>,
    clock: Instant,
    /// In the order files finished.
    outcomes: Mutex<Vec<FileOutcome>>,
    failed: AtomicU32,
    done: Mutex<Option<Done>>,
}

struct Done {
    report: JobReport,
    finished: DateTime<Local>,
    report_file: Option<PathBuf>,
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
    pub fn start(&self, ready: Ready, verify: bool, sink: impl ProgressSink) -> Result<(), String> {
        if self.is_running() {
            return Err("A copy is already running.".into());
        }
        let job = Arc::new(Job {
            ready,
            verify,
            control: JobControl::new(),
            started: Local::now(),
            clock: Instant::now(),
            outcomes: Mutex::new(Vec::new()),
            failed: AtomicU32::new(0),
            done: Mutex::new(None),
        });
        *self.current.lock().expect("jobs lock poisoned") = Some(job.clone());
        let reports_dir = self.reports_dir.clone();
        let handle = std::thread::spawn(move || job.run(&sink, &reports_dir));
        *self.thread.lock().expect("jobs lock poisoned") = Some(handle);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.job()
            .is_some_and(|j| j.done.lock().expect("job lock poisoned").is_none())
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

    /// Stops the job: the file in progress is removed, finished files stay (FR-23).
    pub fn cancel(&self) {
        if let Some(job) = self.job() {
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
        let Some(job) = self.job() else {
            return Vec::new();
        };
        let outcomes = job.outcomes.lock().expect("job lock poisoned");
        outcomes
            .iter()
            .filter(|o| !failed_only || matches!(o.status, FileStatus::Failed(_)))
            .skip(offset as usize)
            .take(limit as usize)
            .map(row)
            .collect()
    }

    /// The summary once the job has ended; `None` while it runs.
    pub fn summary(&self) -> Option<SummaryView> {
        let job = self.job()?;
        let done = job.done.lock().expect("job lock poisoned");
        let done = done.as_ref()?;
        let report = job.report(done);
        let outcomes = job.outcomes.lock().expect("job lock poisoned");
        let c = &report.counts;
        Some(SummaryView {
            outcome: if done.report.fatal.is_some() {
                JobOutcome::Stopped
            } else if done.report.cancelled {
                JobOutcome::Cancelled
            } else if c.failed > 0 {
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
            bytes_written: bytes(c.bytes_written),
            seconds: done.report.elapsed.as_secs_f64(),
            failures: outcomes
                .iter()
                .filter(|o| matches!(o.status, FileStatus::Failed(_)))
                .take(FAILURES_SHOWN)
                .map(row)
                .collect(),
            copy_root: show(&job.ready.copy_root),
            checksum_file: done.report.checksum_file.as_deref().map(show),
            checksum_error: done.report.checksum_error.clone(),
            report_file: done.report_file.as_deref().map(show),
        })
    }

    /// "Save report…": the text report at `path` and the JSON next to it (FR-35).
    pub fn save_report(&self, path: &Path) -> Result<(), String> {
        let job = self.job().ok_or("There is no report yet.")?;
        let done = job.done.lock().expect("job lock poisoned");
        let done = done.as_ref().ok_or("The copy is still running.")?;
        let report = job.report(done);
        let write = |p: &Path, body: String| fs::write(p, body).map_err(|e| e.to_string());
        write(path, report.to_text())?;
        write(&path.with_extension("json"), report.to_json())
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

impl Job {
    fn run(&self, sink: &impl ProgressSink, reports_dir: &Path) {
        let opts = JobOptions {
            verify: self.verify,
            progress_interval: PROGRESS_INTERVAL,
            ..JobOptions::default()
        };
        let plan: &Plan = &self.ready.plan;
        let report = run_job(plan, &opts, &self.control, &|event| match event {
            Event::Progress(p) => sink.send(self.progress(&p, false, None)),
            Event::FileFinished(o) => {
                if matches!(o.status, FileStatus::Failed(_)) {
                    self.failed.fetch_add(1, Relaxed);
                }
                self.outcomes.lock().expect("job lock poisoned").push(o);
            }
        });
        let mut done = Done {
            finished: Local::now(),
            report_file: None,
            report,
        };
        done.report_file = self.save(&done, reports_dir);
        let fatal = done.report.fatal.as_ref().map(|f| sentence(&f.to_string()));
        let last = Progress {
            total_files: count(plan.files.len()).into(),
            ..Progress::default()
        };
        let mut view = self.progress(&last, true, fatal);
        view.copied_bytes = view.total_bytes;
        view.verified_bytes = if self.verify { view.total_bytes } else { 0.0 };
        view.files_done = count(done.report.outcomes.len());
        view.files_skipped = count(done.report.skipped().count());
        *self.done.lock().expect("job lock poisoned") = Some(done);
        sink.send(view);
    }

    fn progress(&self, p: &Progress, finished: bool, fatal: Option<String>) -> ProgressView {
        let total_bytes = self.ready.plan.bytes_to_write();
        let mut small = SmallFilesView {
            count: 0,
            size: 0.0,
            bytes_done: 0.0,
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
                    size: bytes(f.size),
                    bytes_done: bytes(f.bytes_done),
                });
            } else {
                small.count += 1;
                small.size += bytes(f.size);
                small.bytes_done += bytes(f.bytes_done);
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
            elapsed_ms: self.clock.elapsed().as_secs_f64() * 1000.0,
            paused: p.paused,
            verify: self.verify,
            total_files: count(self.ready.plan.files.len()),
            total_bytes: bytes(total_bytes),
            copied_bytes: bytes(p.copied_bytes),
            verified_bytes: bytes(p.verified_bytes),
            files_done: count(p.files_done),
            files_skipped: count(p.files_skipped),
            files_failed: self.failed.load(Relaxed),
            active,
            small_files: (small.count > 0).then_some(small),
            fatal,
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
    fn save(&self, done: &Done, reports_dir: &Path) -> Option<PathBuf> {
        fs::create_dir_all(reports_dir).ok()?;
        let stem = match &done.report.checksum_file {
            Some(path) => path.file_stem()?.to_string_lossy().into_owned(),
            None => checksum_file::file_name(self.started).replace(".xxh64", ""),
        };
        self.report(done)
            .write(reports_dir, &stem)
            .ok()
            .map(|(text, _)| text)
    }
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
    };
    FinishedRow {
        id: count(o.id),
        path: show(&o.rel),
        final_path: show(&o.final_rel),
        size: bytes(o.size),
        seconds: o.elapsed.as_secs_f64(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, scan_source};

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
        let ticket = session.begin_scan();
        let source = Session::source_for(std::slice::from_ref(&card), false).unwrap();
        let scan = scan_source(&source).unwrap();
        session.finish_scan(ticket, source, scan);
        session.set_destination(Some(dest.clone()));
        let jobs = Jobs::new(dir.path().join("reports"));
        Fixture {
            dir,
            dest,
            session,
            jobs,
        }
    }

    fn run(f: &Fixture, verify: bool) -> Collect {
        let sink = Collect::default();
        f.jobs
            .start(f.session.ready().unwrap(), verify, sink.clone())
            .unwrap();
        f.jobs.wait();
        sink
    }

    #[test]
    fn a_job_copies_verifies_and_ends_with_a_done_view() {
        let f = fixture(5, 1000);
        let sink = run(&f, true);
        let last = sink.last();
        assert_eq!(last.phase, JobPhase::Done);
        assert_eq!(last.files_done, 5);
        assert_eq!(last.verified_bytes, 5000.0);
        assert!(!f.jobs.is_running());
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Complete);
        assert_eq!((s.files, s.verified, s.failed), (5, 5, 0));
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
            .start(f.session.ready().unwrap(), true, sink.clone())
            .unwrap();
        f.jobs.cancel();
        f.jobs.wait();
        let s = f.jobs.summary().unwrap();
        assert_eq!(s.outcome, JobOutcome::Cancelled);
        assert_eq!(sink.last().phase, JobPhase::Done);
    }

    #[test]
    fn a_second_job_cannot_start_while_one_runs() {
        let f = fixture(200, 50_000);
        f.jobs
            .start(f.session.ready().unwrap(), true, Collect::default())
            .unwrap();
        f.jobs.pause();
        let err = f
            .jobs
            .start(f.session.ready().unwrap(), true, Collect::default())
            .unwrap_err();
        assert!(err.contains("already running"));
        f.jobs.cancel();
        f.jobs.wait();
    }

    #[test]
    fn a_paused_job_reports_paused_and_finishes_after_resume() {
        let f = fixture(50, 20_000);
        let sink = Collect::default();
        f.jobs
            .start(f.session.ready().unwrap(), true, sink.clone())
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
}
