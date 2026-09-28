//! Command-line front-end for the Secopy engine, used for development and benchmarks (RFD §10, M0).

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Local;
use clap::{Parser, ValueEnum};
use secopy_core::checksum_file;
use secopy_core::filter::ExtensionFilter;
use secopy_core::job::{self, Event, FileStatus, JobControl, JobOptions, JobReport, Progress};
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
    about = "Fast file copy with xxHash64 verification"
)]
struct Args {
    /// One directory, or one or more files.
    #[arg(required = true)]
    sources: Vec<PathBuf>,
    /// Destination directory (must exist).
    #[arg(long, short = 't')]
    to: PathBuf,
    /// Copy only what is inside the source directory, not the directory itself.
    #[arg(long)]
    contents: bool,
    /// Re-read every copy from the destination and compare hashes.
    #[arg(long)]
    verify: bool,
    /// Only copy these extensions, e.g. "mov,wav". "(none)" means files without extension.
    #[arg(long)]
    ext: Option<String>,
    /// Do not write the .xxh64 checksum file.
    #[arg(long)]
    no_checksum: bool,
    /// Also copy system files (.DS_Store, Thumbs.db, …). Hidden files are always copied.
    #[arg(long)]
    include_system_files: bool,
    /// What to do with files that already exist at the destination but differ.
    /// Identical files (same size and date) are always skipped.
    #[arg(long, value_enum, default_value_t = OnConflict::KeepBoth)]
    on_conflict: OnConflict,
    /// Also write the job report (text and JSON) into this folder.
    #[arg(long, value_name = "DIR")]
    report: Option<PathBuf>,
    /// Mirror the one source directory to the destination: new and changed files copied
    /// and verified, files gone from the source archived (or deleted with --delete).
    #[arg(long)]
    mirror: bool,
    /// With --mirror: delete files gone from the source instead of archiving them.
    #[arg(long, requires = "mirror")]
    delete: bool,
    /// With --mirror: days to keep archived files.
    #[arg(long, requires = "mirror", default_value_t = 30, value_name = "N")]
    archive_days: u32,
    /// With --mirror: also compare the contents of files whose size and date match.
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

fn run(args: Args) -> Result<ExitCode, String> {
    if !args.to.is_dir() {
        return Err(format!(
            "destination {} is not a directory",
            args.to.display()
        ));
    }
    if args.mirror {
        return mirror_run(&args);
    }
    let source = source_from(&args)?;
    let scan = scan::scan(
        &source,
        &ScanOptions {
            include_system_files: args.include_system_files,
        },
    )
    .map_err(|e| e.to_string())?;
    for p in &scan.problems {
        eprintln!("warning: {}: {}", p.path.display(), p.message);
    }
    for link in &scan.skipped_symlinks {
        eprintln!("skipped symlink: {}", link.display());
    }
    let filter = args
        .ext
        .as_deref()
        .map(ExtensionFilter::parse_list)
        .unwrap_or_default();
    let selection = scan.select(&filter);
    eprintln!(
        "{} files, {} ({} system files skipped)",
        selection.files.len(),
        fmt_bytes(selection.total_bytes),
        scan.skipped_system
    );
    let pf = preflight(&source, &selection, &args.to).map_err(|e| e.to_string())?;
    let plan = Plan::resolve(&selection, &pf, args.on_conflict.into());
    print_preflight(&pf, &plan);
    if let Some(blocker) = plan.blockers().first() {
        return Err(blocker.to_string());
    }

    let opts = JobOptions {
        verify: args.verify,
        write_checksum_file: !args.no_checksum,
        ..JobOptions::default()
    };
    let started_at = Local::now();
    let report = run_with_progress(&plan, &opts)?;
    print_summary(&report, plan.bytes_to_write());
    if let Some(dir) = &args.report {
        let meta = JobMeta {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            source: args
                .sources
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            verify: args.verify,
            started: started_at,
            finished: Local::now(),
        };
        // Named like the checksum file, so the two are easy to pair up.
        let stem = match &report.checksum_file {
            Some(path) => path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            None => checksum_file::file_name(started_at).replace(".xxh64", ""),
        };
        let (text, _) = Report::new(&plan, &report, &meta)
            .write(dir, &stem)
            .map_err(|e| format!("report not written: {e}"))?;
        println!("report: {}", text.display());
    }
    Ok(if report.is_success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// Runs `plan`, showing progress and failures; Ctrl-C cancels.
fn run_with_progress(plan: &Plan, opts: &JobOptions) -> Result<JobReport, String> {
    let control = Arc::new(JobControl::new());
    let handler_control = control.clone();
    ctrlc::set_handler(move || handler_control.cancel()).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let last_print = Mutex::new(Instant::now());
    let report = job::run_job(plan, opts, &control, &|event| match event {
        Event::Progress(p) => {
            let mut last = last_print.lock().unwrap();
            if last.elapsed() >= Duration::from_millis(500) {
                *last = Instant::now();
                eprint!("\r{}", progress_line(&p, started.elapsed()));
            }
        }
        Event::FileFinished(o) => {
            if let FileStatus::Failed(e) = &o.status {
                eprintln!("\rFAILED {}: {e}", o.rel.display());
            }
        }
    });
    eprintln!();
    Ok(report)
}

/// `--mirror`: the destination becomes a copy of the one source directory (RFD §5.8).
fn mirror_run(args: &Args) -> Result<ExitCode, String> {
    let [origin] = args.sources.as_slice() else {
        return Err("--mirror takes one source directory".into());
    };
    let deleted = if args.delete {
        Deleted::Delete
    } else {
        Deleted::Archive {
            days: args.archive_days,
        }
    };
    let options = MirrorOptions {
        deleted,
        deep_check: args.deep,
    };
    let plan = mirror::plan(origin, &args.to, &options)?;
    let new = plan
        .changes
        .iter()
        .filter(|(_, c)| *c == Change::New)
        .count();
    println!("+ new: {new}");
    println!("~ changed: {}", plan.changes.len() - new);
    let how = if args.delete { "deleted" } else { "archived" };
    println!("- removed: {} ({how})", plan.removals.len());
    println!(
        "= unchanged: {}",
        plan.copy.files.len() - plan.changes.len()
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
    let archive = match deleted {
        Deleted::Archive { days } => {
            mirror::clean_archives(&args.to, days, now);
            Some(mirror::archive_dir(&args.to, now))
        }
        Deleted::Delete => None,
    };
    let opts = JobOptions {
        verify: true,
        write_checksum_file: false,
        archive_replaced: archive.clone(),
        ..JobOptions::default()
    };
    let report = run_with_progress(&plan.copy, &opts)?;
    print_summary(&report, plan.copy.bytes_to_write());
    let removed_ok = match mirror::finish(&plan, &report, archive.as_deref()) {
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
            ok
        }
        Err(why) => {
            println!("{why}");
            false
        }
    };
    Ok(if report.is_success() && removed_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
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

fn print_summary(report: &JobReport, total_bytes: u64) {
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
    println!(
        "{} in {:.2} s ({}/s)",
        fmt_bytes(total_bytes),
        secs,
        fmt_bytes((total_bytes as f64 / secs) as u64)
    );
    if let Some(bypass) = report.cache_bypass {
        println!("verify cache bypass: {bypass:?}");
    }
    if let Some(path) = &report.checksum_file {
        println!("checksum file: {}", path.display());
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
