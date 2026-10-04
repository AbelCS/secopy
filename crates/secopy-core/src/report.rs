//! The job report, as text for people and JSON for tools (FR-35).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
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
    /// Stopped by Cancel while it was copied.
    pub cancelled: u64,
    pub not_started: u64,
    pub bytes_written: u64,
    /// A check: the files read in full (intact or changed). A copy: 0.
    pub bytes_read: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportFile {
    /// Relative to the destination, `/`-separated.
    pub path: String,
    /// Set when the copy got another name (Keep both).
    pub copied_to: Option<String>,
    pub size: u64,
    /// `copied`, `verified`, `skipped`, `failed`, `cancelled` or `not started`; for a
    /// Verify, `intact`, `changed`, `missing`, `failed`, `cancelled` or `not started`.
    pub status: &'static str,
    /// Why it was skipped or failed.
    pub reason: Option<String>,
    pub xxh128: Option<String>,
    pub in_checksum_file: bool,
    /// A check: the checksum file that listed it, relative to the checked directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listed_in: Option<String>,
}

/// The JSON report's format: raised only when a field changes meaning or goes away.
pub const FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// `FORMAT`, for tools that read the JSON.
    pub format: u32,
    pub app_version: String,
    /// `copy`, `copy+verify`, `mirror` or `check`.
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
    /// What the scan couldn't read, so it wasn't copied (#58).
    pub unread: Vec<Unread>,
    /// The device reported an error while the copy was made durable (#58).
    pub durability_error: Option<String>,
    /// A mirror's removals after its copy phase (plan 7).
    pub mirror: Option<MirrorPart>,
    /// Empty directories that couldn't be created (#58).
    pub dir_errors: Vec<Unread>,
    /// A check's own parts (plan 8): its checksum files, their problems, what nothing lists.
    pub check: Option<CheckPart>,
    /// ASC MHL, when the copy writes it (#154, #192).
    pub mhl: Option<MhlPart>,
    /// What Cancel's "Also remove the files already copied" did (#54, #192).
    pub undone: Option<UndonePart>,
}

/// What a cancelled job's removal did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UndonePart {
    pub removed: u64,
    /// Replaced files put back from the archive.
    pub restored: u64,
    /// Replaced files with no old version to put back.
    pub not_restored: u64,
    pub failed: Vec<Unread>,
}

/// What a copy's ASC MHL did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MhlPart {
    /// The `ascmhl` directories a generation was written to.
    pub histories: Vec<String>,
    /// Why it couldn't be written (nothing of it was left).
    pub error: Option<String>,
    /// Files that don't match their earlier hash in the history, from the copy root.
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckPart {
    pub checksum_files: Vec<String>,
    pub not_checked: Vec<String>,
    pub problems: Vec<Unread>,
}

