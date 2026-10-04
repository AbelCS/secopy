//! The Secopy engine in Terminal (`secopy-cli --help`), shipped with each release; tests use it too.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Local;
use clap::{Parser, ValueEnum};
use secopy_core::checksum_file;
use secopy_core::filter::ExtensionFilter;
use secopy_core::ignore::{MAX_LEN, MAX_PATTERNS, PatternError, Patterns};
use secopy_core::job::{self, Event, FileStatus, JobControl, JobOptions, JobReport, Progress};
use secopy_core::mhl::MhlJob;
use secopy_core::mhl::prepare::{MhlBlocker, MhlInputs, prepare};
use secopy_core::mirror::{self, Change, Deleted, MirrorOptions};
use secopy_core::plan::{DiffersPolicy, Plan};
use secopy_core::preflight::{ConflictKind, Preflight, preflight};
use secopy_core::report::{JobMeta, Report};
use secopy_core::scan::{self, ScanOptions};
use secopy_core::source::{DirMode, Source};

#[derive(Parser, Debug)]
#[command(
    name = "secopy-cli",
    version,
    about = "Fast file copy with XXH128 verification",
    after_help = "Exit codes: 0 every file copied (and verified), or every listed file intact; \
1 a file failed, was skipped as different, changed or is missing, or there was nothing to \
verify; 2 wrong arguments, or a job \
that couldn't start (nothing to copy, a destination problem).\n\n\
Unlike the app, a directory is copied with its name unless --contents is given, and copies \
aren't read back unless --verify is given."
)]
struct Args {
    /// One directory, or one or more files.
    #[arg(required_unless_present = "check")]
    sources: Vec<PathBuf>,
    /// Destination directory (must exist).
    #[arg(long, short = 't', required_unless_present = "check")]
    to: Option<PathBuf>,
    /// Verify a directory against its checksum files: every listed file is read again and
    /// compared. Exit 0 when all are intact, 1 otherwise.
    /// With --report, its report is written there too.
    #[arg(
        long,
        value_name = "DIR",
        conflicts_with_all = [
            "sources", "to", "mirror", "contents", "verify", "ext", "no_checksum", "mhl",
            "on_conflict",
        ]
    )]
    check: Option<PathBuf>,
    /// Copy only what is inside the source directory, not the directory itself.
    #[arg(long)]
    contents: bool,
    /// Read every copy back from the destination and compare it with the source's checksum.
    #[arg(long)]
    verify: bool,
    /// Only copy these extensions, e.g. "mov,wav". "(none)" means files without extension.
    #[arg(long)]
    ext: Option<String>,
    /// Do not write the .xxh128 checksum file.
    #[arg(long)]
    no_checksum: bool,
    /// Also write an ASC MHL history (the media industry's proof of copy) in the directory the
    /// files go to, or continue the one there or in the source.
    #[arg(long, conflicts_with = "mirror")]
    mhl: bool,
    /// Also copy system files (.DS_Store, Thumbs.db, …): don't use the default ignore list.
    /// Hidden files are always copied; Secopy's own working files never are.
    #[arg(long)]
    include_system_files: bool,
    /// Never copy or mirror files and directories with this name: * is any characters, ? one
    /// (case doesn't matter). Repeatable, up to 128; added to the default list.
    #[arg(long, value_name = "PATTERN")]
    ignore: Vec<String>,
    /// What to do with files that already exist at the destination but differ. Files with
    /// the same size and date are always skipped (not read).
    #[arg(long, value_enum, default_value_t = OnConflict::KeepBoth)]
    on_conflict: OnConflict,
    /// Also write the job report (text and JSON) into this directory.
    #[arg(long, value_name = "DIR")]
    report: Option<PathBuf>,
    /// Mirror the one source directory (the origin) to the destination: new and changed files
    /// copied and verified, files deleted in the origin archived (or deleted with --delete).
    #[arg(long)]
    mirror: bool,
    /// With --mirror: delete the files deleted in the origin, instead of archiving them.
    #[arg(long, requires = "mirror")]
    delete: bool,
    /// With --mirror: days to keep archived files. Each run first removes archived files older
    /// than this, with --delete too.
    #[arg(
        long,
        requires = "mirror",
        default_value_t = 30,
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..=36_500)
    )]
    archive_days: u32,
    /// With --mirror: also compare the checksums of files whose size and date match (slow:
    /// reads both sides).
    #[arg(long, requires = "mirror")]
    deep: bool,
    /// With --mirror: only show what would change.
    #[arg(long, requires = "mirror")]
    dry_run: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OnConflict {
    KeepBoth,
    Overwrite,
    Skip,
}

