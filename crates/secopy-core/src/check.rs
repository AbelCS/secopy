//! Verify an existing copy (FR-34, plan 8): re-read every file the checksum files in a
//! directory list, and say which are intact, changed or missing, and which nothing lists.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime};

use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

use crate::control::JobControl;
use crate::error::FileError;
use crate::hash::to_hex;
use crate::job::{ActiveFile, Event, FileOutcome, FileStatus, JobReport, Phase, Progress};
use crate::mirror::ARCHIVE_DIR;
use crate::system::is_system_file;
use crate::verify::{CacheBypass, hash_from_device};

/// Lines of a checksum file that couldn't be used: 1-based line number, and why.
pub type BadLines = Vec<(usize, String)>;

/// Parses xxhsum/GNU lines, `<16 hex>  <path>`, with the coreutils escaping a leading `\`
/// announces (`\\`, `\n`, `\r`). Returns the entries and the bad lines (1-based, why).
pub fn parse(text: &str) -> (Vec<(PathBuf, u64)>, BadLines) {
    let (lines, bad) = parse_lines(text);
    (
        lines
            .into_iter()
            .map(|(_, path, hash)| (path, hash))
            .collect(),
        bad,
    )
}

/// `parse`, with each entry's 1-based line number.
fn parse_lines(text: &str) -> (Vec<(usize, PathBuf, u64)>, BadLines) {
    let mut entries = Vec::new();
    let mut bad = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let (escaped, line) = match line.strip_prefix('\\') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        let Some((hex, path)) = line.split_once("  ") else {
            bad.push((i + 1, "not a \"<checksum>  <path>\" line".into()));
            continue;
        };
        let hash = (hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| u64::from_str_radix(hex, 16).ok())
            .flatten();
        let Some(hash) = hash else {
            bad.push((i + 1, format!("\"{hex}\" isn't an xxHash64 checksum")));
            continue;
        };
        let path = if escaped {
            unescape(path)
        } else {
            Some(path.to_string())
        };
        match path {
            Some(path) if !path.is_empty() => entries.push((i + 1, PathBuf::from(path), hash)),
            _ => bad.push((i + 1, "the path can't be read".into())),
        }
    }
    (entries, bad)
}

fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            _ => return None,
        }
    }
    Some(out)
}

/// A mirror's checksum file in its destination (plan 8).
pub const MIRROR_CHECKSUMS: &str = ".secopy-checksums.xxh64";

/// One file a checksum file lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Relative to the checked directory.
    pub rel: PathBuf,
    pub expected: u64,
    /// Its size when planned; 0 when it was missing then.
    pub size: u64,
    /// The checksum file it came from, relative to the checked directory.
    pub from: PathBuf,
}

/// Something that keeps part of the directory from being checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The checksum file (or directory) concerned, relative to the checked directory.
    pub file: PathBuf,
    /// 1-based line in `file`; `None` when the whole file couldn't be read.
    pub line: Option<usize>,
    pub reason: String,
}

/// What a check reads, worked out before anything is read.
#[derive(Debug, Clone, Default)]
pub struct CheckPlan {
    pub dir: PathBuf,
    pub checksum_files: Vec<PathBuf>,
    pub files: Vec<Listed>,
    /// Files no checksum file lists, relative to the checked directory.
    pub not_checked: Vec<PathBuf>,
    pub problems: Vec<Problem>,
    pub total_bytes: u64,
}