/// What a mirror did after copying: removals, and names changed to the origin's spelling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MirrorPart {
    /// Removed files went to the archive (or were deleted).
    pub archived: bool,
    pub removed: Vec<String>,
    pub not_removed: Vec<Unread>,
    pub renamed: Vec<(String, String)>,
    /// Names that couldn't be changed to the origin's: "before → after", and why (#192).
    pub not_renamed: Vec<Unread>,
    /// Why nothing was removed: the copy phase didn't end cleanly.
    pub nothing_removed: Option<String>,
    /// The archive kept more than asked: expired files, or a deletion, that didn't all go
    /// (#136).
    pub archive_problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unread {
    pub path: String,
    pub reason: String,
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
                        xxh128: None,
                        in_checksum_file: false,
                        listed_in: None,
                    };
                };
                let status = match &o.status {
                    FileStatus::Copied => "copied",
                    FileStatus::Verified => "verified",
                    FileStatus::Skipped(_) => "skipped",
                    FileStatus::Failed(_) => "failed",
                    FileStatus::Cancelled => "cancelled",
                };
                let reason = match &o.status {
                    FileStatus::Failed(e) => Some(e.to_string()),
                    FileStatus::Skipped(SkipReason::Identical) => {
                        Some("already at the destination (same size and date), not checked".into())
                    }
                    FileStatus::Skipped(SkipReason::Differs) => {
                        Some("a different file with this name was kept".into())
                    }
                    _ if !planned.in_checksum_file && planned.entry.rel.to_str().is_none() => {
                        Some("not in the checksum file: the name isn't valid UTF-8".into())
                    }
                    // A history's own files, added to after the copy (#154).
                    _ if !planned.in_checksum_file => {
                        Some("not in the checksum file: part of an ASC MHL history".into())
                    }
                    _ => None,
                };
                match &o.status {
                    FileStatus::Copied => counts.copied += 1,
                    FileStatus::Verified => counts.verified += 1,
                    FileStatus::Skipped(SkipReason::Identical) => counts.skipped_identical += 1,
                    FileStatus::Skipped(SkipReason::Differs) => counts.skipped_different += 1,
                    FileStatus::Failed(_) => counts.failed += 1,
                    FileStatus::Cancelled => counts.cancelled += 1,
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
                    xxh128: o.hash.map(|h| h.to_hex()),
                    in_checksum_file: o.in_checksum_file,
                    listed_in: None,
                }
            })
            .collect();
        Report {
            format: FORMAT,
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
            checksum_error: job.checksum_error.as_ref().map(ToString::to_string),
            checksum_off: job.checksum_off,
            removed_partials: job.removed_partials,
            counts,
            files,
            durability_error: job.durability_error.as_ref().map(ToString::to_string),
            mirror: None,
            check: None,
            undone: None,
            mhl: (!job.mhl_off).then(|| MhlPart {
                histories: job
                    .mhl_written
                    .iter()
                    .map(|w| w.chain.parent().unwrap_or(&w.chain).display().to_string())
                    .collect(),
                error: job.mhl_error.as_ref().map(ToString::to_string),
                failed: job.mhl_failed.iter().map(|p| slash_path(p)).collect(),
            }),
            dir_errors: job
                .dir_errors
                .iter()
                .map(|(rel, why)| Unread {
                    path: slash_path(rel),
                    reason: why.to_string(),
                })
                .collect(),
            unread: job
                .unread
                .iter()
                .map(|p| Unread {
                    path: p.path.display().to_string(),
                    reason: p.message.clone(),
                })
                .collect(),
        }
    }

    /// A check's report (plan 8): every listed file with what was found.
    pub fn for_check(
        plan: &crate::check::CheckPlan,
        r: &crate::check::CheckReport,
        meta: &JobMeta,
    ) -> Report {
        use crate::error::FileError;
        let by_id: HashMap<usize, _> = r.job.outcomes.iter().map(|o| (o.id, o)).collect();
        let c = r.counts();
        let files = plan
            .files
            .iter()
            .enumerate()
            .map(|(id, f)| {
                let (status, reason) = match by_id.get(&id).map(|o| &o.status) {
                    Some(FileStatus::Verified) => ("intact", None),
                    Some(FileStatus::Failed(e @ FileError::Changed { .. })) => {
                        ("changed", Some(e.to_string()))
                    }
                    Some(FileStatus::Failed(e @ FileError::Missing)) => {
                        ("missing", Some(e.to_string()))
                    }
                    Some(FileStatus::Failed(e)) => ("failed", Some(e.to_string())),
                    Some(FileStatus::Cancelled) => ("cancelled", None),
                    _ => ("not started", None),
                };
                ReportFile {
                    path: slash_path(&f.rel),
                    copied_to: None,
                    size: f.size,
                    status,
                    reason,
                    xxh128: Some(f.expected.to_hex()),
                    in_checksum_file: true,
                    listed_in: Some(slash_path(&f.from)),
                }
            })
            .collect();
        let mut parts: Vec<String> = [
            (c.changed, "changed"),
            (c.missing, "missing"),
            (c.failed, "couldn't be read"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, what)| format!("{n} {what}"))
        .collect();
        if !r.problems.is_empty() {
            parts.push(format!("{} checksum file problems", r.problems.len()));
        }
        if r.job.cancelled {
            parts.push("cancelled".into());
        }
        let shown = |p: &Path| slash_path(p);
        Report {
            format: FORMAT,
            app_version: meta.app_version.clone(),
            mode: "check",
            source: plan.dir.display().to_string(),
            destination: plan.dir.display().to_string(),
            started: meta.started.to_rfc3339(),
            finished: meta.finished.to_rfc3339(),
            duration_secs: r.job.elapsed.as_secs_f64(),
            result: if r.is_intact() {
                "all intact".into()
            } else {
                parts.join(", ")
            },
            counts: Counts {
                files: plan.files.len() as u64,
                verified: c.intact,
                failed: c.changed + c.missing + c.failed,
                not_started: r.job.not_started,
                bytes_read: r
                    .job
                    .outcomes
                    .iter()
                    .filter(|o| {
                        matches!(
                            o.status,
                            FileStatus::Verified | FileStatus::Failed(FileError::Changed { .. })
                        )
                    })
                    .map(|o| o.size)
                    .sum(),
                ..Counts::default()
            },
            cache_bypass: r.job.cache_bypass.map(|b| b == CacheBypass::Active),
            // A check writes nothing: no checksum file was asked for, or turned off.
            checksum_file: None,
            checksum_error: None,
            checksum_off: false,
            removed_partials: 0,
            files,
            unread: Vec::new(),
            durability_error: None,
            mirror: None,
            dir_errors: Vec::new(),
            mhl: None,
            undone: None,
            check: Some(CheckPart {
                checksum_files: plan.checksum_files.iter().map(|p| shown(p)).collect(),
                not_checked: r.not_checked.iter().map(|p| shown(p)).collect(),
                problems: r
                    .problems
                    .iter()
                    .map(|p| Unread {
                        path: match p.line {
                            Some(n) => format!("{}:{n}", shown(&p.file)),
                            None => shown(&p.file),
                        },
                        reason: p.reason.clone(),
                    })
                    .collect(),
            }),
        }
    }

    /// Adds what Cancel's removal did.
    pub fn with_undone(mut self, u: &crate::job::Undone) -> Report {
        self.undone = Some(UndonePart {
            removed: u.removed,
            restored: u.restored,
            not_restored: u.not_restored,
            failed: u
                .failed
                .iter()
                .map(|(path, why)| Unread {
                    path: slash_path(path),
                    reason: why.clone(),
                })
                .collect(),
        });
        self
    }

    /// Adds a mirror's removals; one that failed means the result isn't "complete".
    pub fn with_mirror(mut self, part: MirrorPart) -> Report {
        if self.result == "complete" && !part.not_removed.is_empty() {
            self.result = match part.not_removed.len() {
                1 => "1 file couldn't be removed".to_string(),
                n => format!("{n} files couldn't be removed"),
            };
        } else if self.result == "complete" && !part.not_renamed.is_empty() {
            self.result = match part.not_renamed.len() {
                1 => "1 name couldn't be changed to match the origin".to_string(),
                n => format!("{n} names couldn't be changed to match the origin"),
            };
        } else if self.result == "complete" && part.archive_problem.is_some() {
            self.result = "archived files not removed as asked".to_string();
        }
        self.mode = "mirror";
        self.mirror = Some(part);
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the report is plain data")
    }

    pub fn to_text(&self) -> String {
        let mut t = String::new();
        let c = &self.counts;
        // A check reads one directory and writes nothing.
        let check = self.mode == "check";
        let mode = match self.mode {
            "copy" => "Copy",
            "mirror" => "Mirror",
            "check" => "Verify",
            _ => "Copy & Verify",
        };
        let _ = writeln!(t, "Secopy {} report", self.app_version);
        let _ = writeln!(t);
        let _ = writeln!(t, "Result:       {}", self.result);
        let _ = writeln!(t, "Mode:         {mode}");
        if check {
            let _ = writeln!(t, "Directory:    {}", self.source);
        } else {
            let _ = writeln!(t, "Source:       {}", self.source);
            let _ = writeln!(t, "Destination:  {}", self.destination);
        }
        let _ = writeln!(t, "Started:      {}", self.started);
        let _ = writeln!(t, "Finished:     {}", self.finished);
        let _ = writeln!(t, "Duration:     {:.1} s", self.duration_secs);
        let _ = writeln!(t);
        let _ = writeln!(t, "Files:        {}", c.files);
        for (label, n) in [
            (if check { "intact" } else { "verified" }, c.verified),
            ("copied", c.copied),
            (
                "skipped, already at the destination (not checked)",
                c.skipped_identical,
            ),
            ("skipped, a different file was kept", c.skipped_different),
            ("failed", c.failed),
            ("cancelled", c.cancelled),
            ("not started", c.not_started),
        ] {
            if n > 0 {
                let _ = writeln!(t, "  {n} {label}");
            }
        }
        if check {
            let _ = writeln!(t, "Read:         {} bytes", c.bytes_read);
        } else {
            let _ = writeln!(t, "Written:      {} bytes", c.bytes_written);
        }
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
            (None, None) if self.checksum_off && !check => {
                let _ = writeln!(t, "Checksum file: off (not written)");
            }
            // Nothing was copied, so there's nothing to list: said, not left out (#135).
            (None, None) if !check && c.copied + c.verified == 0 => {
                let _ = writeln!(t, "Checksum file: none (nothing copied)");
            }
            (None, None) => {}
        }
        if let Some(m) = &self.mhl {
            for h in &m.histories {
                let _ = writeln!(t, "ASC MHL: {h}");
            }
            if let Some(e) = &m.error {
                let _ = writeln!(t, "ASC MHL NOT written: {e}");
            }
            if !m.failed.is_empty() {
                let _ = writeln!(t, "Doesn't match its ASC MHL history: {}", m.failed.len());
                for f in &m.failed {
                    let _ = writeln!(t, "  {f}");
                }
            }
        }
        if let Some(e) = &self.durability_error {
            let _ = writeln!(t, "NOT CONFIRMED SAVED TO DISK: {e}");
        }
        if self.removed_partials > 0 {
            let _ = writeln!(
                t,
                "Removed {} partial file(s) left by an interrupted copy",
                self.removed_partials
            );
        }
        if let Some(c) = &self.check {
            let _ = writeln!(t);
            let _ = writeln!(t, "CHECKSUM FILES");
            for f in &c.checksum_files {
                let _ = writeln!(t, "  {f}");
            }
            if !c.problems.is_empty() {
                let _ = writeln!(t);
                let _ = writeln!(t, "PROBLEMS IN CHECKSUM FILES");
                for p in &c.problems {
                    let _ = writeln!(t, "  {}: {}", p.path, p.reason);
                }
            }
            if !c.not_checked.is_empty() {
                let _ = writeln!(t);
                let _ = writeln!(t, "NOT CHECKED (no checksum)");
                for p in &c.not_checked {
                    let _ = writeln!(t, "  {p}");
                }
            }
        }
        if !self.unread.is_empty() {
            let _ = writeln!(t);
            let _ = writeln!(t, "COULDN'T BE READ (not copied)");
            for u in &self.unread {
                let _ = writeln!(t, "  {}: {}", u.path, u.reason);
            }
        }
        if !self.dir_errors.is_empty() {
            let _ = writeln!(t);
            let _ = writeln!(t, "EMPTY DIRECTORIES NOT CREATED");
            for d in &self.dir_errors {
                let _ = writeln!(t, "  {}: {}", d.path, d.reason);
            }
        }
        if let Some(m) = &self.mirror {
            let _ = writeln!(t);
            match &m.nothing_removed {
                Some(why) => {
                    let _ = writeln!(t, "{why}");
                }
                None => {
                    let how = if m.archived { "archived" } else { "deleted" };
                    let _ = writeln!(
                        t,
                        "Removed from the destination ({how}): {}",
                        m.removed.len()
                    );
                    for path in &m.removed {
                        let _ = writeln!(t, "  {path}");
                    }
                }
            }
            if !m.not_removed.is_empty() {
                let _ = writeln!(t, "Not removed: {}", m.not_removed.len());
                for n in &m.not_removed {
                    let _ = writeln!(t, "  {}: {}", n.path, n.reason);
                }
            }
            if let Some(problem) = &m.archive_problem {
                let _ = writeln!(t, "{problem}");
            }
            if !m.renamed.is_empty() {
                let _ = writeln!(t, "Renamed to match the origin: {}", m.renamed.len());
                for (from, to) in &m.renamed {
                    let _ = writeln!(t, "  {from} → {to}");
                }
            }
            if !m.not_renamed.is_empty() {
                let _ = writeln!(
                    t,
                    "NOT renamed to match the origin: {}",
                    m.not_renamed.len()
                );
                for n in &m.not_renamed {
                    let _ = writeln!(t, "  {}: {}", n.path, n.reason);
                }
            }
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
                let _ = write!(t, "  {}: {reason}", f.path);
                if let Some(sums) = &f.listed_in {
                    let _ = write!(t, " (listed in {sums})");
                }
                let _ = writeln!(t);
            }
        }
        let _ = writeln!(t);
        let _ = writeln!(t, "FILES");
        for f in &self.files {
            let hash = f.xxh128.as_deref().unwrap_or("-");
            let _ = write!(t, "  {:<11} {hash:<32}  {}", f.status, f.path);
            if let Some(to) = &f.copied_to {
                let _ = write!(t, " -> {to}");
            }
            let _ = writeln!(t);
        }
        if let Some(u) = &self.undone {
            let _ = writeln!(
                t,
                "\nRemoved after cancelling: {} copied, {} put back from the archive, {} replaced files not put back",
                u.removed, u.restored, u.not_restored
            );
            for f in &u.failed {
                let _ = writeln!(t, "  {} — NOT REMOVED: {}", f.path, f.reason);
            }
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

    /// Saves `<dir>/<stem>_report.txt` and `.json`. Never overwrites: with those names taken
    /// (two jobs in the same second, #116), `<stem>_report (2).txt` and so on.
    pub fn write(&self, dir: &Path, stem: &str) -> io::Result<(PathBuf, PathBuf)> {
        for n in 1u32.. {
            let name = match n {
                1 => format!("{stem}_report"),
                n => format!("{stem}_report ({n})"),
            };
            let (text, json) = (
                dir.join(format!("{name}.txt")),
                dir.join(format!("{name}.json")),
            );
            if fs::symlink_metadata(&json).is_ok() {
                continue;
            }
            match write_pair(&text, &self.to_text(), &json, &self.to_json()) {
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                other => other?,
            }
            return Ok((text, json));
        }
        unreachable!("some name is free")
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
        0 if !job.unread.is_empty() => match job.unread.len() {
            1 => "1 item couldn't be read".to_string(),
            n => format!("{n} items couldn't be read"),
        },
        0 if !job.dir_errors.is_empty() => match job.dir_errors.len() {
            1 => "1 directory couldn't be created".to_string(),
            n => format!("{n} directories couldn't be created"),
        },
        0 if job.checksum_error.is_some() => "checksum file not written".to_string(),
        0 if job.durability_error.is_some() => "not confirmed saved to disk".to_string(),
        0 if !job.mhl_failed.is_empty() => match job.mhl_failed.len() {
            1 => "1 file doesn't match its ASC MHL history".to_string(),
            n => format!("{n} files don't match their ASC MHL history"),
        },
        0 if job.mhl_error.is_some() => "ASC MHL not written".to_string(),
        0 => "complete".to_string(),
        1 => "1 file failed".to_string(),
        n => format!("{n} files failed"),
    }
}

/// Both files, or neither: never a text without its JSON, nor a file cut short. A name
/// that is taken (`AlreadyExists`) is left alone.
fn write_pair(text: &Path, text_body: &str, json: &Path, json_body: &str) -> io::Result<()> {
    write_new(text, text_body)?;
    write_new(json, json_body).inspect_err(|_| {
        let _ = fs::remove_file(text);
    })
}

/// Written whole under a temporary name, then given `path` unless it's taken: never a
/// report cut short under its name.
fn write_new(path: &Path, body: &str) -> io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
    let tmp = crate::os::write_temp(dir.unwrap_or(Path::new(".")), |f| {
        f.write_all(body.as_bytes())
    })?;
    let published = crate::os::publish_noreplace(&tmp, path);
    if published.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    published
}

#[cfg(test)]
mod pair_tests {
    use super::*;

    /// Code review (#192): the text and the JSON are saved as a pair or not at all; a JSON
    /// that can't be written takes the text back with it.
    #[test]
    fn a_report_whose_json_fails_leaves_no_text() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("r_report.txt");
        let json = dir.path().join("gone/r_report.json");
        assert!(write_pair(&text, "t", &json, "j").is_err());
        assert!(!text.exists());
    }
}
