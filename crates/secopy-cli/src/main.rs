//! Command-line front-end for the Secopy engine, used for development and benchmarks (RFD §10, M0).

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::Parser;
use secopy_core::filter::ExtensionFilter;
use secopy_core::job::{self, Event, FileStatus, JobControl, JobOptions, JobReport, Progress};
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
    /// Include hidden files and folders.
    #[arg(long)]
    include_hidden: bool,
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
    let source = source_from(&args)?;
    let scan = scan::scan(
        &source,
        &ScanOptions {
            include_hidden: args.include_hidden,
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
        "{} files, {} ({} hidden items skipped)",
        selection.files.len(),
        fmt_bytes(selection.total_bytes),
        scan.skipped_hidden
    );

    let opts = JobOptions {
        verify: args.verify,
        write_checksum_file: !args.no_checksum,
        ..JobOptions::default()
    };
    let control = Arc::new(JobControl::new());
    let handler_control = control.clone();
    ctrlc::set_handler(move || handler_control.cancel()).map_err(|e| e.to_string())?;

    let started = Instant::now();
    let last_print = Mutex::new(Instant::now());
    let report = job::run_job(
        &selection,
        &args.to,
        &opts,
        &control,
        &|event| match event {
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
        },
    );
    eprintln!();
    print_summary(&report, selection.total_bytes);
    Ok(if report.is_success() {
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
    println!(
        "{} files ok, {} failed, {} not started",
        report.outcomes.len() - failed,
        failed,
        report.not_started
    );
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