pub fn plan(dir: &Path) -> io::Result<CheckPlan> {
    fs::read_dir(dir)?; // there, and readable
    let mut sums: Vec<(PathBuf, Option<SystemTime>)> = Vec::new();
    let mut others = Vec::new();
    let mut problems = Vec::new();
    let walk = WalkDir::new(dir)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        // The archive, and the system's own directories (.Spotlight-V100, .Trashes…): not the
        // backup's, and often unreadable.
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && (e.file_name() == ARCHIVE_DIR || is_system_file(e.file_name())))
        });
    for entry in walk {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let file = e
                    .path()
                    .and_then(|p| p.strip_prefix(dir).ok())
                    .map(Path::to_path_buf);
                problems.push(Problem {
                    file: file.unwrap_or_default(),
                    line: None,
                    reason: format!("couldn't be read: {e}"),
                });
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .to_path_buf();
        let name = entry.file_name().to_string_lossy();
        if name.ends_with(".xxh64") {
            sums.push((rel, entry.metadata().ok().and_then(|m| m.modified().ok())));
        } else if !is_system_file(entry.file_name())
            && !(name.starts_with("secopy_")
                && (name.ends_with("_report.txt") || name.ends_with("_report.json")))
        {
            others.push(rel);
        }
    }
    // Oldest first, so the newest checksum file's entry is the one kept.
    sums.sort_by_key(|(_, modified)| *modified);
    let mut listed: HashMap<PathBuf, Listed> = HashMap::new();
    for (sum, _) in &sums {
        let text = match fs::read_to_string(dir.join(sum)) {
            Ok(text) => text,
            Err(e) => {
                problems.push(Problem {
                    file: sum.clone(),
                    line: None,
                    reason: format!("couldn't be read: {e}"),
                });
                continue;
            }
        };
        let (entries, bad) = parse_lines(&text);
        for (line, reason) in bad {
            problems.push(Problem {
                file: sum.clone(),
                line: Some(line),
                reason,
            });
        }
        let base = sum.parent().unwrap_or(Path::new(""));
        for (line, path, expected) in entries {
            let rel = base.join(plain(&path));
            if !inside(&path) || through_a_link(dir, &rel) {
                problems.push(Problem {
                    file: sum.clone(),
                    line: Some(line),
                    reason: format!("{} points outside the checked directory", path.display()),
                });
                continue;
            }
            // Not through a link: a link's target is never read.
            let size = fs::symlink_metadata(dir.join(&rel))
                .ok()
                .filter(|m| m.is_file())
                .map_or(0, |m| m.len());
            listed.insert(
                rel.clone(),
                Listed {
                    rel,
                    expected,
                    size,
                    from: sum.clone(),
                },
            );
        }
    }
    let keys: HashSet<String> = listed.keys().map(|p| key(p)).collect();
    let mut not_checked: Vec<PathBuf> = others
        .into_iter()
        .filter(|p| !keys.contains(&key(p)))
        .collect();
    not_checked.sort();
    let mut files: Vec<Listed> = listed.into_values().collect();
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(CheckPlan {
        dir: dir.to_path_buf(),
        checksum_files: sums.into_iter().map(|(p, _)| p).collect(),
        total_bytes: files.iter().map(|f| f.size).sum(),
        files,
        not_checked,
        problems,
    })
}

/// `path` without its `./` parts: `./a` (as `find . | xargs xxhsum` writes it) is `a`.
fn plain(path: &Path) -> PathBuf {
    path.components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect()
}

/// A directory on the way to `rel` is a link: it could lead out of the checked directory.
fn through_a_link(dir: &Path, rel: &Path) -> bool {
    rel.ancestors()
        .skip(1)
        .filter(|a| !a.as_os_str().is_empty())
        .any(|a| fs::symlink_metadata(dir.join(a)).is_ok_and(|m| m.file_type().is_symlink()))
}

