# Verify an Existing Copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Verify tab (and `secopy-cli --check`) that re-reads every file the checksum files in a directory list and reports each as intact, changed, missing or failed, plus the files nothing lists; mirrors keep a checksum file so their backups can be verified.

**Architecture:** A new engine module `secopy-core::check` (parse checksum files, plan, run in lanes with `verify::hash_from_device`, report through the existing `job::Event`/`FileOutcome`/`JobReport` types). The app's `Job` holds a `Work` (a copy or a check) and dispatches on it; progress, finished list, summary, report, queue and notifications are shared. Mirrors write `.secopy-checksums.xxh64` after a clean run.

**Tech Stack:** Rust 2024 (workspace, macOS only), walkdir, unicode-normalization, xxhash-rust; Tauri 2 + tauri-specta; Svelte 5, Vitest, testing-library.

**Spec:** `docs/superpowers/specs/2026-09-28-verify-copy-design.md`

## Global Constraints

- macOS only: no `cfg` code for other platforms (AGENTS.md).
- Reliability first: a check never reports "intact" for a file it didn't read in full and match, and nothing is ever written under the checked directory.
- UI copy: "directory", never "folder"; "source" and "destination", not "card".
- Tabs: Copy ⌘1 · Mirror ⌘2 · Verify ⌘3 · Queue ⌘4.
- "Not checked" alone does not make a check fail (spec: shown, not hidden).
- When several checksum files list a path, the one with the newest modification time wins.
- The mirror checksum file is `.secopy-checksums.xxh64` in the destination, written to `.secopy-checksums.partial` and renamed; written only after a mirror whose copy phase ended cleanly.
- `ui/src/lib/bindings.ts` is generated: `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app --lib ui_bindings`.
- Before every commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; for `ui/`: `npm run check`, `npx vitest run`.
- Commits: Conventional Commits, `Refs: #67`, author and committer `3268106+AbelCS@users.noreply.github.com` (export both, never `env $E`).

## Review Focus

1. A checksum file listing `../x` or `/etc/x` is a problem and is never read (Task 2: `a_path_outside_the_directory_is_a_problem`).
2. A file that grows or is truncated between plan and run is read to its current end and compared: never "intact" unless the hash matches (Task 3: `a_file_that_changed_size_is_changed`).
3. Checking never modifies the checked directory, including timestamps of directories (Task 3: `a_check_writes_nothing`).
4. A name with `\` or a newline written by `checksum_file::write` reads back as the same path (Task 1: `escaped_names_round_trip`).
5. A mirror run that failed leaves the previous `.secopy-checksums.xxh64` untouched (Task 5: `a_failed_mirror_keeps_the_previous_checksums`).

---

### Task 1: Parse checksum files; `Changed` and `Missing` errors

**Files:**
- Create: `crates/secopy-core/src/check.rs`
- Modify: `crates/secopy-core/src/lib.rs` (add `pub mod check;`), `crates/secopy-core/src/error.rs`
- Test: `crates/secopy-core/tests/check.rs`

**Interfaces:**
- Produces: `check::parse(text: &str) -> (Vec<(PathBuf, u64)>, Vec<(usize, String)>)` (entries; bad lines as 1-based line number and why). `FileError::Changed { expected: String, actual: String }` ("changed since it was copied (expected …, found …)"), `FileError::Missing` ("missing").

- [ ] **Step 1: Write the failing tests** (`tests/check.rs`)

```rust
use std::fs;
use std::path::{Path, PathBuf};

use secopy_core::check;

#[test]
fn lines_are_parsed_and_bad_ones_named() {
    let text = "0123456789abcdef  a/b.mov\nnot a line\n0123456789ABCDEF  c.mov\n+123456789abcdef  d.mov\n";
    let (entries, bad) = check::parse(text);
    assert_eq!(
        entries,
        [
            (PathBuf::from("a/b.mov"), 0x0123_4567_89ab_cdef),
            (PathBuf::from("c.mov"), 0x0123_4567_89ab_cdef),
        ]
    );
    assert_eq!(bad.iter().map(|(n, _)| *n).collect::<Vec<_>>(), [2, 4]);
}

/// Review focus 4.
#[test]
fn escaped_names_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let names = [PathBuf::from("back\\slash.mov"), PathBuf::from("new\nline.mov"), PathBuf::from("plain.mov")];
    let entries: Vec<(PathBuf, u64)> = names.iter().cloned().zip([1, 2, 3]).collect();
    let path = secopy_core::checksum_file::write(dir.path(), &entries, chrono::Local::now()).unwrap();
    let (read, bad) = check::parse(&fs::read_to_string(path).unwrap());
    assert!(bad.is_empty(), "{bad:?}");
    let mut read = read;
    read.sort();
    let mut want = entries;
    want.sort();
    assert_eq!(read, want);
}

#[test]
fn crlf_and_empty_lines_are_fine() {
    let (entries, bad) = check::parse("0000000000000001  a\r\n\r\n");
    assert_eq!(entries, [(PathBuf::from("a"), 1)]);
    assert!(bad.is_empty());
    let _ = Path::new("");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p secopy-core --test check`
Expected: FAIL to compile, "could not find `check` in `secopy_core`".

- [ ] **Step 3: Implement**

`error.rs`, in `FileError` after `SourceChanged`:

```rust
    #[error("changed since it was copied (expected {expected}, found {actual})")]
    Changed { expected: String, actual: String },
    #[error("missing")]
    Missing,
```

`lib.rs`: `pub mod check;` (alphabetical with the other modules).

`check.rs`:

```rust
//! Verify an existing copy (FR-34, plan 8): re-read every file the checksum files in a
//! directory list, and say which are intact, changed or missing, and which nothing lists.

use std::path::PathBuf;

/// Parses xxhsum/GNU lines, `<16 hex>  <path>`, with the coreutils escaping a leading `\`
/// announces (`\\`, `\n`, `\r`). Returns the entries and the bad lines (1-based, why).
pub fn parse(text: &str) -> (Vec<(PathBuf, u64)>, Vec<(usize, String)>) {
    let (lines, bad) = parse_lines(text);
    (lines.into_iter().map(|(_, path, hash)| (path, hash)).collect(), bad)
}

/// `parse`, with each entry's 1-based line number.
fn parse_lines(text: &str) -> (Vec<(usize, PathBuf, u64)>, Vec<(usize, String)>) {
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
        let path = if escaped { unescape(path) } else { Some(path.to_string()) };
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
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p secopy-core --test check`
Expected: 3 passed.

- [ ] **Step 5: Commit** `feat(core): read checksum files` (Refs: #67).

---

### Task 2: Plan a check

**Files:**
- Modify: `crates/secopy-core/src/check.rs`, `crates/secopy-core/src/system.rs` (add `.secopy-checksums.xxh64` to `NAMES`)
- Test: `crates/secopy-core/tests/check.rs`

**Interfaces:**
- Consumes: `check::parse` (Task 1), `mirror::ARCHIVE_DIR`, `system::is_system_file`.
- Produces:

```rust
pub const MIRROR_CHECKSUMS: &str = ".secopy-checksums.xxh64";
pub struct Listed { pub rel: PathBuf, pub expected: u64, pub size: u64, pub from: PathBuf }
pub struct Problem { pub file: PathBuf, pub line: Option<usize>, pub reason: String }
pub struct CheckPlan { pub dir: PathBuf, pub checksum_files: Vec<PathBuf>, pub files: Vec<Listed>,
    pub not_checked: Vec<PathBuf>, pub problems: Vec<Problem>, pub total_bytes: u64 }
pub fn plan(dir: &Path) -> std::io::Result<CheckPlan>
```

- [ ] **Step 1: Write the failing tests**

```rust
fn copy_of(files: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Day01");
    let mut entries = Vec::new();
    for (rel, data) in files {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, data).unwrap();
        entries.push((PathBuf::from(rel), secopy_core::hash::hash_bytes(data)));
    }
    secopy_core::checksum_file::write(&root, &entries, chrono::Local::now()).unwrap();
    (dir, root)
}

#[test]
fn a_plan_lists_files_and_what_nothing_lists() {
    let (_dir, root) = copy_of(&[("CLIP/A001.mov", b"a"), ("B.mov", b"bb")]);
    fs::write(root.join("extra.mov"), b"x").unwrap();
    fs::write(root.join(".DS_Store"), b"x").unwrap();
    fs::create_dir_all(root.join(".secopy-archive/2026-01-01 10.00.00")).unwrap();
    fs::write(root.join(".secopy-archive/2026-01-01 10.00.00/old.mov"), b"x").unwrap();
    fs::write(root.join("secopy_x_report.txt"), b"r").unwrap();
    let p = check::plan(&root).unwrap();
    assert_eq!(p.checksum_files.len(), 1);
    let mut listed: Vec<_> = p.files.iter().map(|f| f.rel.clone()).collect();
    listed.sort();
    assert_eq!(listed, [PathBuf::from("B.mov"), PathBuf::from("CLIP/A001.mov")]);
    assert_eq!(p.not_checked, [PathBuf::from("extra.mov")]);
    assert_eq!(p.total_bytes, 3);
    assert!(p.problems.is_empty(), "{:?}", p.problems);
}

#[test]
fn the_newest_checksum_file_wins() {
    let (_dir, root) = copy_of(&[("a.mov", b"old")]);
    fs::write(root.join("a.mov"), b"new").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(root.join("later.xxh64"), format!("{:016x}  a.mov\n", secopy_core::hash::hash_bytes(b"new"))).unwrap();
    let p = check::plan(&root).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.files[0].expected, secopy_core::hash::hash_bytes(b"new"));
    assert_eq!(p.files[0].from, PathBuf::from("later.xxh64"));
}

