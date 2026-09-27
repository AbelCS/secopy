//! The job report, as text for people and JSON for tools (FR-35).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::checksum_file::slash_path;
use crate::job::{FileStatus, JobReport, SkipReason};
use crate::plan::Plan;
use crate::verify::CacheBypass;

/// What the report needs beyond the plan and the job's outcome.
#[derive(Debug, Clone)]
pub struct JobMeta {
    pub app_version: String,
    /// The source as the user picked it, for display.
    pub source: String,
    pub verify: bool,
    pub started: DateTime<Local>,
    pub finished: DateTime<Local>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub files: u64,
    pub copied: u64,
    pub verified: u64,
    /// Already at the destination with the same size and date; not checked (FR-17).
    pub skipped_identical: u64,
    /// A different file has the name, and the user chose Skip.
    pub skipped_different: u64,
    pub failed: u64,
    pub not_started: u64,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportFile {
    /// Relative to the destination, `/`-separated.
    pub path: String,
    /// Set when the copy got another name (Keep both).
    pub copied_to: Option<String>,
    pub size: u64,
    /// `copied`, `verified`, `skipped`, `failed` or `not started`.
    pub status: &'static str,
    /// Why it was skipped or failed.
    pub reason: Option<String>,
    pub xxh64: Option<String>,
    pub in_checksum_file: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub app_version: String,
    pub mode: &'static str,
    pub source: String,
    pub destination: String,
    pub started: String,
    pub finished: String,
    pub duration_secs: f64,
    /// One line: complete, failures, cancelled or stopped.
    pub result: String,
    pub counts: Counts,
    /// `None` in plain Copy mode (FR-26).
    pub cache_bypass: Option<bool>,
    pub checksum_file: Option<String>,
    pub checksum_error: Option<String>,
    /// The checksum file was turned off for this job.
    pub checksum_off: bool,
    /// Partial files left by interrupted jobs that were removed (FR-18).
    pub removed_partials: u64,
    /// In plan order.
    pub files: Vec<ReportFile>,
}

impl Report {
    pub fn new(plan: &Plan, job: &JobReport, meta: &JobMeta) -> Report {
        let by_id: HashMap<usize, _> = job.outcomes.iter().map(|o| (o.id, o)).collect();
        let mut counts = Counts {
            files: plan.files.len() as u64,
            not_started: job.not_started,
            ..Counts::default()
        };
        let files = plan
            .files
            .iter()
            .enumerate()
            .map(|(id, planned)| {
                let path = slash_path(&planned.entry.rel);
                let Some(o) = by_id.get(&id) else {
                    return ReportFile {
                        path,
                        copied_to: None,
                        size: planned.entry.size,
                        status: "not started",
                        reason: None,
                        xxh64: None,
                        in_checksum_file: false,
                    };
                };
                let status = match &o.status {
                    FileStatus::Copied => "copied",
                    FileStatus::Verified => "verified",
                    FileStatus::Skipped(_) => "skipped",
                    FileStatus::Failed(_) => "failed",
                };
                let reason = match &o.status {
                    FileStatus::Failed(e) => Some(e.to_string()),
                    FileStatus::Skipped(SkipReason::Identical) => {
                        Some("already at the destination (same size and date), not checked".into())
                    }
                    FileStatus::Skipped(SkipReason::Differs) => {
                        Some("a different file with this name was kept".into())
                    }
                    _ if !planned.in_checksum_file => {
                        Some("not in the checksum file: the name isn't valid UTF-8".into())
                    }
                    _ => None,
                };
                match &o.status {
                    FileStatus::Copied => counts.copied += 1,
                    FileStatus::Verified => counts.verified += 1,
                    FileStatus::Skipped(SkipReason::Identical) => counts.skipped_identical += 1,
                    FileStatus::Skipped(SkipReason::Differs) => counts.skipped_different += 1,
                    FileStatus::Failed(_) => counts.failed += 1,
                }
                if matches!(o.status, FileStatus::Copied | FileStatus::Verified) {
                    counts.bytes_written += o.size;
                }
                ReportFile {
                    copied_to: (o.final_rel != o.rel).then(|| slash_path(&o.final_rel)),
                    path,
                    size: o.size,
                    status,
                    reason,
                    xxh64: o.hash.map(crate::hash::to_hex),
                    in_checksum_file: o.in_checksum_file,
                }
            })
            .collect();
        Report {
            app_version: meta.app_version.clone(),
            mode: if meta.verify { "copy+verify" } else { "copy" },
            source: meta.source.clone(),
            destination: plan.dest.display().to_string(),
            started: meta.started.to_rfc3339(),
            finished: meta.finished.to_rfc3339(),
            duration_secs: job.elapsed.as_secs_f64(),
            result: result_line(job, &counts),
            cache_bypass: job.cache_bypass.map(|b| b == CacheBypass::Active),
            checksum_file: job.checksum_file.as_ref().map(|p| p.display().to_string()),
            checksum_error: job.checksum_error.clone(),
            checksum_off: job.checksum_off,
            removed_partials: job.removed_partials,
            counts,
            files,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the report is plain data")
    }

    pub fn to_text(&self) -> String {
        let mut t = String::new();
        let c = &self.counts;
        let mode = if self.mode == "copy" {
            "Copy"
        } else {
            "Copy & Verify"
        };
        let _ = writeln!(t, "Secopy {} report", self.app_version);
        let _ = writeln!(t);
        let _ = writeln!(t, "Result:       {}", self.result);
        let _ = writeln!(t, "Mode:         {mode}");
        let _ = writeln!(t, "Source:       {}", self.source);
        let _ = writeln!(t, "Destination:  {}", self.destination);
        let _ = writeln!(t, "Started:      {}", self.started);
        let _ = writeln!(t, "Finished:     {}", self.finished);
        let _ = writeln!(t, "Duration:     {:.1} s", self.duration_secs);
        let _ = writeln!(t);
        let _ = writeln!(t, "Files:        {}", c.files);
        for (label, n) in [
            ("verified", c.verified),
            ("copied", c.copied),
            (
                "skipped, already at the destination (not checked)",
                c.skipped_identical,
            ),
            ("skipped, a different file was kept", c.skipped_different),
            ("failed", c.failed),
            ("not started", c.not_started),
        ] {
            if n > 0 {
                let _ = writeln!(t, "  {n} {label}");
            }
        }
        let _ = writeln!(t, "Written:      {} bytes", c.bytes_written);
        match self.cache_bypass {
            Some(true) => {
                let _ = writeln!(t, "Verify read:  from the device (cache bypassed)");
            }
            Some(false) => {
                let _ = writeln!(
                    t,
                    "Verify read:  the OS cache could not be bypassed on this drive"
                );
            }
            None => {}
        }
        match (&self.checksum_file, &self.checksum_error) {
            (Some(path), _) => {
                let _ = writeln!(t, "Checksum file: {path}");
            }
            (None, Some(e)) => {
                let _ = writeln!(t, "Checksum file NOT written: {e}");
            }
            (None, None) if self.checksum_off => {
                let _ = writeln!(t, "Checksum file: off (not written)");
            }
            (None, None) => {}
        }
        if self.removed_partials > 0 {
            let _ = writeln!(
                t,
                "Removed {} partial file(s) left by an interrupted copy",
                self.removed_partials
            );
        }
        let problems: Vec<&ReportFile> = self
            .files
            .iter()
            .filter(|f| f.reason.is_some() && f.status != "skipped")
            .collect();
        if !problems.is_empty() {
            let _ = writeln!(t);
            let _ = writeln!(t, "PROBLEMS");
            for f in problems {
                let reason = f.reason.as_deref().unwrap_or_default();
                let _ = writeln!(t, "  {}: {reason}", f.path);
            }
        }
        let _ = writeln!(t);
        let _ = writeln!(t, "FILES");
        for f in &self.files {
            let hash = f.xxh64.as_deref().unwrap_or("-");
            let _ = write!(t, "  {:<11} {hash:<16}  {}", f.status, f.path);
            if let Some(to) = &f.copied_to {
                let _ = write!(t, " -> {to}");
            }
            let _ = writeln!(t);
        }
        t
    }

    /// Saves `<stem>_report.txt` and `<stem>_report.json` next to the checksum file
    /// (Settings, RFD §5.5). Never overwrites.
    pub fn write_next_to(&self, checksum_file: &Path) -> io::Result<(PathBuf, PathBuf)> {
        let stem = checksum_file
            .file_stem()
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?
            .to_string_lossy();
        self.write(checksum_file.parent().unwrap_or(Path::new(".")), &stem)
    }

    /// Saves `<dir>/<stem>_report.txt` and `.json`. Never overwrites.
    pub fn write(&self, dir: &Path, stem: &str) -> io::Result<(PathBuf, PathBuf)> {
        let text = dir.join(format!("{stem}_report.txt"));
        let json = dir.join(format!("{stem}_report.json"));
        write_new(&text, &self.to_text())?;
        write_new(&json, &self.to_json())?;
        Ok((text, json))
    }
}

fn result_line(job: &JobReport, counts: &Counts) -> String {
    if let Some(fatal) = &job.fatal {
        return format!("stopped: {fatal}");
    }
    if job.cancelled {
        return "cancelled".to_string();
    }
    match counts.failed {
        0 => "complete".to_string(),
        1 => "1 file failed".to_string(),
        n => format!("{n} files failed"),
    }
}

fn write_new(path: &Path, body: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(body.as_bytes())?;
    file.sync_all()
}