/// Only plain names: no `..`, no root, nothing that leaves the checked directory.
fn inside(path: &Path) -> bool {
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// A name compared in one Unicode form: what a checksum file wrote and what the disk shows
/// can differ in form on some file systems.
fn key(p: &Path) -> String {
    p.to_string_lossy().nfc().collect()
}

#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub lanes: usize,
    pub buffer_size: usize,
    pub progress_interval: Duration,
    pub keep_awake: bool,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            lanes: 4,
            buffer_size: 4 << 20,
            progress_interval: Duration::from_millis(50),
            keep_awake: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckReport {
    /// One outcome per listed file that was reached, in the order they finished.
    pub job: JobReport,
    pub not_checked: Vec<PathBuf>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckCounts {
    pub intact: u64,
    pub changed: u64,
    pub missing: u64,
    pub failed: u64,
}

impl CheckReport {
    /// Every listed file read in full and matched; nothing unreadable; not cancelled.
    pub fn is_intact(&self) -> bool {
        self.job.is_success() && self.problems.is_empty()
    }

    pub fn counts(&self) -> CheckCounts {
        let mut c = CheckCounts::default();
        for o in &self.job.outcomes {
            match &o.status {
                FileStatus::Verified => c.intact += 1,
                FileStatus::Failed(FileError::Changed { .. }) => c.changed += 1,
                FileStatus::Failed(FileError::Missing) => c.missing += 1,
                FileStatus::Failed(_) => c.failed += 1,
                _ => {}
            }
        }
        c
    }
}

pub fn run(
    plan: &CheckPlan,
    opts: &CheckOptions,
    control: &JobControl,
    on_event: &(dyn Fn(Event) + Sync),
) -> CheckReport {
    let started = Instant::now();
    let _awake = opts.keep_awake.then(crate::awake::KeepAwake::new);
    let next = AtomicUsize::new(0);
    let finished_bytes = AtomicU64::new(0);
    let files_done = AtomicU64::new(0);
    let active: Mutex<Vec<(usize, u64)>> = Mutex::new(Vec::new());
    let outcomes: Mutex<Vec<FileOutcome>> = Mutex::new(Vec::new());
    let no_bypass = AtomicBool::new(false);
    let snapshot = || {
        let active = active.lock().expect("active lock poisoned");
        Progress {
            total_files: plan.files.len() as u64,
            total_bytes: plan.total_bytes,
            files_done: files_done.load(Relaxed),
            files_skipped: 0,
            copied_bytes: 0,
            verified_bytes: finished_bytes.load(Relaxed)
                + active.iter().map(|(_, b)| b).sum::<u64>(),
            active: active
                .iter()
                .map(|&(id, bytes_done)| ActiveFile {
                    id,
                    rel: plan.files[id].rel.clone(),
                    size: plan.files[id].size,
                    phase: Phase::Verifying,
                    bytes_done,
                })
                .collect(),
            paused: control.is_paused(),
        }
    };
    std::thread::scope(|s| {
        let (stop_ticker, stop) = mpsc::channel::<()>();
        let tick = &snapshot;
        let ticker = s.spawn(move || {
            while let Err(mpsc::RecvTimeoutError::Timeout) =
                stop.recv_timeout(opts.progress_interval)
            {
                on_event(Event::Progress(tick()));
            }
        });
        let lanes: Vec<_> = (0..opts.lanes.max(1))
            .map(|_| {
                s.spawn(|| {
                    loop {
                        if control.is_stopped() {
                            break;
                        }
                        let id = next.fetch_add(1, Relaxed);
                        let Some(file) = plan.files.get(id) else {
                            break;
                        };
                        let began = Instant::now();
                        active.lock().expect("active lock poisoned").push((id, 0));
                        let set = |bytes: u64| {
                            if let Some(slot) = active
                                .lock()
                                .expect("active lock poisoned")
                                .iter_mut()
                                .find(|(i, _)| *i == id)
                            {
                                slot.1 = bytes;
                            }
                        };
                        let (status, hash, read) =
                            check_one(&plan.dir, file, opts, control, &set, &no_bypass);
                        active
                            .lock()
                            .expect("active lock poisoned")
                            .retain(|(i, _)| *i != id);
                        finished_bytes.fetch_add(read, Relaxed);
                        files_done.fetch_add(1, Relaxed);
                        let outcome = FileOutcome {
                            id,
                            rel: file.rel.clone(),
                            final_rel: file.rel.clone(),
                            size: file.size,
                            hash,
                            status,
                            in_checksum_file: true,
                            elapsed: began.elapsed(),
                        };
                        outcomes
                            .lock()
                            .expect("outcomes lock poisoned")
                            .push(outcome.clone());
                        on_event(Event::FileFinished(outcome));
                    }
                })
            })
            .collect();
        for lane in lanes {
            lane.join().expect("check lane panicked");
        }
        drop(stop_ticker);
        ticker.join().expect("progress thread panicked");
        on_event(Event::Progress(snapshot()));
    });
    let outcomes = outcomes.into_inner().expect("outcomes lock poisoned");
    CheckReport {
        job: JobReport {
            not_started: (plan.files.len() - outcomes.len()) as u64,
            outcomes,
            checksum_file: None,
            checksum_error: None,
            checksum_off: true,
            cache_bypass: Some(if no_bypass.load(Relaxed) {
                CacheBypass::Unavailable
            } else {
                CacheBypass::Active
            }),
            removed_partials: 0,
            fatal: None,
            cancelled: control.is_stopped(),
            elapsed: started.elapsed(),
            created_dirs: Vec::new(),
            unread: Vec::new(),
            durability_error: None,
            dir_errors: Vec::new(),
        },
        not_checked: plan.not_checked.clone(),
        problems: plan.problems.clone(),
    }
}

/// Reads one listed file in full from the device: (status, hash read, bytes counted).
fn check_one(
    dir: &Path,
    file: &Listed,
    opts: &CheckOptions,
    control: &JobControl,
    progress: &dyn Fn(u64),
    no_bypass: &AtomicBool,
) -> (FileStatus, Option<u64>, u64) {
    let path = dir.join(&file.rel);
    let size = match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return (FileStatus::Failed(FileError::Missing), None, file.size);
        }
        Ok(m) if !m.is_file() => {
            // Something is there, just not a file to read: never followed, never "missing".
            let e = if m.is_symlink() {
                FileError::IsLink
            } else if m.is_dir() {
                FileError::IsDirectory
            } else {
                FileError::NotAFile
            };
            return (FileStatus::Failed(e), None, file.size);
        }
        Err(e) => return (FileStatus::Failed(FileError::read_back(e)), None, file.size),
        Ok(m) => m.len(),
    };
    match hash_from_device(&path, opts.buffer_size, progress, control) {
        // Written to while it was read: what was hashed is no version of the file.
        Ok(_) if resized(&path, size) => {
            let e = io::Error::other("it changed while it was read");
            (FileStatus::Failed(FileError::read_back(e)), None, file.size)
        }
        Ok((actual, bypass)) => {
            if bypass == CacheBypass::Unavailable {
                no_bypass.store(true, Relaxed);
            }
            let status = if actual == file.expected {
                FileStatus::Verified
            } else {
                FileStatus::Failed(FileError::Changed {
                    expected: to_hex(file.expected),
                    actual: to_hex(actual),
                })
            };
            (status, Some(actual), file.size)
        }
        Err(FileError::Cancelled) => (FileStatus::Cancelled, None, 0),
        Err(e) => (FileStatus::Failed(e), None, file.size),
    }
}

/// `path` no longer has the size it had when it was opened (or is gone).
fn resized(path: &Path, size: u64) -> bool {
    fs::symlink_metadata(path).map_or(true, |m| m.len() != size)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Final review: a file that grew (or shrank) while it was read isn't what was hashed.
    #[test]
    fn a_file_that_changed_size_while_read_is_noticed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mov");
        fs::write(&path, b"12345").unwrap();
        assert!(!resized(&path, 5));
        fs::write(&path, b"123456").unwrap();
        assert!(resized(&path, 5));
        fs::remove_file(&path).unwrap();
        assert!(resized(&path, 5));
    }
}