impl From<OnConflict> for DiffersPolicy {
    fn from(c: OnConflict) -> Self {
        match c {
            OnConflict::KeepBoth => DiffersPolicy::KeepBoth,
            OnConflict::Overwrite => DiffersPolicy::Overwrite,
            OnConflict::Skip => DiffersPolicy::Skip,
        }
    }
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}

impl Args {
    /// `--to`; clap requires it unless `--check` is given.
    fn to(&self) -> &Path {
        self.to.as_deref().expect("--to is required unless --check")
    }
}

fn run(args: Args) -> Result<ExitCode, String> {
    if let Some(dir) = &args.check {
        return check_run(dir, &patterns(&args)?, args.report.as_deref());
    }
    if !args.to().is_dir() {
        return Err(format!(
            "destination {} is not a directory",
            args.to().display()
        ));
    }
    if args.mirror {
        return mirror_run(&args);
    }
    let source = source_from(&args)?;
    let ignore = patterns(&args)?;
    let scan = scan::scan(
        &source,
        &ScanOptions {
            ignore: ignore.clone(),
        },
    )
    .map_err(|e| e.to_string())?;
    for p in &scan.problems {
        eprintln!("warning: {}: {}", p.path.display(), p.message);
    }
    for link in &scan.skipped_symlinks {
        eprintln!("skipped symlink: {}", link.display());
    }
    for special in &scan.skipped_special {
        eprintln!("skipped special file: {}", special.display());
    }
    let filter = args
        .ext
        .as_deref()
        .map(ExtensionFilter::parse_list)
        .unwrap_or_default();
    let selection = scan.select(&filter);
    eprintln!(
        "{} files, {} ({} ignored)",
        selection.files.len(),
        fmt_bytes(selection.total_bytes),
        scan.ignored
    );
    // Nothing matches (an --ext typo) or nothing at all: not a success a script could take for
    // one (#116). Empty folders alone are still copied (FR-6).
    if selection.files.is_empty() && (filter.is_active() || selection.dirs.is_empty()) {
        return Err(format!(
            "nothing to copy: none of the {} files match",
            scan.files.len()
        ));
    }
    let pf = preflight(&source, &selection, args.to()).map_err(|e| e.to_string())?;
    let mut plan = Plan::resolve(&selection, &pf, args.on_conflict.into());
    print_preflight(&pf, &plan);
    if let Some(blocker) = plan.blockers().first() {
        return Err(blocker.to_string());
    }
    let mhl = if args.mhl {
        Some(prepare_mhl(&mut plan, &source, &scan, &ignore)?)
    } else {
        None
    };

    let opts = JobOptions {
        verify: args.verify,
        write_checksum_file: !args.no_checksum,
        mhl,
        ..JobOptions::default()
    };
    let started_at = Local::now();
    let report = run_with_progress(&plan, &opts)?;
    print_summary(&report);
    if let Some(dir) = &args.report {
        let meta = job_meta(&args, args.verify, started_at);
        write_report(
            &Report::new(&plan, &report, &meta),
            dir,
            &report,
            started_at,
        )?;
    }
    Ok(if report.is_success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// The copy's ASC MHL (#154): what it writes, or every reason it can't.
fn prepare_mhl(
    plan: &mut Plan,
    source: &Source,
    scan: &scan::Scan,
    ignore: &Patterns,
) -> Result<MhlJob, String> {
    let inputs = MhlInputs {
        copy_root: match &scan.root_dir {
            Some(root) => plan.dest.join(root),
            None => plan.dest.clone(),
        },
        source_dir: match source {
            Source::Directory { path, .. } => Some(path.clone()),
            Source::Files(_) => None,
        },
        ignore: ignore.clone(),
    };
    match prepare(plan, &inputs) {
        Err(blockers) => {
            for b in &blockers {
                eprintln!("ASC MHL: {}", mhl_blocker(b));
            }
            Err("ASC MHL can't be written for this copy (see above), or leave out --mhl".into())
        }
        Ok(mhl) => {
            if mhl.generation() == 1 {
                eprintln!("ASC MHL: new history");
            } else {
                eprintln!(
                    "ASC MHL: continues the history (generation {})",
                    mhl.generation()
                );
            }
            match mhl.to_read.len() {
                0 => {}
                1 => eprintln!(
                    "ASC MHL: also records 1 file already there ({})",
                    fmt_bytes(mhl.to_read_bytes)
                ),
                n => eprintln!(
                    "ASC MHL: also records {n} files already there ({})",
                    fmt_bytes(mhl.to_read_bytes)
                ),
            }
            Ok(MhlJob {
                plan: mhl,
                tool_version: env!("CARGO_PKG_VERSION").into(),
            })
        }
    }
}

fn mhl_blocker(b: &MhlBlocker) -> String {
    match b {
        MhlBlocker::TwoHistories { scope } => format!(
            "the source and {} have different histories",
            scope.display()
        ),
        MhlBlocker::OverwritesRecorded { path } => format!(
            "overwriting {} would break the history that lists it",
            path.display()
        ),
        MhlBlocker::Damaged { scope, damage } => {
            format!("the history in {} is damaged ({damage:?})", scope.display())
        }
        MhlBlocker::LeavesOut { scope } => format!(
            "this copy leaves out files the source's history for {} lists",
            scope.display()
        ),
        MhlBlocker::Unlistable { path } => format!(
            "{} has characters a history can't hold in its name",
            path.display()
        ),
        MhlBlocker::Conflicts { path } => format!(
            "{} is in the source's history but would be copied under another name or skipped",
            path.display()
        ),
        MhlBlocker::Unreadable { path } => format!("{} can't be read", path.display()),
    }
}

/// Runs `plan`, showing progress and failures; Ctrl-C cancels.
fn run_with_progress(plan: &Plan, opts: &JobOptions) -> Result<JobReport, String> {
    with_progress(progress_line, |control, on_event| {
        job::run_job(plan, opts, control, on_event)
    })
}

/// Runs `job` with a control Ctrl-C cancels, a progress line every half second and failures
/// as they happen.
fn with_progress<T>(
    line: fn(&Progress, Duration) -> String,
    job: impl FnOnce(&JobControl, &(dyn Fn(Event) + Sync)) -> T,
) -> Result<T, String> {
    let control = Arc::new(JobControl::new());
    let handler_control = control.clone();
    ctrlc::set_handler(move || handler_control.cancel()).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let last_print = Mutex::new(Instant::now());
    let result = job(&control, &|event| match event {
        Event::Progress(p) => {
            let mut last = last_print.lock().unwrap();
            if last.elapsed() >= Duration::from_millis(500) {
                *last = Instant::now();
                eprint!("\r{}", line(&p, started.elapsed()));
            }
        }
        Event::FileFinished(o) => {
            if let FileStatus::Failed(e) = &o.status {
                eprintln!("\rFAILED {}: {e}", o.rel.display());
            }
        }
    });
    eprintln!();
    Ok(result)
}

/// `--mirror`: the destination becomes a copy of the one source directory (RFD §5.8).
fn mirror_run(args: &Args) -> Result<ExitCode, String> {
    let [origin] = args.sources.as_slice() else {
        return Err("--mirror takes one source directory".into());
    };
    let deleted = if args.delete {
        Deleted::Delete
    } else {
        Deleted::Archive
    };
    let options = MirrorOptions {
        deleted,
        deep_check: args.deep,
        ignore: patterns(args)?,
    };
    let plan = mirror::plan(origin, args.to(), &options)?;
    let new = plan
        .changes
        .iter()
        .filter(|(_, c)| *c == Change::New)
        .count();
    println!("+ new: {new}");
    println!("~ changed: {}", plan.changes.len() - new);
    let how = if args.delete { "deleted" } else { "archived" };
    println!("- removed: {} ({how})", plan.removals.len());
    // Files that will fail aren't unchanged (#137).
    let failing = plan
        .copy
        .files
        .iter()
        .filter(|f| matches!(f.action, secopy_core::plan::Action::Fail(_)))
        .count();
    if failing > 0 {
        println!("! will fail: {failing}");
    }
    println!(
        "= unchanged: {}",
        plan.copy.files.len() - plan.changes.len() - failing
    );
    if let Some(guard) = &plan.guard {
        println!("guard: {guard}");
    }
    if args.dry_run {
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(guard) = &plan.guard {
        return Err(format!("{guard} Not mirroring: that looks wrong."));
    }
    let now = Local::now();
    // Archived files older than --archive-days go first, with --delete too (#101).
    let cleaned = mirror::clean_archives(args.to(), args.archive_days, now);
    // Said, and not a success: the archive keeps more than asked (#136).
    let cleaned_ok = cleaned.remaining == 0 && cleaned.error.is_none();
    if !cleaned_ok {
        let why = cleaned
            .error
            .as_ref()
            .map_or_else(String::new, |(path, e)| {
                format!(" ({}: {e})", path.display())
            });
        eprintln!(
            "archived files past --archive-days NOT removed: {}{why}",
            cleaned.remaining
        );
    }
    let archive = match deleted {
        Deleted::Archive => Some(mirror::archive_dir(args.to(), now)),
        Deleted::Delete => None,
    };
    let opts = JobOptions {
        verify: true,
        write_checksum_file: false,
        archive_replaced: archive.clone(),
        ..JobOptions::default()
    };
    let started_at = now;
    let mut report = run_with_progress(&plan.copy, &opts)?;
    print_summary(&report);
    let finished = mirror::finish(&plan, &report, archive.as_deref());
    let mut checksums_failed = None;
    let removed_ok = match &finished {
        Ok(finished) => {
            let removals = &finished.removals;
            let mut ok = true;
            for r in removals {
                if let Err(e) = &r.result {
                    eprintln!("NOT REMOVED {}: {e}", r.rel.display());
                    ok = false;
                }
            }
            println!(
                "removed: {}",
                removals.iter().filter(|r| r.result.is_ok()).count()
            );
            if !finished.renamed.is_empty() {
                println!("renamed to match the origin: {}", finished.renamed.len());
            }
            if let Err(e) = mirror::write_checksums(&plan, &report, Some(finished)) {
                eprintln!("checksum file NOT written: {e}");
                checksums_failed = Some(e);
                ok = false;
            }
            ok
        }
        Err(why) => {
            println!("{why}");
            // What was verified is recorded anyway (#114).
            if let Err(e) = mirror::write_checksums(&plan, &report, None) {
                eprintln!("checksum file NOT written: {e}");
                checksums_failed = Some(e);
            }
            false
        }
    };
    // The report says so too, as the app's does (#116).
    if let Some(e) = checksums_failed {
        report.checksum_error = Some(secopy_core::error::IoFailure {
            kind: e.kind(),
            message: format!("the mirror's checksum file: {e}"),
        });
    }
    if let Some(dir) = &args.report {
        let meta = job_meta(args, true, started_at);
        let mut part = mirror::report_part(&finished, archive.is_some());
        part.archive_problem = cleaned.left_in_english("clean-up");
        let full = Report::new(&plan.copy, &report, &meta).with_mirror(part);
        write_report(&full, dir, &report, started_at)?;
    }
    Ok(if report.is_success() && removed_ok && cleaned_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// What a report says about the run.
fn job_meta(args: &Args, verify: bool, started: chrono::DateTime<Local>) -> JobMeta {
    JobMeta {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        source: args
            .sources
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        verify,
        started,
        finished: Local::now(),
    }
}

/// `--report`: the report in `dir`, named like the copy's checksum file so the two pair up
/// (by the time it started when there is none: a mirror, a check).
fn write_report(
    full: &Report,
    dir: &Path,
    report: &JobReport,
    started: chrono::DateTime<Local>,
) -> Result<(), String> {
    let stem = match &report.checksum_file {
        Some(path) => path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        None => checksum_file::file_name(started).replace(&format!(".{}", checksum_file::EXT), ""),
    };
    let (text, _) = full
        .write(dir, &stem)
        .map_err(|e| format!("report not written: {e}"))?;
    println!("report: {}", text.display());
    Ok(())
}

/// One directory → directory source; otherwise only files are allowed (RFD Q6).
fn source_from(args: &Args) -> Result<Source, String> {
    if let [only] = args.sources.as_slice()
        && only.is_dir()
    {
        let mode = if args.contents {
            DirMode::ContentsOnly
        } else {
            DirMode::FolderItself
        };
        return Ok(Source::Directory {
            path: only.clone(),
            mode,
        });
    }
    if let Some(dir) = args.sources.iter().find(|p| p.is_dir()) {
        return Err(format!(
            "{} is a directory; pass either one directory or only files",
            dir.display()
        ));
    }
    Ok(Source::Files(args.sources.clone()))
}

/// Lists what pre-flight found, so nothing is decided silently (FR-16, FR-17).
fn print_preflight(pf: &Preflight, plan: &Plan) {
    for problem in &pf.file_problems {
        let file = &plan.files[problem.id];
        eprintln!(
            "will fail: {}: {}",
            file.entry.rel.display(),
            problem.kind.to_error()
        );
    }
    let identical = pf
        .conflicts
        .iter()
        .filter(|c| c.kind == ConflictKind::Identical)
        .count();
    let differ = pf.conflicts.len() - identical;
    if identical > 0 {
        eprintln!("{identical} files already at the destination will be skipped (not checked)");
    }
    if differ > 0 {
        eprintln!("{differ} different files have the same name at the destination");
    }
    if !pf.stale_partials.is_empty() {
        eprintln!(
            "{} partial files left by an interrupted copy will be replaced",
            pf.stale_partials.len()
        );
    }
}

fn check_progress_line(p: &Progress, elapsed: Duration) -> String {
    let speed = p.verified_bytes as f64 / elapsed.as_secs_f64().max(0.001);
    format!(
        "read {} / {} ({:.1} %)  files {}/{}  {}/s   ",
        fmt_bytes(p.verified_bytes),
        fmt_bytes(p.total_bytes),
        percent(p.verified_bytes, p.total_bytes),
        p.files_done,
        p.total_files,
        fmt_bytes(speed as u64),
    )
}

fn progress_line(p: &Progress, elapsed: Duration) -> String {
    let speed = p.copied_bytes as f64 / elapsed.as_secs_f64().max(0.001);
    format!(
        "copied {} / {} ({:.1} %)  verified {}  files {}/{}  {}/s   ",
        fmt_bytes(p.copied_bytes),
        fmt_bytes(p.total_bytes),
        percent(p.copied_bytes, p.total_bytes),
        fmt_bytes(p.verified_bytes),
        p.files_done,
        p.total_files,
        fmt_bytes(speed as u64),
    )
}

fn print_summary(report: &JobReport) {
    let secs = report.elapsed.as_secs_f64().max(0.001);
    let failed = report.failed().count();
    let skipped = report.skipped().count();
    let ok = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
        .count();
    println!(
        "{ok} files ok, {skipped} skipped, {failed} failed, {} not started",
        report.not_started
    );
    match report.unread.len() {
        0 => {}
        1 => println!("1 item couldn't be read (not copied)"),
        n => println!("{n} items couldn't be read (not copied)"),
    }
    // What was written, not what was planned: a cancelled or failed job wrote less.
    let written: u64 = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
        .map(|o| o.size)
        .sum();
    println!(
        "{} written in {:.2} s ({}/s)",
        fmt_bytes(written),
        secs,
        fmt_bytes((written as f64 / secs) as u64)
    );
    if let Some(bypass) = report.cache_bypass {
        println!("verify cache bypass: {bypass:?}");
    }
    if let Some(path) = &report.checksum_file {
        println!("checksum file: {}", path.display());
    }
    if let Some(w) = report.mhl_written.last() {
        println!(
            "ASC MHL: {}",
            w.manifest.parent().unwrap_or(&w.manifest).display()
        );
    }
    if let Some(e) = &report.mhl_error {
        println!("ASC MHL couldn't be written: {}", e.message);
    }
    for rel in &report.mhl_failed {
        println!("ASC MHL: {} doesn't match its history", rel.display());
    }
    if let Some(e) = &report.checksum_error {
        println!("checksum file NOT written: {e}");
    }
    for (dir, e) in &report.dir_errors {
        println!("empty directory NOT created {}: {e}", dir.display());
    }
    if let Some(e) = &report.durability_error {
        println!("NOT confirmed saved to disk: {e}");
    }
    if let Some(e) = &report.fatal {
        println!("stopped: {e}");
    }
    if report.cancelled {
        println!("cancelled");
    }
}

fn percent(done: u64, total: u64) -> f64 {
    if total == 0 {
        100.0
    } else {
        done as f64 * 100.0 / total as f64
    }
}

/// Decimal units, like Finder and Explorer's "size on disk" dialogs.
fn fmt_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The ignore list (#158): the defaults (none with --include-system-files) and --ignore's.
fn patterns(args: &Args) -> Result<Patterns, String> {
    let base = if args.include_system_files {
        Patterns::none()
    } else {
        Patterns::defaults()
    };
    for p in &args.ignore {
        if let Err(e) = Patterns::new([p.clone()]) {
            let why = match e {
                PatternError::HasSlash => "a pattern is a name: it can't contain /".to_string(),
                PatternError::TooLong => format!("up to {MAX_LEN} characters"),
                PatternError::TooMany => format!("up to {MAX_PATTERNS} patterns"),
                PatternError::BadChar => "a pattern can't contain control characters".to_string(),
            };
            return Err(format!("--ignore {p}: {why}"));
        }
    }
    let all = base.as_slice().iter().chain(&args.ignore).cloned();
    Patterns::new(all).map_err(|_| format!("--ignore: up to {MAX_PATTERNS} patterns"))
}

/// `--check`: every file the directory's checksum files list, read again (FR-34); Ctrl-C
/// cancels. With `--report`, its report goes there.
fn check_run(dir: &Path, ignore: &Patterns, report: Option<&Path>) -> Result<ExitCode, String> {
    use secopy_core::{check, error::FileError, job::FileStatus};
    let plan = check::plan(dir, ignore).map_err(|e| format!("{}: {e}", dir.display()))?;
    if plan.files.is_empty() {
        println!("No checksum files here: there's nothing to verify.");
        return Ok(ExitCode::from(1));
    }
    let started = Local::now();
    let r = with_progress(check_progress_line, |control, on_event| {
        check::run(&plan, &check::CheckOptions::default(), control, on_event)
    })?;
    let c = r.counts();
    println!("intact: {}", c.intact);
    println!("changed: {}", c.changed);
    println!("missing: {}", c.missing);
    println!("couldn't be read: {}", c.failed);
    println!("not checked: {}", r.not_checked.len());
    for o in &r.job.outcomes {
        let path = o.rel.display();
        match &o.status {
            FileStatus::Failed(FileError::Changed { .. }) => println!("CHANGED {path}"),
            FileStatus::Failed(FileError::Missing) => println!("MISSING {path}"),
            FileStatus::Failed(e) => println!("FAILED {path}: {e}"),
            _ => {}
        }
    }
    for p in &r.problems {
        match p.line {
            Some(n) => println!("PROBLEM {}:{n}: {}", p.file.display(), p.reason),
            None => println!("PROBLEM {}: {}", p.file.display(), p.reason),
        }
    }
    if r.job.cancelled {
        println!("cancelled");
    }
    if let Some(reports) = report {
        let meta = JobMeta {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            source: dir.display().to_string(),
            verify: true,
            started,
            finished: Local::now(),
        };
        write_report(
            &Report::for_check(&plan, &r, &meta),
            reports,
            &r.job,
            started,
        )?;
    }
    Ok(if r.is_intact() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}