#[test]
fn checksum_files_in_subdirectories_are_found() {
    let dir = tempfile::tempdir().unwrap();
    let (_a, day1) = copy_of(&[("x.mov", b"x")]);
    let drive = dir.path().join("drive");
    fs::create_dir_all(&drive).unwrap();
    fs::rename(&day1, drive.join("Day01")).unwrap();
    let p = check::plan(&drive).unwrap();
    assert_eq!(p.files[0].rel, PathBuf::from("Day01/x.mov"));
}

/// Review focus 1.
#[test]
fn a_path_outside_the_directory_is_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.xxh64"), "0000000000000001  ../secret\n0000000000000002  /etc/hosts\n").unwrap();
    let p = check::plan(dir.path()).unwrap();
    assert!(p.files.is_empty());
    assert_eq!(p.problems.len(), 2, "{:?}", p.problems);
    assert!(p.problems.iter().all(|x| x.reason.contains("outside")));
}

#[test]
fn bad_lines_and_mirror_checksums() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("f.mov"), b"f").unwrap();
    fs::write(
        dir.path().join(check::MIRROR_CHECKSUMS),
        format!("{:016x}  f.mov\ngarbage\n", secopy_core::hash::hash_bytes(b"f")),
    )
    .unwrap();
    let p = check::plan(dir.path()).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.problems.len(), 1);
    assert_eq!(p.problems[0].line, Some(2));
    assert!(p.not_checked.is_empty());
}
```

- [ ] **Step 2: Run** `cargo test -p secopy-core --test check` → FAIL (no `plan`).

- [ ] **Step 3: Implement** (append to `check.rs`; add imports `std::collections::{HashMap, HashSet}`, `std::fs`, `std::io`, `std::path::{Component, Path}`, `std::time::SystemTime`, `unicode_normalization::UnicodeNormalization`, `walkdir::WalkDir`, `crate::mirror::ARCHIVE_DIR`, `crate::system::is_system_file`)

```rust
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
        .filter_entry(|e| !(e.file_type().is_dir() && e.file_name() == ARCHIVE_DIR));
    for entry in walk {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let file = e.path().and_then(|p| p.strip_prefix(dir).ok()).map(Path::to_path_buf);
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
        let rel = entry.path().strip_prefix(dir).unwrap_or(entry.path()).to_path_buf();
        let name = entry.file_name().to_string_lossy();
        if name.ends_with(".xxh64") {
            sums.push((rel, entry.metadata().ok().and_then(|m| m.modified().ok())));
        } else if !is_system_file(entry.file_name())
            && !name.ends_with("_report.txt")
            && !name.ends_with("_report.json")
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
                problems.push(Problem { file: sum.clone(), line: None, reason: format!("couldn't be read: {e}") });
                continue;
            }
        };
        let (entries, bad) = parse_lines(&text);
        for (line, reason) in bad {
            problems.push(Problem { file: sum.clone(), line: Some(line), reason });
        }
        let base = sum.parent().unwrap_or(Path::new(""));
        for (line, path, expected) in entries {
            if !inside(&path) {
                problems.push(Problem {
                    file: sum.clone(),
                    line: Some(line),
                    reason: format!("{} points outside the checked directory", path.display()),
                });
                continue;
            }
            let rel = base.join(&path);
            let size = fs::metadata(dir.join(&rel)).map_or(0, |m| m.len());
            listed.insert(rel.clone(), Listed { rel, expected, size, from: sum.clone() });
        }
    }
    let keys: HashSet<String> = listed.keys().map(|p| key(p)).collect();
    let mut not_checked: Vec<PathBuf> = others.into_iter().filter(|p| !keys.contains(&key(p))).collect();
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

/// Only plain names: no `..`, no root, nothing that leaves the checked directory.
fn inside(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// A name compared in one Unicode form: what a checksum file wrote and what the disk shows
/// can differ in form on some file systems.
fn key(p: &Path) -> String {
    p.to_string_lossy().nfc().collect()
}
```

`system.rs` `NAMES`: add `".secopy-checksums.xxh64",` under `// Secopy` (a new group before `// macOS`), so the copy engine never copies it and a mirror never plans it for removal.

- [ ] **Step 4: Run** `cargo test -p secopy-core --test check` → 8 passed; `cargo test -p secopy-core` all pass.

- [ ] **Step 5: Commit** `feat(core): plan a check of a directory's checksum files` (Refs: #67).

---

### Task 3: Run a check

**Files:**
- Modify: `crates/secopy-core/src/check.rs`
- Test: `crates/secopy-core/tests/check.rs`

**Interfaces:**
- Consumes: `CheckPlan` (Task 2), `verify::hash_from_device`, `job::{Event, FileOutcome, FileStatus, JobReport, Progress, ActiveFile, Phase}`, `JobControl`, `awake::KeepAwake`.
- Produces:

```rust
pub struct CheckOptions { pub lanes: usize, pub buffer_size: usize, pub progress_interval: Duration, pub keep_awake: bool }
pub struct CheckReport { pub job: JobReport, pub not_checked: Vec<PathBuf>, pub problems: Vec<Problem> }
impl CheckReport { pub fn is_intact(&self) -> bool; pub fn counts(&self) -> CheckCounts }
pub struct CheckCounts { pub intact: u64, pub changed: u64, pub missing: u64, pub failed: u64 }
pub fn run(plan: &CheckPlan, opts: &CheckOptions, control: &JobControl, on_event: &(dyn Fn(Event) + Sync)) -> CheckReport
```
Outcomes: `Verified` = intact; `Failed(Changed{..})`, `Failed(Missing)`, `Failed(other)`, `Cancelled`. `FileOutcome.id` is the index in `plan.files`; `hash` is the hash read.

- [ ] **Step 1: Write the failing tests**

```rust
fn quick() -> check::CheckOptions {
    check::CheckOptions { keep_awake: false, ..check::CheckOptions::default() }
}

fn check_all(root: &Path) -> check::CheckReport {
    let p = check::plan(root).unwrap();
    check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {})
}

#[test]
fn an_untouched_copy_is_intact() {
    let (_dir, root) = copy_of(&[("a.mov", b"a"), ("b/c.mov", b"cc")]);
    let r = check_all(&root);
    assert!(r.is_intact(), "{:?}", r.job.outcomes);
    assert_eq!(r.counts().intact, 2);
}

#[test]
fn changed_missing_and_unreadable_files_are_named() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, root) = copy_of(&[("a.mov", b"aaaa"), ("gone.mov", b"g"), ("locked.mov", b"l")]);
    fs::write(root.join("a.mov"), b"aaab").unwrap(); // same size, flipped byte
    fs::remove_file(root.join("gone.mov")).unwrap();
    fs::set_permissions(root.join("locked.mov"), fs::Permissions::from_mode(0o000)).unwrap();
    let r = check_all(&root);
    fs::set_permissions(root.join("locked.mov"), fs::Permissions::from_mode(0o644)).unwrap();
    let c = r.counts();
    assert_eq!((c.intact, c.changed, c.missing, c.failed), (0, 1, 1, 1));
    assert!(!r.is_intact());
}

/// Review focus 2.
#[test]
fn a_file_that_changed_size_is_changed() {
    let (_dir, root) = copy_of(&[("a.mov", b"short")]);
    let p = check::plan(&root).unwrap();
    fs::write(root.join("a.mov"), b"longer than before").unwrap();
    let r = check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {});
    assert_eq!(r.counts().changed, 1);
}

/// Review focus 3.
#[test]
fn a_check_writes_nothing() {
    let (_dir, root) = copy_of(&[("a.mov", b"a"), ("sub/b.mov", b"b")]);
    let snapshot = |root: &Path| -> Vec<(PathBuf, std::time::SystemTime)> {
        let mut all: Vec<_> = walkdir::WalkDir::new(root)
            .into_iter()
            .map(|e| e.unwrap())
            .map(|e| (e.path().to_path_buf(), e.metadata().unwrap().modified().unwrap()))
            .collect();
        all.sort();
        all
    };
    let before = snapshot(&root);
    check_all(&root);
    assert_eq!(snapshot(&root), before);
}

#[test]
fn problems_and_cancel_are_not_intact() {
    let (_dir, root) = copy_of(&[("a.mov", b"a")]);
    fs::write(root.join("bad.xxh64"), "garbage\n").unwrap();
    assert!(!check_all(&root).is_intact(), "a checksum file problem");
    let p = check::plan(&root).unwrap();
    let control = secopy_core::job::JobControl::new();
    control.cancel();
    let r = check::run(&p, &quick(), &control, &|_| {});
    assert!(r.job.cancelled && !r.is_intact());
}

#[test]
fn progress_counts_bytes_checked() {
    let (_dir, root) = copy_of(&[("a.mov", &[1u8; 10_000]), ("b.mov", &[2u8; 5_000])]);
    let p = check::plan(&root).unwrap();
    let last = std::sync::Mutex::new(None);
    check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|e| {
        if let secopy_core::job::Event::Progress(p) = e {
            *last.lock().unwrap() = Some(p);
        }
    });
    let p = last.into_inner().unwrap().expect("a final progress event");
    assert_eq!((p.files_done, p.verified_bytes, p.total_bytes), (2, 15_000, 15_000));
}
```

(`tests/check.rs` needs `walkdir` as a dev-dependency of `secopy-core`: add `walkdir.workspace = true` under `[dev-dependencies]` if it isn't reachable; it is already a normal dependency, which integration tests can use.)

- [ ] **Step 2: Run** `cargo test -p secopy-core --test check` → FAIL (no `run`).

- [ ] **Step 3: Implement** (append; imports `std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed}`, `std::sync::{Mutex, mpsc}`, `std::time::{Duration, Instant}`, `crate::control::JobControl`, `crate::error::FileError`, `crate::job::{ActiveFile, Event, FileOutcome, FileStatus, JobReport, Phase, Progress}`, `crate::verify::{CacheBypass, hash_from_device}`, `crate::hash::to_hex`)

```rust
#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub lanes: usize,
    pub buffer_size: usize,
    pub progress_interval: Duration,
    pub keep_awake: bool,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self { lanes: 4, buffer_size: 4 << 20, progress_interval: Duration::from_millis(50), keep_awake: true }
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
            verified_bytes: finished_bytes.load(Relaxed) + active.iter().map(|(_, b)| b).sum::<u64>(),
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
        let ticker = s.spawn(|| {
            while let Err(mpsc::RecvTimeoutError::Timeout) = stop.recv_timeout(opts.progress_interval) {
                on_event(Event::Progress(snapshot()));
            }
        });
        let lanes: Vec<_> = (0..opts.lanes.max(1))
            .map(|_| {
                s.spawn(|| loop {
                    if control.is_stopped() {
                        break;
                    }
                    let id = next.fetch_add(1, Relaxed);
                    let Some(file) = plan.files.get(id) else { break };
                    let began = Instant::now();
                    active.lock().expect("active lock poisoned").push((id, 0));
                    let set = |bytes: u64| {
                        if let Some(slot) = active.lock().expect("active lock poisoned").iter_mut().find(|(i, _)| *i == id) {
                            slot.1 = bytes;
                        }
                    };
                    let (status, hash, read) = check_one(&plan.dir, file, opts, control, &set, &no_bypass);
                    active.lock().expect("active lock poisoned").retain(|(i, _)| *i != id);
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
                    outcomes.lock().expect("outcomes lock poisoned").push(outcome.clone());
                    on_event(Event::FileFinished(outcome));
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
            cache_bypass: Some(if no_bypass.load(Relaxed) { CacheBypass::Unavailable } else { CacheBypass::Active }),
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
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (FileStatus::Failed(FileError::Missing), None, file.size),
        Ok(m) if !m.is_file() => return (FileStatus::Failed(FileError::Missing), None, file.size),
        Err(e) => return (FileStatus::Failed(FileError::read_back(e)), None, file.size),
        Ok(_) => {}
    }
    match hash_from_device(&path, opts.buffer_size, progress, control) {
        Ok((actual, bypass)) => {
            if bypass == CacheBypass::Unavailable {
                no_bypass.store(true, Relaxed);
            }
            let status = if actual == file.expected {
                FileStatus::Verified
            } else {
                FileStatus::Failed(FileError::Changed { expected: to_hex(file.expected), actual: to_hex(actual) })
            };
            (status, Some(actual), file.size)
        }
        Err(FileError::Cancelled) => (FileStatus::Cancelled, None, 0),
        Err(e) => (FileStatus::Failed(e), None, file.size),
    }
}
```

- [ ] **Step 4: Run** `cargo test -p secopy-core --test check` → 14 passed; `cargo test -p secopy-core` all pass; clippy clean.

- [ ] **Step 5: Commit** `feat(core): check a directory against its checksum files` (Refs: #67).

---

### Task 4: The check's report

**Files:**
- Modify: `crates/secopy-core/src/report.rs`
- Test: `crates/secopy-core/tests/check.rs`

**Interfaces:**
- Consumes: `CheckPlan`, `CheckReport`, `CheckCounts` (Tasks 2–3), `JobMeta`.
- Produces: `Report::for_check(plan: &CheckPlan, r: &CheckReport, meta: &JobMeta) -> Report`; `Report.check: Option<CheckPart>`; `pub struct CheckPart { pub checksum_files: Vec<String>, pub not_checked: Vec<String>, pub problems: Vec<Unread> }`. `mode` is `"check"`; `result` is `"all intact"`, or the non-zero parts joined by `", "` in this order: `"N changed"`, `"N missing"`, `"N couldn't be read"`, `"N checksum file problems"`, then `"cancelled"`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn the_report_says_what_was_checked() {
    let (_dir, root) = copy_of(&[("a.mov", b"aaaa"), ("b.mov", b"b")]);
    fs::write(root.join("a.mov"), b"aaab").unwrap();
    fs::write(root.join("extra.mov"), b"x").unwrap();
    let p = check::plan(&root).unwrap();
    let r = check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {});
    let meta = secopy_core::report::JobMeta {
        app_version: "test".into(),
        source: root.display().to_string(),
        verify: true,
        started: chrono::Local::now(),
        finished: chrono::Local::now(),
    };
    let report = secopy_core::report::Report::for_check(&p, &r, &meta);
    assert_eq!(report.mode, "check");
    assert_eq!(report.result, "1 changed");
    let text = report.to_text();
    assert!(text.contains("NOT CHECKED (no checksum)\n  extra.mov"), "{text}");
    assert!(text.contains("changed since it was copied"), "{text}");
    assert!(report.to_json().contains("\"not_checked\""));
}
```

- [ ] **Step 2: Run** → FAIL (no `for_check`).

- [ ] **Step 3: Implement** in `report.rs`: add `pub check: Option<CheckPart>` to `Report` (set `check: None` in `Report::new`), the `CheckPart` struct (`#[derive(Debug, Clone, PartialEq, Eq, Serialize)]`), and:

```rust
    /// A check's report (plan 8): every listed file with what was found.
    pub fn for_check(plan: &crate::check::CheckPlan, r: &crate::check::CheckReport, meta: &JobMeta) -> Report {
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
                    Some(FileStatus::Failed(e @ FileError::Changed { .. })) => ("changed", Some(e.to_string())),
                    Some(FileStatus::Failed(FileError::Missing)) => ("missing", None),
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
                    xxh64: Some(crate::hash::to_hex(f.expected)),
                    in_checksum_file: true,
                }
            })
            .collect();
        let mut parts: Vec<String> = [(c.changed, "changed"), (c.missing, "missing"), (c.failed, "couldn't be read")]
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
            app_version: meta.app_version.clone(),
            mode: "check",
            source: plan.dir.display().to_string(),
            destination: plan.dir.display().to_string(),
            started: meta.started.to_rfc3339(),
            finished: meta.finished.to_rfc3339(),
            duration_secs: r.job.elapsed.as_secs_f64(),
            result: if r.is_intact() { "all intact".into() } else { parts.join(", ") },
            counts: Counts {
                files: plan.files.len() as u64,
                verified: c.intact,
                failed: c.changed + c.missing + c.failed,
                not_started: r.job.not_started,
                ..Counts::default()
            },
            cache_bypass: r.job.cache_bypass.map(|b| b == CacheBypass::Active),
            checksum_file: None,
            checksum_error: None,
            checksum_off: true,
            removed_partials: 0,
            files,
            unread: Vec::new(),
            durability_error: None,
            mirror: None,
            dir_errors: Vec::new(),
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
```

In `to_text`, before the `PROBLEMS` section:

```rust
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
```

- [ ] **Step 4: Run** `cargo test -p secopy-core` → all pass.

- [ ] **Step 5: Commit** `feat(core): a report for a check` (Refs: #67).

---

### Task 5: Mirrors keep a checksum file

**Files:**
- Modify: `crates/secopy-core/src/checksum_file.rs` (`write_replacing`), `crates/secopy-core/src/mirror.rs` (deep-check hashes, `write_checksums`)
- Test: `crates/secopy-core/tests/mirror.rs`

**Interfaces:**
- Consumes: `check::{parse, MIRROR_CHECKSUMS}`, `mirror::{MirrorPlan, Finished}`, `job::JobReport`.
- Produces: `checksum_file::write_replacing(path: &Path, entries: &[(PathBuf, u64)]) -> io::Result<()>` (writes `path.with_extension("partial")`, syncs, renames over `path`); `MirrorPlan.same: HashMap<PathBuf, u64>` (hashes the deep check found equal); `mirror::write_checksums(plan: &MirrorPlan, report: &JobReport, finished: &Finished) -> io::Result<()>`.

- [ ] **Step 1: Write the failing tests** (`tests/mirror.rs`)

```rust
#[test]
fn a_mirror_keeps_a_checksum_file_a_check_can_use() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a"), ("new/b.mov", b"b")]);
    write(&d, &[("gone.mov", b"g"), ("a.mov", b"old a")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let (report, finished) = run(&p, None);
    let finished = finished.unwrap();
    mirror::write_checksums(&p, &report, &finished).unwrap();
    let text = fs::read_to_string(d.join(secopy_core::check::MIRROR_CHECKSUMS)).unwrap();
    let (entries, bad) = secopy_core::check::parse(&text);
    assert!(bad.is_empty());
    let mut names: Vec<_> = entries.iter().map(|(p, _)| p.display().to_string()).collect();
    names.sort();
    assert_eq!(names, ["a.mov", "new/b.mov"]);
    let plan = secopy_core::check::plan(&d).unwrap();
    let r = secopy_core::check::run(&plan, &secopy_core::check::CheckOptions { keep_awake: false, ..Default::default() }, &JobControl::new(), &|_| {});
    assert!(r.is_intact(), "{:?}", r.job.outcomes);
    assert!(!d.join(".secopy-checksums.partial").exists());
}

/// Review focus 5.
#[test]
fn a_failed_mirror_keeps_the_previous_checksums() {
    let (_dir, o, d) = pair();
    write(&d, &[(".secopy-checksums.xxh64", b"0000000000000001  a.mov\n")]);
    write(&o, &[("a.mov", b"a")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    fs::remove_file(o.join("a.mov")).unwrap(); // vanished: the copy phase fails
    let (_, finished) = run(&p, None);
    assert!(finished.is_err());
    // The caller writes checksums only after Ok(finished); the file is as it was.
    assert_eq!(fs::read(d.join(".secopy-checksums.xxh64")).unwrap(), b"0000000000000001  a.mov\n");
}

#[test]
fn the_checksum_file_is_never_planned_for_removal() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    write(&d, &[("a.mov", b"a"), (".secopy-checksums.xxh64", b"x")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    assert!(p.removals.is_empty(), "{:?}", p.removals);
}

#[test]
fn a_deep_check_records_the_hashes_it_compared() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    write(&d, &[("a.mov", b"a")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    let deep = MirrorOptions { deep_check: true, ..opts() };
    let p = mirror::plan(&o, &d, &deep).unwrap();
    assert_eq!(p.same.get(Path::new("a.mov")), Some(&secopy_core::hash::hash_bytes(b"a")));
}
```

- [ ] **Step 2: Run** `cargo test -p secopy-core --test mirror` → FAIL (no `write_checksums`, no `same`).

- [ ] **Step 3: Implement**

`checksum_file.rs`:

```rust
/// Writes `entries` to `path` whole or not at all: a temporary `<name>.partial` beside it
/// (a Secopy partial file, which no copy or mirror picks up), synced, then renamed over it.
pub fn write_replacing(path: &Path, entries: &[(PathBuf, u64)]) -> io::Result<()> {
    let mut lines: Vec<(String, String)> = entries
        .iter()
        .map(|(rel, hash)| (slash_path(rel), format_line(*hash, rel)))
        .collect();
    lines.sort();
    let mut body = String::new();
    for (_, line) in &lines {
        body.push_str(line);
        body.push('\n');
    }
    let tmp = path.with_extension("partial");
    let written = File::create(&tmp)
        .and_then(|mut f| f.write_all(body.as_bytes()).and_then(|()| crate::os::sync_durable(&f)));
    if let Err(e) = written.and_then(|()| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}
```

`mirror.rs`:
- `MirrorPlan` gains `/// Unchanged files the deep check read on both sides and found equal, with their hash.\npub same: HashMap<PathBuf, u64>,` (set in `plan_watched`).
- Replace `differs(a, b, control) -> bool` with `compare(a, b, control) -> Option<u64>` returning `Some(hash)` only when both read and equal; the deep-check arm becomes: `match compare(..) { Some(h) => { same.insert(f.entry.rel.clone(), h); } None => { f.action = Action::Overwrite; changes.push((i, Change::ContentsDiffer)); } }` (the `is_stopped()` check stays before it).
- Add:

```rust
/// The mirror's checksum file, after a clean run (plan 8): the previous one, with this run's
/// copied and updated files, the deep check's equal files, removals dropped and renames moved.
pub fn write_checksums(plan: &MirrorPlan, report: &JobReport, finished: &Finished) -> std::io::Result<()> {
    use crate::job::FileStatus;
    let path = plan.copy.dest.join(crate::check::MIRROR_CHECKSUMS);
    let mut sums: std::collections::BTreeMap<PathBuf, u64> = fs::read_to_string(&path)
        .map(|text| crate::check::parse(&text).0.into_iter().collect())
        .unwrap_or_default();
    sums.extend(plan.same.iter().map(|(p, h)| (p.clone(), *h)));
    for o in &report.outcomes {
        if matches!(o.status, FileStatus::Copied | FileStatus::Verified)
            && let Some(hash) = o.hash
        {
            sums.insert(o.final_rel.clone(), hash);
        }
    }
    for r in finished.removals.iter().filter(|r| r.result.is_ok()) {
        sums.remove(&r.rel);
    }
    for (from, to) in &finished.renamed {
        if let Some(hash) = sums.remove(from) {
            sums.insert(to.clone(), hash);
        }
    }
    let entries: Vec<(PathBuf, u64)> = sums.into_iter().collect();
    crate::checksum_file::write_replacing(&path, &entries)
}
```

- [ ] **Step 4: Run** `cargo test -p secopy-core` → all pass.

- [ ] **Step 5: Commit** `feat(core): mirrors keep a checksum file` (Refs: #67).

---

### Task 6: CLI `--check`, and mirrors write their checksum file

**Files:**
- Modify: `crates/secopy-cli/src/main.rs`
- Test: `crates/secopy-cli/tests/cli.rs`

**Interfaces:**
- Consumes: `check::{plan, run, CheckOptions}`, `mirror::write_checksums`.
- Produces: `secopy-cli --check <DIR>`: prints `intact: N`, `changed: N`, `missing: N`, `couldn't be read: N`, `not checked: N`, then each problem line `CHANGED <path>`, `MISSING <path>`, `FAILED <path>: <why>`, `PROBLEM <file>[:line]: <why>`; exit 0 when intact, 1 otherwise, 2 when the directory can't be read.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn check_says_intact_then_changed() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("A001.mov"), b"movie").unwrap();
    assert!(cli().arg(&src).arg("--to").arg(&dest).arg("--verify").status().unwrap().success());
    let out = cli().arg("--check").arg(&dest).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(String::from_utf8_lossy(&out.stdout).contains("intact: 1"));
    fs::write(dest.join("CARD/A001.mov"), b"movif").unwrap();
    let out = cli().arg("--check").arg(&dest).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("CHANGED CARD/A001.mov"));
}
```

- [ ] **Step 2: Run** `cargo test -p secopy-cli --test cli check_says` → FAIL (unknown argument).

- [ ] **Step 3: Implement**: `Args` gets `/// Check a directory against its checksum files (plan 8).\n#[arg(long, value_name = "DIR", conflicts_with_all = ["sources", "to", "mirror"])]\ncheck: Option<PathBuf>`; `sources` becomes `#[arg(required_unless_present = "check")]` and `to` becomes `Option<PathBuf>` with `required_unless_present = "check"` (unwrap it where used: `let to = args.to.clone().ok_or("--to is required")?;`). In `main`, before the mirror branch:

```rust
    if let Some(dir) = &args.check {
        return check_run(dir);
    }
```

```rust
/// `--check`: every file the directory's checksum files list, read again (plan 8).
fn check_run(dir: &Path) -> Result<ExitCode, String> {
    use secopy_core::{check, error::FileError, job::FileStatus};
    let plan = check::plan(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let r = check::run(&plan, &check::CheckOptions::default(), &JobControl::new(), &|_| {});
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
    Ok(if r.is_intact() { ExitCode::SUCCESS } else { ExitCode::from(1) })
}
```

In `mirror_run`, after `Ok(finished)` of `mirror::finish` and the removal printing: `if let Err(e) = mirror::write_checksums(&plan, &report, &finished) { println!("checksum file NOT written: {e}"); ok = false; }`.

- [ ] **Step 4: Run** `cargo test -p secopy-cli` → all pass.

- [ ] **Step 5: Commit** `feat(cli): --check a directory` (Refs: #67).

---

### Task 7: The app runs checks

**Files:**
- Modify: `crates/secopy-app/src/jobs.rs`, `crates/secopy-app/src/dto.rs`
- Test: `crates/secopy-app/src/jobs.rs` (tests module)

**Interfaces:**
- Consumes: `check::{CheckPlan, CheckReport, run, CheckOptions}`, `Report::for_check`, `mirror::write_checksums`.
- Produces:
  - `pub enum Work { Copy { ready: Ready, verify: bool, settings: JobSettings }, Check(Arc<CheckPlan>) }` and `Jobs::start_work(work: Work, sink: impl ProgressSink) -> Result<(), String>`; `Jobs::start(ready, verify, settings, sink)` calls it with `Work::Copy`.
  - DTO `CheckSummaryView { intact: u32, changed: u32, missing: u32, failed: u32, not_checked: u32, checksum_files: u32, problems: Vec<String> }`; `SummaryView.check: Option<CheckSummaryView>`; `RowStatus` gains `Intact`, `Changed`, `Missing`.
  - A check job's label is `"Verify · <dir>"`, its `copy_root` the checked directory, `verify` true, `checksum_off` true.

- [ ] **Step 1: Write the failing tests** (jobs.rs tests)

```rust
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
        jobs.start_work(Work::Check(plan), Collect::default()).unwrap();
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

    #[test]
    fn an_intact_check_is_complete() {
        let (dir, root) = check_fixture();
        let jobs = Jobs::new(dir.path().join("reports"));
        let plan = Arc::new(secopy_core::check::plan(&root).unwrap());
        jobs.start_work(Work::Check(plan), Collect::default()).unwrap();
        jobs.wait();
        assert_eq!(jobs.summary().unwrap().outcome, JobOutcome::Complete);
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
        jobs.start(job.ready(), true, JobSettings::for_mirror(&job, Local::now()), Collect::default())
            .unwrap();
        jobs.wait();
        assert_eq!(jobs.summary().unwrap().outcome, JobOutcome::Complete);
        let text = fs::read_to_string(d.join(secopy_core::check::MIRROR_CHECKSUMS)).unwrap();
        let (entries, bad) = secopy_core::check::parse(&text);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(entries.len(), 2);
    }
```

- [ ] **Step 2: Run** `cargo test -p secopy-app --lib check` → FAIL (no `Work`, no `start_work`).

- [ ] **Step 3: Implement**

`dto.rs`: `RowStatus` gains `/// A check found it as copied.\nIntact, Changed, Missing,`; `CheckSummaryView` (serde camelCase, `Type`); `SummaryView.check: Option<CheckSummaryView>` (`check: None` in every other constructor).

`jobs.rs`:
- `pub enum Work { Copy { ready: Ready, verify: bool, settings: JobSettings }, Check(Arc<CheckPlan>) }`.
- `Job` replaces `ready`, `verify`, `settings` with `work: Work`, and gets helpers:

```rust
impl Job {
    fn copy(&self) -> Option<(&Ready, bool, &JobSettings)> {
        match &self.work {
            Work::Copy { ready, verify, settings } => Some((ready, *verify, settings)),
            Work::Check(_) => None,
        }
    }
    fn check_plan(&self) -> Option<&CheckPlan> {
        match &self.work {
            Work::Check(plan) => Some(plan),
            Work::Copy { .. } => None,
        }
    }
    fn verify(&self) -> bool { self.copy().is_none_or(|(_, v, _)| v) }
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
    /// (files, bytes) the job works through.
    fn totals(&self) -> (usize, u64) {
        match &self.work {
            Work::Copy { ready, .. } => (ready.plan.files.len(), ready.plan.bytes_to_write()),
            Work::Check(plan) => (plan.files.len(), plan.total_bytes),
        }
    }
}
```

- Every `self.ready`, `self.verify`, `self.settings` use goes through these (`self.settings.mirror` → `self.copy().and_then(|(_, _, s)| s.mirror.as_ref())`, `write_checksum_file` → `self.copy().is_some_and(|(_, _, s)| s.write_checksum_file)`), `retry` returns `None` for a check.
- `Jobs::start_work(work, sink)` is today's `start` body with `Work`; `small_total` for a check counts `plan.files` under `OWN_ROW`. `Jobs::start(ready, verify, settings, sink)` becomes `self.start_work(Work::Copy { ready, verify, settings }, sink)`.
- `Done` gains `check: Option<CheckReport>`.
- `Job::run`: `match &self.work { Work::Check(plan) => self.run_check(plan, sink, reports_dir), Work::Copy { .. } => self.run_copy(sink, reports_dir) }` where `run_copy` is today's body; after `mirror::finish` returns `Some(Ok(finished))`, call `mirror::write_checksums(&m.plan, &report, finished)` and on `Err(e)` set `report.checksum_error = Some(format!("the mirror's checksum file: {e}"))`. `run_check`:

```rust
    fn run_check(&self, plan: &CheckPlan, sink: &impl ProgressSink, reports_dir: &Path) {
        let opts = CheckOptions { progress_interval: PROGRESS_INTERVAL, ..CheckOptions::default() };
        let checked = secopy_core::check::run(plan, &opts, &self.control, &|event| match event {
            Event::Progress(p) => {
                sink.send(self.progress(&p, false, None));
                *self.last.lock().expect("job lock poisoned") = p;
            }
            Event::FileFinished(o) => {
                if matches!(o.status, FileStatus::Failed(_)) {
                    self.failed.fetch_add(1, Relaxed);
                }
                if plan.files[o.id].size < OWN_ROW {
                    self.small_done.fetch_add(1, Relaxed);
                }
                self.outcomes.lock().expect("job lock poisoned").push(o);
            }
        });
        let mut done = Done {
            finished: Local::now(),
            report_file: Err(String::new()),
            next_to_error: None,
            report: checked.job.clone(),
            removals: None,
            undone: None,
            check: Some(checked),
        };
        done.report_file = self.save(&done, reports_dir);
        let last = self.last.lock().expect("job lock poisoned").clone();
        let mut view = self.progress(&last, true, None);
        view.files_done = count(done.report.outcomes.len());
        *self.done.lock().expect("job lock poisoned") = Some(done);
        sink.send(view);
    }
```

- `progress()`: totals from `self.totals()`; phase `Verifying` for a check while not finished.
- `report(done)`: `match (self.check_plan(), &done.check) { (Some(plan), Some(c)) => Report::for_check(plan, c, &meta), _ => …today… }` with `meta.source = self.label()`, `meta.verify = self.verify()`.
- `summary()`: for a check, `outcome` is `Complete` when `c.is_intact()`, `Cancelled` when cancelled, else `Failures`; `check: Some(CheckSummaryView { … counts, not_checked: count(c.not_checked.len()), checksum_files: count(plan.checksum_files.len()), problems: c.problems.iter().take(FAILURES_SHOWN).map(|p| match p.line { Some(n) => format!("{}:{n}: {}", show(&p.file), p.reason), None => format!("{}: {}", show(&p.file), p.reason) }).collect() })`; `copy_root: show(self.root())`; `checksum_off: true`.
- `row(o)` gains `check: bool`: `FileStatus::Verified if check => RowStatus::Intact`, `Failed(FileError::Changed{..}) => (RowStatus::Changed, reason)`, `Failed(FileError::Missing) => (RowStatus::Missing, None)`; callers pass `self.check_plan().is_some()`.

- [ ] **Step 4: Run** `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app --lib ui_bindings` then `cargo test -p secopy-app --lib` → all pass.

- [ ] **Step 5: Commit** `feat(app): run a check as a job` (Refs: #67).

---

### Task 8: Commands, queue and menu for Verify

**Files:**
- Modify: `crates/secopy-app/src/commands.rs`, `crates/secopy-app/src/queue.rs`, `crates/secopy-app/src/dto.rs`, `crates/secopy-app/src/lib.rs`
- Test: `crates/secopy-app/src/commands.rs`, `crates/secopy-app/src/queue.rs` (tests modules)

**Interfaces:**
- Consumes: `Work`, `Jobs::start_work` (Task 7), `check::plan`.
- Produces:
  - DTO `CheckView { directory: String, checksum_files: u32, files: u32, bytes: u64 (specta Number), not_checked: u32, problems: Vec<String> }`.
  - `AppState::check_directory(&self, path: &Path) -> Result<CheckView, String>` (plans; stores `(PathBuf, Arc<CheckPlan>)` in `checking`; errors "Choose a directory." for a file, "<path> isn't there." when missing); `AppState::start_check(&self, path: &str, sink) -> Result<(), String>` (runs the stored plan for `path`, once; "Choose the directory again." when none; "No checksum files here: there's nothing to verify." when the plan lists no file); `AppState::add_check_to_queue(&self, path: &str) -> Result<QueueView, String>`.
  - Tauri commands `check_directory(path: String)`, `start_check(path: String, on_progress: Channel<ProgressView>)`, `add_check_to_queue(path: String)`, registered in `lib.rs`.
  - `QueuedJob::Check { directory: PathBuf }`, JSON `{"kind":"check","directory":"…"}`; `Queue::add_check(&mut self, dir: PathBuf)`; `job_view` kind `"check"`, `source` the directory, `destination` `""`, `verify` true.
  - `run_one` for `Check`: plans the directory at its turn; fails with the plan error, or "No checksum files here: there's nothing to verify."; starts `Work::Check` through `start_and_wait` (which now takes a `Work` instead of `ready, verify, settings`).
  - Menu: `SHOW_VERIFY = "show-verify"` "Verify" ⌘3, Queue ⌘4; `MENU_ITEMS` gets it.
  - `failure_reason` for a check: the check headline's text ("1 changed, 2 missing" from the summary's check counts).

- [ ] **Step 1: Write the failing tests**

```rust
    // commands.rs tests
    #[test]
    fn a_directory_is_checked_then_verified() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().join("data"));
        let root = dir.path().join("Day01");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.mov"), b"a").unwrap();
        secopy_core::checksum_file::write(&root, &[(PathBuf::from("a.mov"), secopy_core::hash::hash_bytes(b"a"))], chrono::Local::now()).unwrap();
        fs::write(root.join("extra.mov"), b"x").unwrap();
        let v = state.check_directory(&root).unwrap();
        assert_eq!((v.checksum_files, v.files, v.not_checked), (1, 1, 1));
        state.start_check(&show(&root), Sink::default()).unwrap();
        state.jobs.wait();
        assert_eq!(state.jobs.summary().unwrap().outcome, JobOutcome::Complete);
        assert_eq!(state.start_check(&show(&root), Sink::default()).unwrap_err(), "Choose the directory again.");
    }

    #[test]
    fn a_queued_check_that_finds_a_change_fails() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().join("data"));
        let root = dir.path().join("Day01");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.mov"), b"A").unwrap();
        secopy_core::checksum_file::write(&root, &[(PathBuf::from("a.mov"), secopy_core::hash::hash_bytes(b"a"))], chrono::Local::now()).unwrap();
        state.add_check_to_queue(&show(&root)).unwrap();
        assert_eq!(state.queue_view().jobs[0].kind, "check");
        let summary = state.run_queue(Events::default()).unwrap();
        assert_eq!(summary.results[0].result, QueueResult::Failed);
        assert_eq!(summary.results[0].reason.as_deref(), Some("1 changed."));
    }

    #[test]
    fn a_directory_without_checksum_files_cant_be_verified() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.check_directory(dir.path()).unwrap();
        assert_eq!(
            state.start_check(&show(dir.path()), Sink::default()).unwrap_err(),
            "No checksum files here: there's nothing to verify."
        );
    }
```

```rust
    // queue.rs tests
    #[test]
    fn a_check_is_saved_and_read_back() {
        let mut q = Queue::default();
        q.add_check(PathBuf::from("/Volumes/Backup/Day01"));
        let json = serde_json::to_string(&q).unwrap();
        assert!(json.contains(r#""kind":"check""#), "{json}");
        let back: Queue = serde_json::from_str(&json).unwrap();
        assert_eq!(back.jobs[0].job, QueuedJob::Check { directory: PathBuf::from("/Volumes/Backup/Day01") });
    }
```

- [ ] **Step 2: Run** `cargo test -p secopy-app --lib` → FAIL (missing methods).

- [ ] **Step 3: Implement** as in Interfaces. The stored plan: `checking: Mutex<Option<(PathBuf, Arc<CheckPlan>)>>` on `AppState`; `start_check` takes it (`take()`), compares the path (`show(&stored) == path`), refuses when busy like `run_mirror`. `QueuedJob::Check` serializes `serde_json::json!({ "kind": "check", "directory": show(directory) })` and deserializes `Some("check")` with a string `directory`, else `Unknown`. `start_and_wait(work: Work, started: QueueEvent, sink)` calls `self.jobs.start_work(work, Forward(sink.clone()))`; the copy and mirror arms build `Work::Copy { ready, verify, settings }`. Then regenerate bindings.

- [ ] **Step 4: Run** `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app --lib ui_bindings`, then `cargo test --workspace` → all pass; clippy clean.

- [ ] **Step 5: Commit** `feat(app): check a directory, run and queue it` (Refs: #67).

---

### Task 9: The Verify tab

**Files:**
- Create: `ui/src/components/VerifyScreen.svelte`, `ui/src/components/VerifyScreen.test.ts`
- Modify: `ui/src/lib/api.ts`, `ui/src/test/fake-api.ts`, `ui/src/App.svelte`, `ui/src/App.test.ts`, `ui/src/components/JobProgress.svelte`, `ui/src/components/Summary.svelte`, `ui/src/components/FinishedList.svelte`, `ui/src/lib/headline.ts`, `ui/src/lib/headline.test.ts`, `ui/src/components/QueueScreen.svelte`, `ui/src/components/QueueSummary.svelte`, `ui/src/a11y.test.ts`, `ui/src/gallery/Gallery.svelte`, `ui/src/gallery/fake.ts`

**Interfaces:**
- Consumes: commands `checkDirectory`, `startCheck`, `addCheckToQueue`; `SummaryView.check`; `RowStatus` `intact|changed|missing`; `CheckView`.
- Produces: `api.checkDirectory(path)`, `api.startCheck(path, onProgress)`, `api.addCheckToQueue(path)`; `VerifyScreen` props `{ onStart: (path: string) => void; onQueue: (q: QueueView) => void; banner?: Snippet }`; `JobProgress` prop `check?: boolean` (title "Verifying", one bar "Checked" = `verifiedBytes / totalBytes`, Cancel question without the remove option); `headline()` for a check: "All N files intact", else the parts "N changed", "N missing", "N couldn't be read", "N checksum file problems" joined by " · ", or "Cancelled".

- [ ] **Step 1: Write the failing tests**

`VerifyScreen.test.ts`:

```ts
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { QueueView } from "../lib/bindings";
import { checkView, fakeApi } from "../test/fake-api";
import VerifyScreen from "./VerifyScreen.svelte";

function show() {
  const { api } = fakeApi();
  const calls = { start: [] as string[], queue: [] as QueueView[] };
  render(VerifyScreen, {
    props: { onStart: (p: string) => calls.start.push(p), onQueue: (q: QueueView) => calls.queue.push(q) },
    context: apiContext(api),
  });
  return { api, calls };
}

describe("VerifyScreen", () => {
  test("choosing a directory says what it will check", async () => {
    const { api, calls } = show();
    expect(screen.getByRole("button", { name: "Start verify" })).toHaveProperty("disabled", true);
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText("3 checksum files · 1,284 files listed · 212.4 GB · 12 files not listed");
    await fireEvent.click(screen.getByRole("button", { name: "Start verify" }));
    expect(calls.start).toEqual(["/Volumes/Backup/Day01"]);
  });

  test("no checksum files: says so, and Start stays off", async () => {
    const { api } = show();
    api.checkDirectory.mockResolvedValueOnce(checkView({ checksumFiles: 0, files: 0, notChecked: 5 }));
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText(/No checksum files here/);
    expect(screen.getByRole("button", { name: "Start verify" })).toHaveProperty("disabled", true);
  });

  test("problems in checksum files are shown before starting", async () => {
    const { api } = show();
    api.checkDirectory.mockResolvedValueOnce(checkView({ problems: ["a.xxh64:2: not a \"<checksum>  <path>\" line"] }));
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText(/a\.xxh64:2/);
  });

  test("Add to queue queues the directory", async () => {
    const { api, calls } = show();
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText(/checksum files/);
    await fireEvent.click(screen.getByRole("button", { name: "Add to queue" }));
    await waitFor(() => expect(calls.queue).toHaveLength(1));
    expect(api.addCheckToQueue).toHaveBeenCalledWith("/Volumes/Backup/Day01");
  });
});
```

`headline.test.ts`:

```ts
test("a check says intact, or what it found", () => {
  const check = { intact: 1284, changed: 0, missing: 0, failed: 0, notChecked: 12, checksumFiles: 3, problems: [] };
  expect(headline(summaryView({ check }))).toBe("All 1,284 files intact");
  expect(headline(summaryView({ outcome: "failures", check: { ...check, intact: 1280, changed: 3, missing: 1 } }))).toBe(
    "3 files changed · 1 missing",
  );
});
```

`App.test.ts`:

```ts
  test("the Verify tab and ⌘3 open Verify; Queue is ⌘4", async () => {
    const { state } = app();
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-verify");
    await screen.findByRole("heading", { level: 1, name: "Verify" });
    state.menu!("show-queue");
    await screen.findByRole("heading", { level: 1, name: "Queue" });
  });

  test("a check runs as Verifying and ends in its summary on the Verify tab", async () => {
    const { api, state } = app();
    api.jobSummary.mockResolvedValue(summaryView({ check: { intact: 2, changed: 0, missing: 0, failed: 0, notChecked: 0, checksumFiles: 1, problems: [] } }));
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-verify");
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(await screen.findByRole("button", { name: "Choose…" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Start verify" }));
    await screen.findByRole("heading", { level: 1, name: "Verifying" });
    state.progress!(progressView({ phase: "done" }));
    await screen.findByText("All 2 files intact");
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    await screen.findByRole("heading", { level: 1, name: "Verify" });
  });
```

`fake-api.ts`: `checkView(over)` fixture (`directory: "/Volumes/Backup/Day01", checksumFiles: 3, files: 1284, bytes: 212_400_000_000, notChecked: 12, problems: []`), spies `checkDirectory` (resolves `checkView()`), `startCheck` (stores `state.progress`), `addCheckToQueue`; `summaryView` gets `check: null`.

- [ ] **Step 2: Run** `cd ui && npx vitest run` → FAIL.

- [ ] **Step 3: Implement**

`VerifyScreen.svelte`: `AppShell` with `ScreenHeader title="Verify"`; `Section title="Directory"` with a `FormRow label="Directory"` showing the chosen path (mono) or "Choose the directory of a copy, or a whole drive: Secopy checks every file its checksum files list." and an aside `Button` "Choose…" (`api.pickDirectory("Directory to verify")`), a drop zone like Setup's for one directory. After choosing: `api.checkDirectory(path)` → the line `${plural(v.checksumFiles, "checksum file")} · ${plural(v.files, "file")} listed · ${formatBytes(v.bytes)} · ${plural(v.notChecked, "file")} not listed`; with `v.files === 0`: `Notice tone="warning"` "No checksum files here: there's nothing to verify. Secopy writes one with every copy and every mirror."; with `v.problems.length`: `Notice tone="danger"` listing them. `ActionBar`: end `Add to queue` (off without files) and primary `Start verify` (off without files) → `onStart(path)`.

`App.svelte`: `Screen` adds `"verify" | "verify-summary"`; `verifyScreens`; section `"verify"`; `TabBar` items Copy, Mirror, Verify, Queue; `go("verify")`; menu `show-verify`; `runCheck(path)` like `runMirror` (clears the Copy and Mirror summaries, `progress = { ...waiting(), totalFiles: 0, totalBytes: 0 }`, `checkRunning = true`, `api.startCheck(path, …)`, on error back to `verify`); `finish()` routes a summary with `check` to `verifySummary` / `screen = "verify-summary"`; `JobProgress check={checkRunning || queueRun?.kind === "check"}`; `Summary summary={verifySummary} onDone={() => (screen = "verify")}`; `start()`, `runQueue()`, `runMirror()` forget the verify summary.

`JobProgress.svelte`: prop `check?: boolean`; when set, header title "Verifying" (unless done/paused), `work = totalBytes`, `workDone = verifiedBytes`, one `ProgressBar label="Checked"`, and the Cancel dialog hides the remove checkbox.

`Summary.svelte`: when `summary.check`, the stats line is files, bytes read, time, speed and `{ text: "N not checked", hint: "Files no checksum file lists: nothing to compare them with." }`; a `Section title="Problems"` lists `summary.check.problems`; no checksum-file note; Retry hidden.

`FinishedList.svelte` `statusText`: `intact: "✓ Intact"`, `changed: "✗ Changed"`, `missing: "✗ Missing"`; classes `.intact` like `.verified`, `.changed, .missing` like `.failed`.

`headline.ts`: before the outcome switch,

```ts
  const c = s.check;
  if (c) {
    if (s.outcome === "cancelled") return "Cancelled";
    const parts = [
      c.changed > 0 ? plural(c.changed, "file") + " changed" : "",
      c.missing > 0 ? `${formatCount(c.missing)} missing` : "",
      c.failed > 0 ? `${formatCount(c.failed)} couldn't be read` : "",
      c.problems.length > 0 ? plural(c.problems.length, "checksum file problem") : "",
    ].filter(Boolean);
    return parts.length === 0 ? `All ${plural(c.intact, "file")} intact` : parts.join(" · ");
  }
```

Queue rows: `job.kind === "check"` → `Verify` in bold and the directory; `QueueSummary` the same.

`a11y.test.ts`: a `VerifyScreen` case. Gallery: `#verify` page and a `#verify-summary` fixture.

- [ ] **Step 4: Run** `npx vitest run && npm run check` → pass; screenshots of `#verify` and a check summary.

- [ ] **Step 5: Commit** `feat(ui): the Verify tab` (Refs: #67).

---

### Task 10: Docs, checklist and the manual check

**Files:** `docs/rfd/0001-secopy.md` (FR-34 becomes S with the design's rules; §5.7 tabs list Verify; FR-52 mirror checksum file; decision log row), `docs/testing/macos-app-checklist.md`, `README.md`, `docs/design/design-system.md` (tab list), `docs/superpowers/plans/2026-09-26-v1-roadmap.md` (plan 8 row).

- [ ] **Step 1: Checklist** — append:

```markdown
29. **Verify.** Copy a directory with Copy & Verify; Verify the destination: all intact. Change
    one byte in one file (a hex editor), delete another, add a third: Verify says 1 changed,
    1 missing, 1 not checked; the report lists them. Verify a whole drive with several copies.
30. **Verify a mirror.** Run a mirror to the NAS; Verify its destination: all intact, and
    `.secopy-checksums.xxh64` is there (⇧⌘. in Finder). Queue a verify of it with another job.
```

- [ ] **Step 2: README** — the feature list gains "**Verify:** point at a backup; every file its checksum files list is read again and compared, so silent damage shows up. Mirrors keep a checksum file too." and the keyboard line "⌘1 Copy, ⌘2 Mirror, ⌘3 Verify, ⌘4 Queue"; the CLI examples gain `cargo run --release -p secopy-cli -- --check /path/to/backup`.

- [ ] **Step 3: Full check** (Global Constraints), commit `docs: Verify in the RFD, README and checklist` (Refs: #67).

- [ ] **Step 4: Manual check (the user)** — build the app and install it (`npm run tauri build -- --bundles app`, then copy `target/release/bundle/macos/Secopy.app` into /Applications; don't delete the old one before the build succeeded), ask the user to run 29–30. Fix findings test-first.

- [ ] **Step 5: Roadmap, push, PR (ask first)** — plan 8 `Done (#67)`, push, PR with `Closes #67`, rebase-merge when the user agrees; the release PR is the user's call.
