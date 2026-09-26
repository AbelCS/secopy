# Engine Complete (M1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish `secopy-core` so the desktop app only has to call it: pre-flight, conflict
handling, a resolved plan per job, metadata, pause/resume, fatal-error detection,
keep-awake, the job report, the plan 1 carry-overs and fault-injection tests. Release 0.2.0.

**Architecture:** `scan → select → preflight → Plan::resolve → run_job`. Pre-flight
inspects the destination; the plan gives every file one action (copy, overwrite, keep
both, skip, fail); `run_job` carries it out with the plan 1 lanes. New modules: `control`,
`fsinfo`, `names`, `preflight`, `plan`, `awake`, `report` and a private `metadata`;
`job.rs` becomes `job/{mod,runner,progress}.rs`.

**Tech Stack:** Rust stable (edition 2024, `rust-version = "1.88"`), `xxhash-rust`, `walkdir`,
`chrono`, `thiserror`, `libc` (Unix), `windows-sys` 0.61 (Windows), `serde` + `serde_json`
(report), `clap`, `ctrlc`, `tempfile` (tests).

**Spec:** [2026-09-27-engine-complete-design.md](../specs/2026-09-27-engine-complete-design.md),
requirements in [RFD 0001](../../rfd/0001-secopy.md). Roadmap:
[2026-09-26-v1-roadmap.md](2026-09-26-v1-roadmap.md).

**How this plan was checked:** every task was built first in a throwaway prototype. The
code below is that prototype's code. After each task's commit, `cargo fmt --check`, clippy
for macOS, Linux and Windows, and `cargo test --workspace` passed on macOS. The macOS
RAM-disk tests passed too. Linux and Windows only get their first test run in CI.

## Global Constraints

- Work on the current branch. Conventional Commits exactly as in `CLAUDE.md`, one commit per logical
  change, with the messages given in each task. Commit only when the user has asked for this
  plan to be executed.
- Before every commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`. All must pass. Tasks marked with a cross-platform check also run
  clippy for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`.
- Versions are release-please's job: never edit `version` fields, `CHANGELOG.md` or tags.
  `Cargo.lock` changes when a task adds a dependency; commit it with that task.
- `secopy-core` has no UI dependencies and no network access. New source files start with a
  `//!` comment citing the RFD requirements they implement.
- Hashes: xxHash64, seed 0, 16 lowercase hex characters. Checksum file lines: `<hash>  <path>`,
  sorted, UTF-8, LF.
- A file only ever appears under its final name once it is complete (FR-18), and the job
  never replaces a file the plan didn't mean to replace.
- Identical means same size and modification times less than 2 s apart (`SAME_MTIME`).
- Free-space margin: `max(1 % of the bytes to write, 64 MiB)`.
- Changes to existing files are shown as unified diffs from the repository root
  (`git apply`-compatible); new files and heavily changed files are shown in full.
- TDD: write the tests shown first and watch them fail, then implement. For a new module,
  its unit tests go at the bottom of the file and the implementation above them.
- Test fixtures must be independent of the machine: tests that need a case-insensitive or
  case-sensitive drive read `plan.fs.case_sensitive` instead of assuming the OS.

## Review Focus

The five input classes most likely to hurt a real user. Each has a pinning test in the
task named:

1. **Two jobs, or a job and its own stale files, racing for one destination name.** Exactly
   one copy lands under each name, and no job ever reports bytes that aren't its own.
   Pinned in Task 6 (`a_live_writers_partial_file_is_never_touched`, and the existing
   `concurrent_jobs_into_one_destination_never_report_foreign_bytes` run 30×) and Task 10
   (the same test on plans).
2. **Re-running an interrupted offload.** Leftover partial files are replaced, and files
   already there are skipped, not re-copied, and reported as not checked. Pinned in Task 6
   (`partial_files_left_by_an_interrupted_job_are_replaced`), Task 10
   (`identical_files_are_skipped_and_left_out_of_the_checksum_file`) and Task 17
   (`a_second_run_skips_what_the_first_copied`).
3. **The card or the destination disappearing mid-job, including while paused.** The job
   stops with a clear fatal error, files finished before stay listed, and nothing hangs.
   Pinned in Task 12 (`a_destination_that_disappears_stops_the_job`,
   `a_source_that_disappears_stops_the_job`) and Task 16
   (`unplugging_the_destination_stops_the_job`).
4. **Overwrite when the new copy is bad.** The old file survives a failed verification.
   Pinned in Task 10 (`a_copy_that_fails_verification_never_replaces_the_old_file`).
5. **Copying from a Mac to exFAT or FAT32.** Names APFS allows but the card doesn't are
   flagged, files over 4 GiB are caught before copying, and the no-hard-link commit path
   works. Pinned in Task 5 (`windows_rules_apply_on_exfat`) and Task 16
   (`fat32_limits_are_found_in_preflight`,
   `exfat_copies_and_verifies_through_the_fallback_commit`).

## File map

| File | Responsibility | Tasks |
|---|---|---|
| `crates/secopy-core/src/control.rs` | `JobControl`: cancel, pause, resume, checkpoint | 1, 2 |
| `crates/secopy-core/src/job/mod.rs` | Public job types, `run_job`, checksum file, empty folders, folder mtimes, durability | 1, 3, 6, 10–13 |
| `crates/secopy-core/src/job/runner.rs` | Copy and verify lanes, commits, fatal-error checks | 1, 2, 6, 9, 10, 12 |
| `crates/secopy-core/src/job/progress.rs` | Progress types and snapshots | 1, 2, 10 |
| `crates/secopy-core/src/scan.rs` | Scan with mtimes, `DirEntry`, `Selection::subset` | 3 |
| `crates/secopy-core/src/fsinfo.rs` | Destination volume facts | 4 |
| `crates/secopy-core/src/names.rs` | Name rules, `numbered` | 5, 9 |
| `crates/secopy-core/src/copy.rs` | Partial files, locks, commit modes, metadata call | 1, 2, 6, 9, 11 |
| `crates/secopy-core/src/os.rs` | Locks, stale removal, Windows rename, cache tests | 4, 6, 15 |
| `crates/secopy-core/src/preflight.rs` | Pre-flight | 7, 8 |
| `crates/secopy-core/src/plan.rs` | Plan resolution, free space | 8, 9 |
| `crates/secopy-core/src/metadata.rs` | File times, permissions, folder mtimes | 11 |
| `crates/secopy-core/src/error.rs` | New `FileError` variants, `FatalError` | 6, 8, 12 |
| `crates/secopy-core/src/awake.rs` | Keep-awake | 13 |
| `crates/secopy-core/src/report.rs` | Text and JSON report | 14, 17 |
| `crates/secopy-core/tests/{preflight,plan,report,devices}.rs` | New integration tests | 7, 8, 14, 16 |
| `crates/secopy-cli/src/main.rs` | Plans, `--on-conflict`, `--report` | 10, 17 |

---

### Task 1: Split the job module and route cancel checks through `JobControl`

A refactor with no behaviour change, plus the CI lint fix from the plan 1 carry-overs.
`job.rs` is 650 lines and every later task adds to it, so it becomes `job/mod.rs` (public
types, `run_job`, checksum file, durability), `job/runner.rs` (lanes, `Runner`, `Queue`,
`VerifyTask`) and `job/progress.rs` (`Phase`, `ActiveFile`, `Progress`, `Slot`, the progress
snapshot). `JobControl` moves to `control.rs` and gains `checkpoint()`, which copy and verify
call at every buffer boundary instead of reading an `AtomicBool`. Task 2 makes the same
checkpoint pause the job.

**Files:**
- Create: `crates/secopy-core/src/control.rs`, `crates/secopy-core/src/job/mod.rs`, `crates/secopy-core/src/job/progress.rs`, `crates/secopy-core/src/job/runner.rs`
- Modify: `crates/secopy-core/src/copy.rs`, `crates/secopy-core/src/lib.rs`, `crates/secopy-core/src/verify.rs`, `crates/secopy-core/tests/copy.rs`, `crates/secopy-core/tests/verify.rs`, `.github/workflows/ci.yml`
- Delete: `crates/secopy-core/src/job.rs`

**Interfaces:**
- Consumes: plan 1's `job.rs`, `copy.rs`, `verify.rs`.
- Produces: `secopy_core::control::JobControl { new(), cancel(), is_stopped(), checkpoint() -> Result<(), FileError> }`
  (also re-exported as `secopy_core::job::JobControl`).
  `copy::copy_to_partial(src, final_path, cfg, progress, control: &JobControl)` and
  `verify::hash_from_device(path, buffer_size, progress, control: &JobControl)` take the
  control instead of `&AtomicBool`.

- [ ] **Step 1: Write the failing tests**

The tests pass a `JobControl` where they passed an `AtomicBool`.

Create `crates/secopy-core/src/control.rs` with its unit tests; the implementation goes above them in the implementation step:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_passes_when_running_and_fails_when_stopped() {
        let control = JobControl::new();
        assert_eq!(control.checkpoint(), Ok(()));
        control.cancel();
        assert_eq!(control.checkpoint(), Err(FileError::Cancelled));
    }
}
```

Update the tests in `crates/secopy-core/tests/copy.rs`:

```diff
diff --git a/crates/secopy-core/tests/copy.rs b/crates/secopy-core/tests/copy.rs
index 380f649..fee60de 100644
--- a/crates/secopy-core/tests/copy.rs
+++ b/crates/secopy-core/tests/copy.rs
@@ -2,9 +2,9 @@ mod common;
 
 use std::cell::Cell;
 use std::fs;
-use std::sync::atomic::AtomicBool;
 
 use common::pattern;
+use secopy_core::control::JobControl;
 use secopy_core::copy::{CopyConfig, commit, copy_to_partial, partial_path};
 use secopy_core::error::FileError;
 use secopy_core::hash::hash_bytes;
@@ -34,7 +34,7 @@ fn small_file_is_copied_hashed_and_committed() {
         &dst,
         &CopyConfig::default(),
         &|_| {},
-        &AtomicBool::new(false),
+        &JobControl::new(),
     )
     .unwrap();
     assert_eq!(pc.hash, hash_bytes(b"hello secopy"));
@@ -59,7 +59,7 @@ fn large_file_goes_through_the_pipeline_in_chunks() {
         &dst,
         &small_buffers(),
         &|b| last.set(b),
-        &AtomicBool::new(false),
+        &JobControl::new(),
     )
     .unwrap();
     assert_eq!(pc.hash, hash_bytes(&data));
@@ -77,7 +77,7 @@ fn empty_file_has_the_empty_hash() {
         &dst,
         &CopyConfig::default(),
         &|_| {},
-        &AtomicBool::new(false),
+        &JobControl::new(),
     )
     .unwrap();
     assert_eq!(pc.hash, 0xef46_db37_51d8_e999);
@@ -90,14 +90,7 @@ fn cancel_removes_the_partial_file() {
     let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
     fs::write(&src, pattern(1000)).unwrap();
 
-    let err = copy_to_partial(
-        &src,
-        &dst,
-        &small_buffers(),
-        &|_| {},
-        &AtomicBool::new(true),
-    )
-    .unwrap_err();
+    let err = copy_to_partial(&src, &dst, &small_buffers(), &|_| {}, &cancelled()).unwrap_err();
     assert_eq!(err, FileError::Cancelled);
     assert!(!partial_path(&dst).exists());
 }
@@ -111,7 +104,7 @@ fn missing_source_is_a_read_error_and_leaves_nothing() {
         &dst,
         &CopyConfig::default(),
         &|_| {},
-        &AtomicBool::new(false),
+        &JobControl::new(),
     )
     .unwrap_err();
     assert!(matches!(err, FileError::ReadSource(_)));
@@ -128,7 +121,7 @@ fn missing_destination_folder_is_a_write_error() {
         &dir.path().join("no/such/b.bin"),
         &CopyConfig::default(),
         &|_| {},
-        &AtomicBool::new(false),
+        &JobControl::new(),
     )
     .unwrap_err();
     assert!(matches!(err, FileError::WriteDest(_)));
@@ -145,7 +138,7 @@ fn sizes_around_the_buffer_size_copy_exactly() {
         );
         let data = pattern(size);
         fs::write(&src, &data).unwrap();
-        let pc = copy_to_partial(&src, &dst, &cfg, &|_| {}, &AtomicBool::new(false)).unwrap();
+        let pc = copy_to_partial(&src, &dst, &cfg, &|_| {}, &JobControl::new()).unwrap();
         assert_eq!(pc.hash, hash_bytes(&data), "size {size}");
         assert_eq!(fs::read(&pc.partial).unwrap(), data, "size {size}");
     }
@@ -163,7 +156,7 @@ fn another_writers_partial_file_is_never_truncated() {
         &dst,
         &CopyConfig::default(),
         &|_| {},
-        &AtomicBool::new(false),
+        &JobControl::new(),
     )
     .unwrap_err();
     assert_eq!(err, FileError::NameClash);
@@ -180,7 +173,7 @@ fn commit_never_replaces_an_existing_file() {
         &dst,
         &CopyConfig::default(),
         &|_| {},
-        &AtomicBool::new(false),
+        &JobControl::new(),
     )
     .unwrap();
     fs::write(&dst, b"mine").unwrap();
@@ -188,3 +181,9 @@ fn commit_never_replaces_an_existing_file() {
     assert_eq!(commit(&pc.partial, &dst), Err(FileError::AlreadyExists));
     assert_eq!(fs::read(&dst).unwrap(), b"mine");
 }
+
+fn cancelled() -> JobControl {
+    let control = JobControl::new();
+    control.cancel();
+    control
+}
```

Update the tests in `crates/secopy-core/tests/verify.rs`:

```diff
diff --git a/crates/secopy-core/tests/verify.rs b/crates/secopy-core/tests/verify.rs
index 5ac805c..e5e6e96 100644
--- a/crates/secopy-core/tests/verify.rs
+++ b/crates/secopy-core/tests/verify.rs
@@ -2,9 +2,9 @@ mod common;
 
 use std::cell::Cell;
 use std::fs;
-use std::sync::atomic::AtomicBool;
 
 use common::pattern;
+use secopy_core::control::JobControl;
 use secopy_core::error::FileError;
 use secopy_core::hash::hash_bytes;
 use secopy_core::verify::{CacheBypass, hash_from_device};
@@ -18,8 +18,7 @@ fn hashes_the_file_in_chunks_and_reports_progress() {
     fs::write(&path, &data).unwrap();
     let last = Cell::new(0);
 
-    let (hash, _) =
-        hash_from_device(&path, 4096, &|b| last.set(b), &AtomicBool::new(false)).unwrap();
+    let (hash, _) = hash_from_device(&path, 4096, &|b| last.set(b), &JobControl::new()).unwrap();
     assert_eq!(hash, hash_bytes(&data));
     assert_eq!(last.get(), data.len() as u64);
 }
@@ -30,7 +29,7 @@ fn cache_bypass_is_active_on_local_disks() {
     let dir = tempfile::tempdir().unwrap();
     let path = dir.path().join("a.bin");
     fs::write(&path, b"data").unwrap();
-    let (_, bypass) = hash_from_device(&path, 4096, &|_| {}, &AtomicBool::new(false)).unwrap();
+    let (_, bypass) = hash_from_device(&path, 4096, &|_| {}, &JobControl::new()).unwrap();
     assert_eq!(bypass, CacheBypass::Active);
 }
 
@@ -39,20 +38,15 @@ fn empty_file_hashes_to_the_empty_hash() {
     let dir = tempfile::tempdir().unwrap();
     let path = dir.path().join("a.bin");
     fs::write(&path, b"").unwrap();
-    let (hash, _) = hash_from_device(&path, 4096, &|_| {}, &AtomicBool::new(false)).unwrap();
+    let (hash, _) = hash_from_device(&path, 4096, &|_| {}, &JobControl::new()).unwrap();
     assert_eq!(hash, 0xef46_db37_51d8_e999);
 }
 
 #[test]
 fn missing_file_is_a_read_back_error() {
     let dir = tempfile::tempdir().unwrap();
-    let err = hash_from_device(
-        &dir.path().join("nope"),
-        4096,
-        &|_| {},
-        &AtomicBool::new(false),
-    )
-    .unwrap_err();
+    let err =
+        hash_from_device(&dir.path().join("nope"), 4096, &|_| {}, &JobControl::new()).unwrap_err();
     assert!(matches!(err, FileError::ReadBack(_)));
 }
 
@@ -61,6 +55,12 @@ fn cancel_stops_verification() {
     let dir = tempfile::tempdir().unwrap();
     let path = dir.path().join("a.bin");
     fs::write(&path, pattern(10_000)).unwrap();
-    let err = hash_from_device(&path, 4096, &|_| {}, &AtomicBool::new(true)).unwrap_err();
+    let err = hash_from_device(&path, 4096, &|_| {}, &cancelled()).unwrap_err();
     assert_eq!(err, FileError::Cancelled);
 }
+
+fn cancelled() -> JobControl {
+    let control = JobControl::new();
+    control.cancel();
+    control
+}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test copy --test verify`
Expected: compile errors: `unresolved import secopy_core::control`.

- [ ] **Step 3: Implement**

Move code, don't rewrite it. `job/mod.rs`, `job/runner.rs` and `job/progress.rs` below are
plan 1's `job.rs` split up, with `pub(super)` where the files use each other's items and
`self.control` passed where `&self.control.stop` was.

Add the implementation at the top of `crates/secopy-core/src/control.rs`, above the tests:

```rust
//! Cancelling a running job from another thread (FR-23).

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use crate::error::FileError;

/// Lets another thread cancel a running job.
#[derive(Debug, Default)]
pub struct JobControl {
    stop: AtomicBool,
}

impl JobControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops the job: no new files start, in-flight partial files are removed.
    pub fn cancel(&self) {
        self.stop.store(true, Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.load(Relaxed)
    }

    /// Called at every buffer boundary: fails with `Cancelled` once the job is stopped.
    pub fn checkpoint(&self) -> Result<(), FileError> {
        if self.stop.load(Relaxed) {
            Err(FileError::Cancelled)
        } else {
            Ok(())
        }
    }
}
```

Change `crates/secopy-core/src/copy.rs`:

```diff
diff --git a/crates/secopy-core/src/copy.rs b/crates/secopy-core/src/copy.rs
index 1a55435..6ce90a8 100644
--- a/crates/secopy-core/src/copy.rs
+++ b/crates/secopy-core/src/copy.rs
@@ -4,9 +4,9 @@ use std::ffi::OsString;
 use std::fs::{self, File, OpenOptions};
 use std::io::{self, Read, Write};
 use std::path::{Path, PathBuf};
-use std::sync::atomic::{AtomicBool, Ordering};
 use std::sync::mpsc;
 
+use crate::control::JobControl;
 use crate::error::FileError;
 use crate::{hash, os};
 
@@ -64,7 +64,7 @@ pub fn copy_to_partial(
     final_path: &Path,
     cfg: &CopyConfig,
     progress: &dyn Fn(u64),
-    cancel: &AtomicBool,
+    control: &JobControl,
 ) -> Result<PartialCopy, FileError> {
     let partial = partial_path(final_path);
     let reader = File::open(src).map_err(FileError::read_source)?;
@@ -77,7 +77,7 @@ pub fn copy_to_partial(
         Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(FileError::NameClash),
         Err(e) => return Err(FileError::write_dest(e)),
     };
-    let result = copy_inner(reader, &mut writer, cfg, progress, cancel);
+    let result = copy_inner(reader, &mut writer, cfg, progress, control);
     // Close before removing: Windows cannot delete an open file.
     drop(writer);
     match result {
@@ -126,16 +126,16 @@ fn copy_inner(
     writer: &mut File,
     cfg: &CopyConfig,
     progress: &dyn Fn(u64),
-    cancel: &AtomicBool,
+    control: &JobControl,
 ) -> Result<(u64, u64), FileError> {
     let len = reader.metadata().map_err(FileError::read_source)?.len();
     if cfg.uncached_write {
         os::set_nocache(writer);
     }
     let result = if len <= cfg.buffer_size as u64 {
-        copy_small(&mut reader, writer, len, progress, cancel)?
+        copy_small(&mut reader, writer, len, progress, control)?
     } else {
-        copy_pipelined(reader, writer, cfg, progress, cancel)?
+        copy_pipelined(reader, writer, cfg, progress, control)?
     };
     os::sync_file(writer).map_err(FileError::write_dest)?;
     Ok(result)
@@ -147,11 +147,9 @@ fn copy_small(
     writer: &mut File,
     len: u64,
     progress: &dyn Fn(u64),
-    cancel: &AtomicBool,
+    control: &JobControl,
 ) -> Result<(u64, u64), FileError> {
-    if cancel.load(Ordering::Relaxed) {
-        return Err(FileError::Cancelled);
-    }
+    control.checkpoint()?;
     let mut buf = Vec::with_capacity(len as usize);
     reader
         .read_to_end(&mut buf)
@@ -168,7 +166,7 @@ fn copy_pipelined(
     writer: &mut File,
     cfg: &CopyConfig,
     progress: &dyn Fn(u64),
-    cancel: &AtomicBool,
+    control: &JobControl,
 ) -> Result<(u64, u64), FileError> {
     let buffers = cfg.buffers.max(2);
     let (full_tx, full_rx) = mpsc::sync_channel::<(Vec<u8>, usize)>(buffers);
@@ -193,7 +191,7 @@ fn copy_pipelined(
             }
             Ok(hasher.digest())
         });
-        let written = write_chunks(&full_rx, &empty_tx, writer, progress, cancel);
+        let written = write_chunks(&full_rx, &empty_tx, writer, progress, control);
         // Unblock the reader if the writer stopped early.
         drop(full_rx);
         drop(empty_tx);
@@ -209,13 +207,11 @@ fn write_chunks(
     empty_tx: &mpsc::SyncSender<Vec<u8>>,
     writer: &mut File,
     progress: &dyn Fn(u64),
-    cancel: &AtomicBool,
+    control: &JobControl,
 ) -> Result<u64, FileError> {
     let mut written = 0u64;
     for (buf, n) in full_rx.iter() {
-        if cancel.load(Ordering::Relaxed) {
-            return Err(FileError::Cancelled);
-        }
+        control.checkpoint()?;
         writer.write_all(&buf[..n]).map_err(FileError::write_dest)?;
         written += n as u64;
         progress(written);
```

Delete `crates/secopy-core/src/job.rs` (its code moves to the files above).

Create `crates/secopy-core/src/job/mod.rs`:

```rust
//! Runs a copy job: copy lanes, verify lanes, progress events and the checksum file (RFD §7).

mod progress;
mod runner;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use chrono::Local;

use crate::checksum_file;
use crate::copy::CopyConfig;
use crate::error::FileError;
use crate::os;
use crate::scan::Selection;
use crate::verify::CacheBypass;

pub use crate::control::JobControl;
pub use progress::{ActiveFile, Phase, Progress};
use runner::{Queue, Runner, SetOnDrop, VERIFY_QUEUE_PER_LANE, VerifyTask};

#[derive(Debug, Clone)]
pub struct JobOptions {
    /// Copy & Verify mode (FR-25).
    pub verify: bool,
    /// Write the `.xxh64` checksum file (FR-29).
    pub write_checksum_file: bool,
    pub copy: CopyConfig,
    /// Files up to this size go to the small-file lanes (RFD §7.2).
    pub small_file_threshold: u64,
    pub small_file_lanes: usize,
    pub large_file_lanes: usize,
    pub verify_lanes: usize,
    /// How often `Event::Progress` is emitted.
    pub progress_interval: Duration,
    #[doc(hidden)]
    pub hooks: Hooks,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            verify: true,
            write_checksum_file: true,
            copy: CopyConfig::default(),
            small_file_threshold: 8 << 20,
            small_file_lanes: 8,
            large_file_lanes: 1,
            verify_lanes: 2,
            progress_interval: Duration::from_millis(50),
            hooks: Hooks::default(),
        }
    }
}

/// Fault injection for tests. Not part of the stable API.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Hooks {
    /// Called with the partial file after each copy attempt (0 = first), before verifying.
    pub after_copy: Option<fn(&Path, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Copied,
    Verified,
    Failed(FileError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOutcome {
    pub rel: PathBuf,
    pub size: u64,
    /// Source hash; `None` if the file failed before it was hashed.
    pub hash: Option<u64>,
    pub status: FileStatus,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub enum Event {
    Progress(Progress),
    FileFinished(FileOutcome),
}

#[derive(Debug, Clone)]
pub struct JobReport {
    /// In the order files finished.
    pub outcomes: Vec<FileOutcome>,
    /// Files never started because the job was cancelled or hit a fatal error.
    pub not_started: u64,
    pub checksum_file: Option<PathBuf>,
    pub checksum_error: Option<String>,
    /// `None` when not verifying.
    pub cache_bypass: Option<CacheBypass>,
    pub fatal: Option<FileError>,
    pub cancelled: bool,
    pub elapsed: Duration,
}

impl JobReport {
    pub fn failed(&self) -> impl Iterator<Item = &FileOutcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, FileStatus::Failed(_)))
    }

    pub fn is_success(&self) -> bool {
        self.fatal.is_none()
            && !self.cancelled
            && self.not_started == 0
            && self.failed().next().is_none()
    }
}

/// Copies every file in `sel` into `dest`. Blocks until done; call from a worker thread.
/// `on_event` is called from several threads.
pub fn run_job(
    sel: &Selection,
    dest: &Path,
    opts: &JobOptions,
    control: &JobControl,
    on_event: &(dyn Fn(Event) + Sync),
) -> JobReport {
    let started = Instant::now();
    for dir in &sel.dirs {
        // A failure here surfaces as a per-file write error.
        let _ = fs::create_dir_all(dest.join(dir));
    }
    let clashes = find_name_clashes(sel);
    let runner = Runner::new(sel, dest, opts, control, on_event, clashes);
    let (small, large): (Vec<usize>, Vec<usize>) =
        (0..sel.files.len()).partition(|&i| sel.files[i].size <= opts.small_file_threshold);
    let small = Queue::new(small);
    let large = Queue::new(large);
    let finished = AtomicBool::new(false);

    std::thread::scope(|s| {
        let ticker = s.spawn(|| {
            while !finished.load(Relaxed) {
                std::thread::sleep(opts.progress_interval);
                runner.emit_progress();
            }
        });
        // Stops the ticker even when joining a worker panics below; otherwise the scope
        // would wait for the ticker forever and the panic would become a hang.
        let stop_ticker = SetOnDrop(&finished);
        // Bounded, so copying can't run thousands of files ahead of verification.
        let (verify_tx, verify_rx) =
            mpsc::sync_channel::<VerifyTask>(VERIFY_QUEUE_PER_LANE * opts.verify_lanes.max(1));
        // Owned by the verify lanes only: if they all die, sends fail instead of blocking.
        let verify_rx = Arc::new(Mutex::new(verify_rx));
        let mut workers = Vec::new();
        for (queue, lanes) in [
            (&small, opts.small_file_lanes),
            (&large, opts.large_file_lanes),
        ] {
            for _ in 0..lanes.max(1) {
                let (runner, verify_tx) = (&runner, verify_tx.clone());
                workers.push(s.spawn(move || runner.copy_lane(queue, &verify_tx)));
            }
        }
        // Verify lanes end when every copy lane has dropped its sender.
        drop(verify_tx);
        if opts.verify {
            for _ in 0..opts.verify_lanes.max(1) {
                let (runner, verify_rx) = (&runner, verify_rx.clone());
                workers.push(s.spawn(move || runner.verify_lane(&verify_rx)));
            }
        }
        drop(verify_rx);
        for worker in workers {
            worker.join().expect("worker thread panicked");
        }
        drop(stop_ticker);
        ticker.join().expect("progress thread panicked");
        runner.emit_progress();
    });

    let bypass_unavailable = runner.bypass_unavailable.load(Relaxed);
    let outcomes = runner
        .outcomes
        .into_inner()
        .expect("outcomes lock poisoned");
    let fatal = runner.fatal.into_inner().expect("fatal lock poisoned");
    let (checksum_file, checksum_error) = if opts.write_checksum_file {
        write_checksum(dest, &outcomes)
    } else {
        (None, None)
    };
    make_durable(dest, &sel.dirs);
    JobReport {
        not_started: (sel.files.len() - outcomes.len()) as u64,
        outcomes,
        checksum_file,
        checksum_error,
        cache_bypass: opts.verify.then_some(if bypass_unavailable {
            CacheBypass::Unavailable
        } else {
            CacheBypass::Active
        }),
        cancelled: control.is_stopped() && fatal.is_none(),
        fatal,
        elapsed: started.elapsed(),
    }
}

/// Marks every file whose destination path repeats an earlier one, ignoring case:
/// two lanes writing the same name would corrupt each other, and case-insensitive
/// destinations (macOS, Windows, exFAT) treat `a.txt` and `A.TXT` as one file.
fn find_name_clashes(sel: &Selection) -> Vec<bool> {
    let mut seen = HashSet::new();
    sel.files
        .iter()
        .map(|f| !seen.insert(checksum_file::slash_path(&f.rel).to_lowercase()))
        .collect()
}

fn write_checksum(dest: &Path, outcomes: &[FileOutcome]) -> (Option<PathBuf>, Option<String>) {
    let entries: Vec<(PathBuf, u64)> = outcomes
        .iter()
        .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
        .filter_map(|o| o.hash.map(|h| (o.rel.clone(), h)))
        .collect();
    if entries.is_empty() {
        return (None, None);
    }
    match checksum_file::write(dest, &entries, Local::now()) {
        Ok(path) => (Some(path), None),
        Err(e) => (None, Some(e.to_string())),
    }
}

/// Makes the job durable: one fsync per directory for the renames, then one
/// drive-cache flush for the whole volume (RFD §7.4).
fn make_durable(dest: &Path, dirs: &[PathBuf]) {
    #[cfg(unix)]
    for dir in dirs
        .iter()
        .map(|d| dest.join(d))
        .chain([dest.to_path_buf()])
    {
        if let Ok(f) = fs::File::open(&dir) {
            let _ = os::sync_file(&f);
        }
    }
    #[cfg(not(unix))]
    let _ = dirs;
    let _ = os::full_barrier(dest);
}
```

Create `crates/secopy-core/src/job/progress.rs`:

```rust
//! Live per-file state and the progress snapshots sent to the UI (RFD §5.3, §7).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering::Relaxed};

use super::Event;
use super::runner::Runner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    Copying = 0,
    Verifying = 1,
}

/// A file currently being copied or verified (RFD §5.3, "Active files").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveFile {
    /// Index in `Selection::files`; stable for the whole job.
    pub id: usize,
    pub rel: PathBuf,
    pub size: u64,
    pub phase: Phase,
    /// Bytes done in the current phase.
    pub bytes_done: u64,
}

/// Raw counters; the UI derives speed and ETA from successive snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub total_files: u64,
    pub total_bytes: u64,
    pub files_done: u64,
    pub copied_bytes: u64,
    pub verified_bytes: u64,
    pub active: Vec<ActiveFile>,
}

/// Live state of one in-flight file.
pub(super) struct Slot {
    pub(super) id: usize,
    pub(super) rel: PathBuf,
    pub(super) size: u64,
    pub(super) phase: AtomicU8,
    pub(super) bytes: AtomicU64,
    /// True once this file's size is included in `Runner::copied_done`.
    pub(super) copy_counted: AtomicBool,
}

impl Slot {
    pub(super) fn set_phase(&self, phase: Phase) {
        self.bytes.store(0, Relaxed);
        self.phase.store(phase as u8, Relaxed);
    }

    pub(super) fn phase(&self) -> Phase {
        if self.phase.load(Relaxed) == Phase::Verifying as u8 {
            Phase::Verifying
        } else {
            Phase::Copying
        }
    }
}

impl Runner<'_> {
    pub(super) fn emit_progress(&self) {
        (self.on_event)(Event::Progress(self.progress()));
    }

    fn progress(&self) -> Progress {
        let active = self.active.lock().expect("active lock poisoned");
        let mut copied = self.copied_done.load(Relaxed);
        let mut verified = self.verified_done.load(Relaxed);
        let files = active
            .iter()
            .map(|s| {
                let phase = s.phase();
                let bytes_done = s.bytes.load(Relaxed);
                match phase {
                    Phase::Copying if !s.copy_counted.load(Relaxed) => copied += bytes_done,
                    Phase::Copying => {}
                    Phase::Verifying => verified += bytes_done,
                }
                ActiveFile {
                    id: s.id,
                    rel: s.rel.clone(),
                    size: s.size,
                    phase,
                    bytes_done,
                }
            })
            .collect();
        let total_bytes = self.sel.total_bytes;
        Progress {
            total_files: self.sel.files.len() as u64,
            total_bytes,
            files_done: self.files_done.load(Relaxed),
            copied_bytes: copied.min(total_bytes),
            verified_bytes: verified.min(total_bytes),
            active: files,
        }
    }
}
```

Create `crates/secopy-core/src/job/runner.rs`:

```rust
//! Copy and verify lanes (RFD §7.2, §7.3).

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use super::progress::{Phase, Slot};
use super::{Event, FileOutcome, FileStatus, JobControl, JobOptions};
use crate::copy::{self, CopyConfig, PartialCopy};
use crate::error::FileError;
use crate::hash;
use crate::scan::{ScanEntry, Selection};
use crate::verify::{self, CacheBypass};

/// Verify tasks that may wait per verify lane before copy lanes block (RFD §7.3).
pub(super) const VERIFY_QUEUE_PER_LANE: usize = 4;

/// Sets the flag when dropped, including during a panic.
pub(super) struct SetOnDrop<'a>(pub(super) &'a AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Relaxed);
    }
}

/// Lock-free work list shared by the lanes of one kind.
pub(super) struct Queue {
    items: Vec<usize>,
    next: AtomicUsize,
}

impl Queue {
    pub(super) fn new(items: Vec<usize>) -> Self {
        Self {
            items,
            next: AtomicUsize::new(0),
        }
    }

    fn pop(&self) -> Option<usize> {
        self.items.get(self.next.fetch_add(1, Relaxed)).copied()
    }
}

pub(super) struct VerifyTask {
    slot: Arc<Slot>,
    idx: usize,
    partial: PartialCopy,
    started: Instant,
}

pub(super) struct Runner<'a> {
    pub(super) sel: &'a Selection,
    pub(super) dest: &'a Path,
    pub(super) opts: &'a JobOptions,
    pub(super) copy_cfg: CopyConfig,
    pub(super) control: &'a JobControl,
    pub(super) on_event: &'a (dyn Fn(Event) + Sync),
    /// Indexed like `Selection::files`; true = skip with `FileError::NameClash`.
    pub(super) clashes: Vec<bool>,
    pub(super) copied_done: AtomicU64,
    pub(super) verified_done: AtomicU64,
    pub(super) files_done: AtomicU64,
    pub(super) active: Mutex<Vec<Arc<Slot>>>,
    pub(super) outcomes: Mutex<Vec<FileOutcome>>,
    pub(super) fatal: Mutex<Option<FileError>>,
    pub(super) bypass_unavailable: AtomicBool,
}

impl<'a> Runner<'a> {
    pub(super) fn new(
        sel: &'a Selection,
        dest: &'a Path,
        opts: &'a JobOptions,
        control: &'a JobControl,
        on_event: &'a (dyn Fn(Event) + Sync),
        clashes: Vec<bool>,
    ) -> Self {
        let copy_cfg = CopyConfig {
            uncached_write: opts.verify,
            ..opts.copy.clone()
        };
        Self {
            sel,
            dest,
            opts,
            copy_cfg,
            control,
            on_event,
            clashes,
            copied_done: AtomicU64::new(0),
            verified_done: AtomicU64::new(0),
            files_done: AtomicU64::new(0),
            active: Mutex::new(Vec::new()),
            outcomes: Mutex::new(Vec::new()),
            fatal: Mutex::new(None),
            bypass_unavailable: AtomicBool::new(false),
        }
    }

    pub(super) fn copy_lane(&self, queue: &Queue, verify_tx: &mpsc::SyncSender<VerifyTask>) {
        while !self.control.is_stopped() {
            let Some(idx) = queue.pop() else { break };
            self.copy_one(idx, verify_tx);
        }
    }

    fn copy_one(&self, idx: usize, verify_tx: &mpsc::SyncSender<VerifyTask>) {
        let entry = &self.sel.files[idx];
        let final_path = self.dest.join(&entry.rel);
        let started = Instant::now();
        let slot = self.begin(idx, entry);
        let blocked = if self.clashes[idx] {
            Some(FileError::NameClash)
        } else if fs::symlink_metadata(&final_path).is_ok() {
            Some(FileError::AlreadyExists)
        } else {
            None
        };
        if let Some(e) = blocked {
            return self.finish(&slot, entry, None, FileStatus::Failed(e), started);
        }
        let partial = match self.copy_attempt(entry, &final_path, &slot, 0) {
            Ok(partial) => partial,
            Err(e) => return self.finish(&slot, entry, None, FileStatus::Failed(e), started),
        };
        self.count_copied(&slot);
        if self.opts.verify {
            slot.set_phase(Phase::Verifying);
            let task = VerifyTask {
                slot,
                idx,
                partial,
                started,
            };
            // Blocks while the verify queue is full. Fails only if every verify lane died,
            // and then the panic propagates out of `run_job`.
            verify_tx.send(task).expect("verify lanes are running");
            return;
        }
        let hash = partial.hash;
        match copy::commit(&partial.partial, &final_path) {
            Ok(()) => self.finish(&slot, entry, Some(hash), FileStatus::Copied, started),
            Err(e) => {
                let _ = fs::remove_file(&partial.partial);
                self.finish(&slot, entry, Some(hash), FileStatus::Failed(e), started)
            }
        }
    }

    fn copy_attempt(
        &self,
        entry: &ScanEntry,
        final_path: &Path,
        slot: &Slot,
        attempt: u32,
    ) -> Result<PartialCopy, FileError> {
        slot.set_phase(Phase::Copying);
        let partial = copy::copy_to_partial(
            &entry.source,
            final_path,
            &self.copy_cfg,
            &|b| slot.bytes.store(b, Relaxed),
            self.control,
        )?;
        if let Some(hook) = self.opts.hooks.after_copy {
            hook(&partial.partial, attempt);
        }
        Ok(partial)
    }

    pub(super) fn verify_lane(&self, rx: &Mutex<mpsc::Receiver<VerifyTask>>) {
        loop {
            let task = match rx.lock().expect("verify queue lock poisoned").recv() {
                Ok(task) => task,
                Err(_) => break,
            };
            self.verify_one(task);
        }
    }

    /// Verifies the partial file, re-copying once on mismatch (FR-27), then commits it.
    /// Removes only partial files this lane created: a failed re-copy may have hit a
    /// partial file that belongs to another writer.
    fn verify_one(&self, task: VerifyTask) {
        let VerifyTask {
            slot,
            idx,
            mut partial,
            started,
        } = task;
        let entry = &self.sel.files[idx];
        let final_path = self.dest.join(&entry.rel);
        let mut attempt = 0;
        let result = loop {
            slot.set_phase(Phase::Verifying);
            let (actual, bypass) = match verify::hash_from_device(
                &partial.partial,
                self.copy_cfg.buffer_size,
                &|b| slot.bytes.store(b, Relaxed),
                self.control,
            ) {
                Ok(v) => v,
                Err(e) => {
                    let _ = fs::remove_file(&partial.partial);
                    break Err(e);
                }
            };
            if bypass == CacheBypass::Unavailable {
                self.bypass_unavailable.store(true, Relaxed);
            }
            if actual == partial.hash {
                let committed = copy::commit(&partial.partial, &final_path);
                if committed.is_err() {
                    let _ = fs::remove_file(&partial.partial);
                }
                break committed.map(|()| partial.hash);
            }
            let _ = fs::remove_file(&partial.partial);
            if attempt == 1 {
                break Err(FileError::HashMismatch {
                    expected: hash::to_hex(partial.hash),
                    actual: hash::to_hex(actual),
                });
            }
            attempt += 1;
            partial = match self.copy_attempt(entry, &final_path, &slot, attempt) {
                Ok(p) => p,
                Err(e) => break Err(e),
            };
        };
        match result {
            Ok(hash) => self.finish(&slot, entry, Some(hash), FileStatus::Verified, started),
            Err(e) => self.finish(&slot, entry, None, FileStatus::Failed(e), started),
        }
    }

    fn begin(&self, idx: usize, entry: &ScanEntry) -> Arc<Slot> {
        let slot = Arc::new(Slot {
            id: idx,
            rel: entry.rel.clone(),
            size: entry.size,
            phase: AtomicU8::new(Phase::Copying as u8),
            bytes: AtomicU64::new(0),
            copy_counted: AtomicBool::new(false),
        });
        self.active
            .lock()
            .expect("active lock poisoned")
            .push(slot.clone());
        slot
    }

    fn count_copied(&self, slot: &Slot) {
        if !slot.copy_counted.swap(true, Relaxed) {
            self.copied_done.fetch_add(slot.size, Relaxed);
        }
    }

    fn finish(
        &self,
        slot: &Arc<Slot>,
        entry: &ScanEntry,
        hash: Option<u64>,
        status: FileStatus,
        started: Instant,
    ) {
        self.active
            .lock()
            .expect("active lock poisoned")
            .retain(|s| !Arc::ptr_eq(s, slot));
        // A finished file counts as fully processed, so the bars reach 100 % even with failures.
        self.count_copied(slot);
        if self.opts.verify {
            self.verified_done.fetch_add(slot.size, Relaxed);
        }
        self.files_done.fetch_add(1, Relaxed);
        if let FileStatus::Failed(e) = &status
            && e.is_fatal()
        {
            self.fatal
                .lock()
                .expect("fatal lock poisoned")
                .get_or_insert(e.clone());
            self.control.cancel();
        }
        let outcome = FileOutcome {
            rel: entry.rel.clone(),
            size: entry.size,
            hash,
            status,
            elapsed: started.elapsed(),
        };
        self.outcomes
            .lock()
            .expect("outcomes lock poisoned")
            .push(outcome.clone());
        (self.on_event)(Event::FileFinished(outcome));
    }
}
```

Change `crates/secopy-core/src/lib.rs`:

```diff
diff --git a/crates/secopy-core/src/lib.rs b/crates/secopy-core/src/lib.rs
index 7200a52..85886e1 100644
--- a/crates/secopy-core/src/lib.rs
+++ b/crates/secopy-core/src/lib.rs
@@ -2,6 +2,7 @@
 //! UI-independent; used by the desktop app, the CLI, tests and benchmarks.
 
 pub mod checksum_file;
+pub mod control;
 pub mod copy;
 pub mod error;
 pub mod filter;
```

Change `crates/secopy-core/src/verify.rs`:

```diff
diff --git a/crates/secopy-core/src/verify.rs b/crates/secopy-core/src/verify.rs
index dc4cb08..5cba0e5 100644
--- a/crates/secopy-core/src/verify.rs
+++ b/crates/secopy-core/src/verify.rs
@@ -4,8 +4,8 @@ use std::alloc::{self, Layout};
 use std::io::{self, Read};
 use std::path::Path;
 use std::ptr::NonNull;
-use std::sync::atomic::{AtomicBool, Ordering};
 
+use crate::control::JobControl;
 use crate::error::FileError;
 use crate::{hash, os};
 
@@ -21,7 +21,7 @@ pub fn hash_from_device(
     path: &Path,
     buffer_size: usize,
     progress: &dyn Fn(u64),
-    cancel: &AtomicBool,
+    control: &JobControl,
 ) -> Result<(u64, CacheBypass), FileError> {
     let (mut file, bypassed) = os::open_uncached(path).map_err(FileError::read_back)?;
     let size = file.metadata().map_err(FileError::read_back)?.len();
@@ -31,9 +31,7 @@ pub fn hash_from_device(
     // Driven by the file size: with unbuffered I/O on Windows a read after a short
     // (unaligned) read fails, and on Unix a short read before EOF must not end the hash.
     while done < size {
-        if cancel.load(Ordering::Relaxed) {
-            return Err(FileError::Cancelled);
-        }
+        control.checkpoint()?;
         let chunk = buf.as_mut_slice();
         let n = read_once(&mut file, chunk).map_err(FileError::read_back)?;
         if n == 0 {
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit, then lint every OS in CI**

```bash
git add -A
git commit -F - <<'EOF'
refactor(core): split the job module and route cancel checks through JobControl

No behaviour change. job.rs grew to 650 lines; it becomes job/mod.rs,
job/runner.rs and job/progress.rs. JobControl moves to control.rs and
gains checkpoint(), which copy and verify call at buffer boundaries.
EOF
```


Change `.github/workflows/ci.yml`:

```diff
diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml
index 98dae8f..1b9a0b4 100644
--- a/.github/workflows/ci.yml
+++ b/.github/workflows/ci.yml
@@ -8,7 +8,12 @@ on:
 
 jobs:
   lint:
-    runs-on: ubuntu-latest
+    # Every OS, so macOS- and Windows-only `cfg` code is linted too.
+    strategy:
+      fail-fast: false
+      matrix:
+        os: [ubuntu-latest, macos-latest, windows-latest]
+    runs-on: ${{ matrix.os }}
     steps:
       - uses: actions/checkout@v7
       - uses: Swatinem/rust-cache@v2
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
ci: lint on macos and windows too

Clippy only ran on Ubuntu, so macOS- and Windows-only cfg code was
never linted (plan 1 carry-over).
EOF
```


---

### Task 2: Pause and resume (FR-22)

`JobControl::pause()` makes every `checkpoint()` block until `resume()` or `cancel()`. The
copy lanes call it before each file, copy and verify at every buffer boundary, and the
pipelined reader thread before each read, so a paused job does no I/O. Progress events
keep coming, with `paused: true`.

The reader thread now returns `FileError` and stops on cancel too. Its hash is only used
when the writer also finished, and a cancelled writer returns `Cancelled`, so a stopped
reader can never produce a short copy that looks complete.

**Files:**
- Modify: `crates/secopy-core/src/control.rs`, `crates/secopy-core/src/copy.rs`, `crates/secopy-core/src/job/progress.rs`, `crates/secopy-core/src/job/runner.rs`, `crates/secopy-core/tests/job.rs`

**Interfaces:**
- Consumes: Task 1's `JobControl`.
- Produces: `JobControl::{pause(), resume(), is_paused()}`; `Progress::paused: bool`.

- [ ] **Step 1: Write the failing tests**

Update the tests in `crates/secopy-core/tests/job.rs`:

```diff
diff --git a/crates/secopy-core/tests/job.rs b/crates/secopy-core/tests/job.rs
index 038e447..8462449 100644
--- a/crates/secopy-core/tests/job.rs
+++ b/crates/secopy-core/tests/job.rs
@@ -247,6 +247,7 @@ fn final_progress_event_is_complete() {
             copied_bytes: sel.total_bytes,
             verified_bytes: sel.total_bytes,
             active: vec![],
+            paused: false,
         }
     );
     let finished = events
@@ -536,3 +537,53 @@ fn copying_never_runs_far_ahead_of_verification() {
     let max = max_active.into_inner();
     assert!(max <= 20, "{max} files in flight");
 }
+
+#[test]
+fn a_paused_job_does_no_io_until_resumed() {
+    let dir = tempfile::tempdir().unwrap();
+    let src = dir.path().join("src");
+    let dest = dir.path().join("dest");
+    fs::create_dir_all(&dest).unwrap();
+    for i in 0..40 {
+        write_files(&src, &[(&format!("f{i:02}.bin"), &pattern(2000))]);
+    }
+    let sel = select(&src);
+    let control = JobControl::new();
+    let finished = std::sync::atomic::AtomicUsize::new(0);
+    let saw_paused = std::sync::atomic::AtomicBool::new(false);
+    let report = std::thread::scope(|s| {
+        let job = s.spawn(|| {
+            run_job(&sel, &dest, &opts(true), &control, &|e| match e {
+                Event::FileFinished(_) => {
+                    if finished.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
+                        control.pause();
+                    }
+                }
+                Event::Progress(p) if p.paused => {
+                    saw_paused.store(true, std::sync::atomic::Ordering::SeqCst)
+                }
+                _ => {}
+            })
+        });
+        // Wait until the pause has taken hold, then check that nothing moves.
+        while !saw_paused.load(std::sync::atomic::Ordering::SeqCst) {
+            std::thread::sleep(std::time::Duration::from_millis(5));
+        }
+        std::thread::sleep(std::time::Duration::from_millis(100));
+        let before = (
+            finished.load(std::sync::atomic::Ordering::SeqCst),
+            read_tree(&dest),
+        );
+        std::thread::sleep(std::time::Duration::from_millis(300));
+        let after = (
+            finished.load(std::sync::atomic::Ordering::SeqCst),
+            read_tree(&dest),
+        );
+        assert_eq!(before, after, "files changed while paused");
+        assert!(before.0 < 40, "the job finished before the pause");
+        control.resume();
+        job.join().unwrap()
+    });
+    assert!(report.is_success(), "{report:?}");
+    assert_eq!(read_tree(&dest.join("src")), read_tree(&src));
+}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test job a_paused_job`
Expected: compile error: no field `paused` on `Progress` / no method `pause`.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/control.rs`:

```diff
diff --git a/crates/secopy-core/src/control.rs b/crates/secopy-core/src/control.rs
index bb309e8..d772c80 100644
--- a/crates/secopy-core/src/control.rs
+++ b/crates/secopy-core/src/control.rs
@@ -1,13 +1,18 @@
-//! Cancelling a running job from another thread (FR-23).
+//! Pausing and cancelling a running job from another thread (FR-22, FR-23).
 
 use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
+use std::sync::{Condvar, Mutex};
 
 use crate::error::FileError;
 
-/// Lets another thread cancel a running job.
+/// Lets another thread pause, resume or cancel a running job.
 #[derive(Debug, Default)]
 pub struct JobControl {
     stop: AtomicBool,
+    paused: AtomicBool,
+    /// Guards changes to the flags above, so a waiting lane never misses a wake-up.
+    lock: Mutex<()>,
+    wake: Condvar,
 }
 
 impl JobControl {
@@ -17,26 +22,57 @@ impl JobControl {
 
     /// Stops the job: no new files start, in-flight partial files are removed.
     pub fn cancel(&self) {
-        self.stop.store(true, Relaxed);
+        self.set(&self.stop, true);
     }
 
     pub fn is_stopped(&self) -> bool {
         self.stop.load(Relaxed)
     }
 
-    /// Called at every buffer boundary: fails with `Cancelled` once the job is stopped.
+    /// Stops all I/O at the next buffer boundary until [`resume`](Self::resume).
+    pub fn pause(&self) {
+        self.set(&self.paused, true);
+    }
+
+    pub fn resume(&self) {
+        self.set(&self.paused, false);
+    }
+
+    pub fn is_paused(&self) -> bool {
+        self.paused.load(Relaxed)
+    }
+
+    /// Called at every buffer boundary and before each file: blocks while the job is
+    /// paused, and fails with `Cancelled` once it is stopped.
     pub fn checkpoint(&self) -> Result<(), FileError> {
+        if self.paused.load(Relaxed) {
+            let guard = self.lock.lock().expect("control lock poisoned");
+            let _guard = self
+                .wake
+                .wait_while(guard, |_| {
+                    self.paused.load(Relaxed) && !self.stop.load(Relaxed)
+                })
+                .expect("control lock poisoned");
+        }
         if self.stop.load(Relaxed) {
             Err(FileError::Cancelled)
         } else {
             Ok(())
         }
     }
+
+    fn set(&self, flag: &AtomicBool, value: bool) {
+        let _guard = self.lock.lock().expect("control lock poisoned");
+        flag.store(value, Relaxed);
+        self.wake.notify_all();
+    }
 }
 
 #[cfg(test)]
 mod tests {
     use super::*;
+    use std::sync::mpsc;
+    use std::time::Duration;
 
     #[test]
     fn checkpoint_passes_when_running_and_fails_when_stopped() {
@@ -45,4 +81,35 @@ mod tests {
         control.cancel();
         assert_eq!(control.checkpoint(), Err(FileError::Cancelled));
     }
+
+    #[test]
+    fn checkpoint_blocks_while_paused_until_resumed() {
+        let control = JobControl::new();
+        control.pause();
+        let (tx, rx) = mpsc::channel();
+        std::thread::scope(|s| {
+            s.spawn(|| tx.send(control.checkpoint()).unwrap());
+            assert!(
+                rx.recv_timeout(Duration::from_millis(100)).is_err(),
+                "must block"
+            );
+            control.resume();
+            assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Ok(()));
+        });
+    }
+
+    #[test]
+    fn cancel_wakes_a_paused_checkpoint() {
+        let control = JobControl::new();
+        control.pause();
+        let (tx, rx) = mpsc::channel();
+        std::thread::scope(|s| {
+            s.spawn(|| tx.send(control.checkpoint()).unwrap());
+            control.cancel();
+            assert_eq!(
+                rx.recv_timeout(Duration::from_secs(5)).unwrap(),
+                Err(FileError::Cancelled)
+            );
+        });
+    }
 }
```

Change `crates/secopy-core/src/copy.rs`:

```diff
diff --git a/crates/secopy-core/src/copy.rs b/crates/secopy-core/src/copy.rs
index 6ce90a8..646af62 100644
--- a/crates/secopy-core/src/copy.rs
+++ b/crates/secopy-core/src/copy.rs
@@ -177,10 +177,12 @@ fn copy_pipelined(
             .expect("channel has room for every buffer");
     }
     std::thread::scope(|s| {
-        let reader_thread = s.spawn(move || -> io::Result<u64> {
+        let reader_thread = s.spawn(move || -> Result<u64, FileError> {
             let mut hasher = hash::hasher();
             while let Ok(mut buf) = empty_rx.recv() {
-                let n = read_full(&mut reader, &mut buf)?;
+                // Pausing stops the reader too, not only the writer (FR-22).
+                control.checkpoint()?;
+                let n = read_full(&mut reader, &mut buf).map_err(FileError::read_source)?;
                 if n == 0 {
                     break;
                 }
@@ -197,7 +199,7 @@ fn copy_pipelined(
         drop(empty_tx);
         let read = reader_thread.join().expect("reader thread panicked");
         let written = written?;
-        let hash = read.map_err(FileError::read_source)?;
+        let hash = read?;
         Ok((hash, written))
     })
 }
```

Change `crates/secopy-core/src/job/progress.rs`:

```diff
diff --git a/crates/secopy-core/src/job/progress.rs b/crates/secopy-core/src/job/progress.rs
index 6ba8b12..5e2b3f2 100644
--- a/crates/secopy-core/src/job/progress.rs
+++ b/crates/secopy-core/src/job/progress.rs
@@ -34,6 +34,8 @@ pub struct Progress {
     pub copied_bytes: u64,
     pub verified_bytes: u64,
     pub active: Vec<ActiveFile>,
+    /// The job is paused (FR-22); snapshots keep coming so the UI stays live.
+    pub paused: bool,
 }
 
 /// Live state of one in-flight file.
@@ -98,6 +100,7 @@ impl Runner<'_> {
             copied_bytes: copied.min(total_bytes),
             verified_bytes: verified.min(total_bytes),
             active: files,
+            paused: self.control.is_paused(),
         }
     }
 }
```

Change `crates/secopy-core/src/job/runner.rs`:

```diff
diff --git a/crates/secopy-core/src/job/runner.rs b/crates/secopy-core/src/job/runner.rs
index 85d7ddd..1b65fbc 100644
--- a/crates/secopy-core/src/job/runner.rs
+++ b/crates/secopy-core/src/job/runner.rs
@@ -102,7 +102,8 @@ impl<'a> Runner<'a> {
     }
 
     pub(super) fn copy_lane(&self, queue: &Queue, verify_tx: &mpsc::SyncSender<VerifyTask>) {
-        while !self.control.is_stopped() {
+        // Blocks while paused, so no new file starts during a pause (FR-22).
+        while self.control.checkpoint().is_ok() {
             let Some(idx) = queue.pop() else { break };
             self.copy_one(idx, verify_tx);
         }
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): pause and resume a running job

JobControl::pause() blocks every lane at its next buffer boundary until
resume() or cancel(). Progress events keep coming with paused: true.
EOF
```


---

### Task 3: Scan: mtimes, `std::path::absolute`, `Selection::subset`

Pre-flight needs the source mtime of every file (identical check, FR-17) and metadata
needs the mtime of every folder (FR-19). `ScanEntry` gains `mtime`, and `Selection::dirs`
becomes `Vec<DirEntry { rel, mtime }>`. `scan` resolves the source with
`std::path::absolute` instead of `fs::canonicalize` (plan 1 carry-over): a symlinked source
folder keeps its own name, and volumes that can't be canonicalized (some Windows RAM disks,
VeraCrypt, network drives) work. A path ending in `..` has no name of its own, so only then
is it canonicalized. `Selection::subset(&ids)` supports "Retry failed".

**Files:**
- Modify: `crates/secopy-core/src/job/mod.rs`, `crates/secopy-core/src/scan.rs`, `crates/secopy-core/tests/scan.rs`

**Interfaces:**
- Consumes: plan 1's `scan`.
- Produces: `ScanEntry::mtime: Option<SystemTime>`; `scan::DirEntry { rel: PathBuf, mtime: Option<SystemTime> }`;
  `Selection::dirs: Vec<DirEntry>`; `Selection::subset(&self, ids: &[usize]) -> Selection`;
  `Scan::dir_mtimes: HashMap<PathBuf, SystemTime>`.

- [ ] **Step 1: Write the failing tests**

Update the tests in `crates/secopy-core/tests/scan.rs`:

```diff
diff --git a/crates/secopy-core/tests/scan.rs b/crates/secopy-core/tests/scan.rs
index caaf816..a1a7add 100644
--- a/crates/secopy-core/tests/scan.rs
+++ b/crates/secopy-core/tests/scan.rs
@@ -3,6 +3,7 @@ mod common;
 use std::collections::BTreeSet;
 use std::fs;
 use std::path::{Path, PathBuf};
+use std::time::{Duration, SystemTime};
 
 use common::write_files;
 use secopy_core::filter::ExtensionFilter;
@@ -142,7 +143,7 @@ fn selecting_without_filter_keeps_every_file_and_empty_dirs() {
     assert_eq!(sel.files.len(), 4);
     assert_eq!(sel.total_bytes, 14);
     assert_eq!(
-        rels(sel.dirs),
+        rels(sel.dirs.iter().map(|d| d.rel.clone())),
         rels(["CARD", "CARD/clips", "CARD/clips/empty"].map(PathBuf::from))
     );
 }
@@ -158,7 +159,7 @@ fn selecting_with_filter_drops_other_files_and_empty_dirs() {
     );
     assert_eq!(sel.total_bytes, 10);
     assert_eq!(
-        rels(sel.dirs),
+        rels(sel.dirs.iter().map(|d| d.rel.clone())),
         rels(["CARD", "CARD/clips"].map(PathBuf::from))
     );
 }
@@ -197,3 +198,83 @@ fn missing_file_source_is_reported_as_a_problem() {
     assert!(scan.files.is_empty());
     assert_eq!(scan.problems.len(), 1);
 }
+
+#[test]
+fn file_and_folder_mtimes_are_recorded() {
+    let (_dir, card) = card();
+    let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
+    let file = fs::File::options()
+        .write(true)
+        .open(card.join("clips/B002.mov"))
+        .unwrap();
+    file.set_times(fs::FileTimes::new().set_modified(t))
+        .unwrap();
+    drop(file);
+    let clips = fs::File::open(card.join("clips")).unwrap();
+    // Directories can't be opened for writing on every OS; skip where setting fails.
+    let dir_set = clips
+        .set_times(fs::FileTimes::new().set_modified(t))
+        .is_ok();
+
+    let sel = scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::All);
+    let b002 = sel
+        .files
+        .iter()
+        .find(|f| f.rel.ends_with("B002.mov"))
+        .unwrap();
+    assert_eq!(b002.mtime, Some(t));
+    let clips = sel
+        .dirs
+        .iter()
+        .find(|d| d.rel == Path::new("CARD").join("clips"))
+        .unwrap();
+    assert!(clips.mtime.is_some());
+    if dir_set {
+        assert_eq!(clips.mtime, Some(t));
+    }
+    let root = sel
+        .dirs
+        .iter()
+        .find(|d| d.rel == Path::new("CARD"))
+        .unwrap();
+    assert!(
+        root.mtime.is_some(),
+        "the copied folder itself has an mtime"
+    );
+}
+
+#[cfg(unix)]
+#[test]
+fn a_symlinked_source_folder_keeps_its_own_name() {
+    let (dir, card) = card();
+    let link = dir.path().join("TODAY");
+    std::os::unix::fs::symlink(&card, &link).unwrap();
+    let scan = scan_card(&link, DirMode::FolderItself, false);
+    assert_eq!(scan.root_dir, Some(PathBuf::from("TODAY")));
+    assert!(scan.files.iter().all(|f| f.rel.starts_with("TODAY")));
+}
+
+#[test]
+fn a_path_ending_in_dotdot_uses_the_resolved_folder_name() {
+    let (_dir, card) = card();
+    let scan = scan_card(&card.join("clips/.."), DirMode::FolderItself, false);
+    assert_eq!(scan.root_dir, Some(PathBuf::from("CARD")));
+}
+
+#[test]
+fn subset_keeps_only_the_given_files_and_their_folders() {
+    let (_dir, card) = card();
+    let sel = scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::All);
+    let b002 = sel
+        .files
+        .iter()
+        .position(|f| f.rel.ends_with("B002.mov"))
+        .unwrap();
+    let sub = sel.subset(&[b002]);
+    assert_eq!(sub.files.len(), 1);
+    assert_eq!(sub.total_bytes, 5);
+    assert_eq!(
+        rels(sub.dirs.iter().map(|d| d.rel.clone())),
+        rels(["CARD", "CARD/clips"].map(PathBuf::from))
+    );
+}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test scan`
Expected: compile errors: no field `mtime`/`rel` on the dir entries, no method `subset`.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/job/mod.rs`:

```diff
diff --git a/crates/secopy-core/src/job/mod.rs b/crates/secopy-core/src/job/mod.rs
index 2706001..2d6e6da 100644
--- a/crates/secopy-core/src/job/mod.rs
+++ b/crates/secopy-core/src/job/mod.rs
@@ -16,7 +16,7 @@ use crate::checksum_file;
 use crate::copy::CopyConfig;
 use crate::error::FileError;
 use crate::os;
-use crate::scan::Selection;
+use crate::scan::{DirEntry, Selection};
 use crate::verify::CacheBypass;
 
 pub use crate::control::JobControl;
@@ -130,7 +130,7 @@ pub fn run_job(
     let started = Instant::now();
     for dir in &sel.dirs {
         // A failure here surfaces as a per-file write error.
-        let _ = fs::create_dir_all(dest.join(dir));
+        let _ = fs::create_dir_all(dest.join(&dir.rel));
     }
     let clashes = find_name_clashes(sel);
     let runner = Runner::new(sel, dest, opts, control, on_event, clashes);
@@ -238,11 +238,11 @@ fn write_checksum(dest: &Path, outcomes: &[FileOutcome]) -> (Option<PathBuf>, Op
 
 /// Makes the job durable: one fsync per directory for the renames, then one
 /// drive-cache flush for the whole volume (RFD §7.4).
-fn make_durable(dest: &Path, dirs: &[PathBuf]) {
+fn make_durable(dest: &Path, dirs: &[DirEntry]) {
     #[cfg(unix)]
     for dir in dirs
         .iter()
-        .map(|d| dest.join(d))
+        .map(|d| dest.join(&d.rel))
         .chain([dest.to_path_buf()])
     {
         if let Ok(f) = fs::File::open(&dir) {
```

Change `crates/secopy-core/src/scan.rs`:

```diff
diff --git a/crates/secopy-core/src/scan.rs b/crates/secopy-core/src/scan.rs
index b200bcf..f4b1cbc 100644
--- a/crates/secopy-core/src/scan.rs
+++ b/crates/secopy-core/src/scan.rs
@@ -1,9 +1,10 @@
 //! Walks the source and builds the list of files to copy (FR-1..FR-14).
 
-use std::collections::{BTreeMap, BTreeSet, HashSet};
+use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
 use std::fs;
 use std::io;
 use std::path::{Path, PathBuf};
+use std::time::SystemTime;
 
 use walkdir::WalkDir;
 
@@ -25,6 +26,16 @@ pub struct ScanEntry {
     pub rel: PathBuf,
     pub size: u64,
     pub ext: ExtKey,
+    /// Source modification time, kept on the copy (FR-19) and used to spot identical files (FR-17).
+    pub mtime: Option<SystemTime>,
+}
+
+/// A directory the job creates, relative to the destination.
+#[derive(Debug, Clone, PartialEq, Eq)]
+pub struct DirEntry {
+    pub rel: PathBuf,
+    /// Source directory's modification time, restored after its contents are written (FR-19).
+    pub mtime: Option<SystemTime>,
 }
 
 #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
@@ -54,6 +65,8 @@ pub struct Scan {
     /// Symlinks are never followed or copied (FR-24).
     pub skipped_symlinks: Vec<PathBuf>,
     pub problems: Vec<ScanProblem>,
+    /// Modification time of every source directory, by path relative to the destination.
+    pub dir_mtimes: HashMap<PathBuf, SystemTime>,
 }
 
 /// The files and directories a job will create.
@@ -61,10 +74,29 @@ pub struct Scan {
 pub struct Selection {
     pub files: Vec<ScanEntry>,
     /// Directories to create, relative to the destination, parents first.
-    pub dirs: Vec<PathBuf>,
+    pub dirs: Vec<DirEntry>,
     pub total_bytes: u64,
 }
 
+impl Selection {
+    /// Only the files at `ids` (indexes into `files`), e.g. to retry failed files.
+    /// Keeps the directories that contain them; empty directories are dropped.
+    pub fn subset(&self, ids: &[usize]) -> Selection {
+        let files: Vec<ScanEntry> = ids.iter().map(|&i| self.files[i].clone()).collect();
+        let dirs = self
+            .dirs
+            .iter()
+            .filter(|d| files.iter().any(|f| f.rel.starts_with(&d.rel)))
+            .cloned()
+            .collect();
+        Selection {
+            total_bytes: files.iter().map(|f| f.size).sum(),
+            files,
+            dirs,
+        }
+    }
+}
+
 pub fn scan(source: &Source, opts: &ScanOptions) -> io::Result<Scan> {
     match source {
         Source::Directory { path, mode } => scan_dir(path, *mode, opts),
@@ -91,14 +123,22 @@ impl Scan {
             }
         }
         let total_bytes = files.iter().map(|f| f.size).sum();
+        let dirs = dirs
+            .into_iter()
+            .map(|rel| DirEntry {
+                mtime: self.dir_mtimes.get(&rel).copied(),
+                rel,
+            })
+            .collect();
         Selection {
             files,
-            dirs: dirs.into_iter().collect(),
+            dirs,
             total_bytes,
         }
     }
 
-    fn push_file(&mut self, source: PathBuf, rel: PathBuf, size: u64) {
+    fn push_file(&mut self, source: PathBuf, rel: PathBuf, meta: &fs::Metadata) {
+        let (size, mtime) = (meta.len(), meta.modified().ok());
         let ext = ext_key(&rel);
         let stat = self.ext_stats.entry(ext.clone()).or_default();
         stat.files += 1;
@@ -108,6 +148,7 @@ impl Scan {
             rel,
             size,
             ext,
+            mtime,
         });
     }
 
@@ -128,7 +169,7 @@ fn scan_files(paths: &[PathBuf]) -> Scan {
             Ok(meta) if meta.file_type().is_symlink() => scan.skipped_symlinks.push(path.clone()),
             Ok(meta) if meta.is_file() => {
                 let name = path.file_name().expect("a regular file has a file name");
-                scan.push_file(path.clone(), PathBuf::from(name), meta.len());
+                scan.push_file(path.clone(), PathBuf::from(name), &meta);
             }
             Ok(_) => scan.problem(path, "not a regular file"),
             Err(e) => scan.problem(path, e),
@@ -138,20 +179,21 @@ fn scan_files(paths: &[PathBuf]) -> Scan {
 }
 
 fn scan_dir(root: &Path, mode: DirMode, opts: &ScanOptions) -> io::Result<Scan> {
-    let root = fs::canonicalize(root)?;
+    // Not `canonicalize`: a symlinked source folder keeps its own name, and some Windows
+    // volumes (RAM disks, VeraCrypt, network drives) can't be canonicalized.
+    let root = std::path::absolute(root)?;
+    let root_meta = fs::metadata(&root)?;
     let prefix = match mode {
         DirMode::ContentsOnly => PathBuf::new(),
-        DirMode::FolderItself => PathBuf::from(root.file_name().ok_or_else(|| {
-            io::Error::new(
-                io::ErrorKind::InvalidInput,
-                "a drive root has no folder name; copy only its contents instead",
-            )
-        })?),
+        DirMode::FolderItself => PathBuf::from(folder_name(&root)?),
     };
     let mut scan = Scan {
         root_dir: (!prefix.as_os_str().is_empty()).then(|| prefix.clone()),
         ..Scan::default()
     };
+    if let Ok(mtime) = root_meta.modified() {
+        scan.dir_mtimes.insert(prefix.clone(), mtime);
+    }
     let mut dirs = BTreeSet::new();
     let mut non_empty = HashSet::new();
     let mut skipped_hidden = 0u64;
@@ -196,13 +238,16 @@ fn scan_dir(root: &Path, mode: DirMode, opts: &ScanOptions) -> io::Result<Scan>
         if file_type.is_symlink() {
             scan.skipped_symlinks.push(entry.into_path());
         } else if file_type.is_dir() {
+            if let Some(mtime) = entry.metadata().ok().and_then(|m| m.modified().ok()) {
+                scan.dir_mtimes.insert(rel.clone(), mtime);
+            }
             dirs.insert(rel);
             non_empty.insert(parent);
         } else if file_type.is_file() {
             match entry.metadata() {
                 Ok(meta) => {
                     non_empty.insert(parent);
-                    scan.push_file(entry.into_path(), rel, meta.len());
+                    scan.push_file(entry.into_path(), rel, &meta);
                 }
                 Err(e) => scan.problem(entry.path(), e),
             }
@@ -217,3 +262,21 @@ fn scan_dir(root: &Path, mode: DirMode, opts: &ScanOptions) -> io::Result<Scan>
         .collect();
     Ok(scan)
 }
+
+/// The folder's own name for "copy the folder itself" (FR-4a). A path ending in `..`
+/// has no name of its own, so it is resolved first.
+fn folder_name(root: &Path) -> io::Result<std::ffi::OsString> {
+    let resolved;
+    let named = if root.file_name().is_some() {
+        root
+    } else {
+        resolved = fs::canonicalize(root)?;
+        &resolved
+    };
+    named.file_name().map(ToOwned::to_owned).ok_or_else(|| {
+        io::Error::new(
+            io::ErrorKind::InvalidInput,
+            "a drive root has no folder name; copy only its contents instead",
+        )
+    })
+}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): record mtimes and resolve source paths without canonicalize

Pre-flight needs file mtimes to spot identical files (FR-17) and folder
mtimes are restored after a copy (FR-19). std::path::absolute replaces
canonicalize, so a symlinked source keeps its own name and volumes that
can't be canonicalized work (plan 1 carry-over). Selection::subset
supports "Retry failed".
EOF
```


---

### Task 4: Destination file system facts (`fsinfo`)

Everything pre-flight needs to know about the destination volume: its kind (from
`statfs` on macOS and Linux, `GetVolumeInformationByHandleW` on Windows), case
sensitivity (probed by creating a hidden file and looking it up in upper case, which also
proves the folder is writable), free space, the FAT 4 GiB limit, the name-length rule and a
device id that later detects an unplugged drive. Adds `windows-sys` for Windows.

**Files:**
- Create: `crates/secopy-core/src/fsinfo.rs`
- Modify: `Cargo.toml`, `crates/secopy-core/Cargo.toml`, `crates/secopy-core/src/lib.rs`, `crates/secopy-core/src/os.rs`
- Generated: `Cargo.lock` (Cargo updates it; commit it)

**Interfaces:**
- Consumes: `os::c_path` (made `pub(crate)`).
- Produces: `fsinfo::{FsInfo { kind, case_sensitive, free_bytes, max_file_size, name_limit, device }, FsKind, NameLimit::{Bytes, Utf16Units}, FAT_MAX_FILE_SIZE, fs_info(dir) -> io::Result<FsInfo>, device_id(path) -> io::Result<u64>}`;
  `FsKind::has_windows_names()`.

- [ ] **Step 1: Write the failing tests**

Create `crates/secopy-core/src/fsinfo.rs` with its unit tests; the implementation goes above them in the implementation step:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_temp_volume() {
        let dir = tempfile::tempdir().unwrap();
        let info = fs_info(dir.path()).unwrap();
        assert!(info.free_bytes > 0);
        assert_eq!(info.device, device_id(dir.path()).unwrap());
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            0,
            "the probe file is removed"
        );
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn default_system_volumes_are_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!fs_info(dir.path()).unwrap().case_sensitive);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_temp_volumes_are_case_sensitive() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fs_info(dir.path()).unwrap().case_sensitive);
    }

    #[test]
    fn a_missing_directory_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fs_info(&dir.path().join("nope")).is_err());
    }

    #[test]
    fn fat_has_windows_names_and_a_4_gib_limit() {
        let fat = FsKind::from_name("msdos");
        assert_eq!(fat, FsKind::Fat);
        assert!(fat.has_windows_names());
        assert_eq!(fat.max_file_size(), Some(FAT_MAX_FILE_SIZE));
        assert_eq!(FsKind::from_magic(0x4D44), FsKind::Fat);
        assert!(!FsKind::from_name("apfs").has_windows_names());
        assert_eq!(FsKind::from_name("apfs").max_file_size(), None);
        assert_eq!(
            FsKind::from_magic(0xEF53).name_limit(),
            NameLimit::Bytes(255)
        );
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --lib fsinfo`
Expected: compile errors: `fs_info`, `FsKind` and friends don't exist yet.

- [ ] **Step 3: Implement**

Change `Cargo.toml`:

```diff
diff --git a/Cargo.toml b/Cargo.toml
index 0178d7b..c59a180 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -17,4 +17,5 @@ libc = "0.2"
 tempfile = "3.27.0"
 thiserror = "2.0.21"
 walkdir = "2.5.0"
+windows-sys = { version = "0.61.2", features = ["Win32_Foundation", "Win32_Storage_FileSystem"] }
 xxhash-rust = { version = "0.8.18", features = ["xxh64"] }
```

Change `crates/secopy-core/Cargo.toml`:

```diff
diff --git a/crates/secopy-core/Cargo.toml b/crates/secopy-core/Cargo.toml
index b3048d0..febbd7c 100644
--- a/crates/secopy-core/Cargo.toml
+++ b/crates/secopy-core/Cargo.toml
@@ -15,5 +15,8 @@ xxhash-rust.workspace = true
 [target.'cfg(unix)'.dependencies]
 libc.workspace = true
 
+[target.'cfg(windows)'.dependencies]
+windows-sys.workspace = true
+
 [dev-dependencies]
 tempfile.workspace = true
```

Add the implementation at the top of `crates/secopy-core/src/fsinfo.rs`, above the tests:

```rust
//! Facts about the destination volume, for pre-flight and fatal-error detection
//! (FR-16, FR-17a, FR-21).

use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// File system families that change what pre-flight checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsKind {
    Apfs,
    HfsPlus,
    Ext4,
    Btrfs,
    Xfs,
    Ntfs,
    ReFs,
    ExFat,
    /// FAT12/16/32.
    Fat,
    Smb,
    Nfs,
    /// Anything else, with the name or magic number the OS reported.
    Other(String),
}

/// Longest file name the file system accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameLimit {
    Bytes(usize),
    Utf16Units(usize),
}

/// Largest file FAT can hold.
pub const FAT_MAX_FILE_SIZE: u64 = (4 << 30) - 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsInfo {
    pub kind: FsKind,
    /// Probed by creating a file, not guessed from the kind.
    pub case_sensitive: bool,
    pub free_bytes: u64,
    pub max_file_size: Option<u64>,
    pub name_limit: NameLimit,
    /// Identifies the volume; a change means it was unplugged or remounted (FR-21).
    pub device: u64,
}

impl FsKind {
    /// Windows file-name rules apply on these, whatever OS writes to them (FR-16).
    pub fn has_windows_names(&self) -> bool {
        matches!(
            self,
            FsKind::Ntfs | FsKind::ReFs | FsKind::ExFat | FsKind::Fat
        )
    }

    fn name_limit(&self) -> NameLimit {
        match self {
            FsKind::Apfs
            | FsKind::HfsPlus
            | FsKind::Ntfs
            | FsKind::ReFs
            | FsKind::ExFat
            | FsKind::Fat => NameLimit::Utf16Units(255),
            _ => NameLimit::Bytes(255),
        }
    }

    fn max_file_size(&self) -> Option<u64> {
        (*self == FsKind::Fat).then_some(FAT_MAX_FILE_SIZE)
    }

    /// Names as macOS (`f_fstypename`) and Windows (`GetVolumeInformation`) report them.
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "apfs" => FsKind::Apfs,
            "hfs" => FsKind::HfsPlus,
            "ntfs" => FsKind::Ntfs,
            "refs" => FsKind::ReFs,
            "exfat" => FsKind::ExFat,
            "msdos" | "fat" | "fat32" => FsKind::Fat,
            "smbfs" => FsKind::Smb,
            "nfs" => FsKind::Nfs,
            other => FsKind::Other(other.to_string()),
        }
    }

    /// Linux `statfs.f_type` magic numbers.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn from_magic(magic: u64) -> Self {
        match magic {
            0xEF53 => FsKind::Ext4,
            0x9123_683E => FsKind::Btrfs,
            0x5846_5342 => FsKind::Xfs,
            0x4D44 => FsKind::Fat,
            0x2011_BAB0 => FsKind::ExFat,
            0x7366_746E | 0x5346_544E => FsKind::Ntfs,
            0xFE53_4D42 | 0xFF53_4D42 | 0x517B => FsKind::Smb,
            0x6969 => FsKind::Nfs,
            other => FsKind::Other(format!("0x{other:x}")),
        }
    }
}

/// Reads the facts for the volume holding `dir`. Probing case sensitivity creates and
/// removes a hidden file, so an error here also means `dir` is not writable.
pub fn fs_info(dir: &Path) -> io::Result<FsInfo> {
    let kind = sys::fs_kind(dir)?;
    Ok(FsInfo {
        case_sensitive: probe_case_sensitive(dir)?,
        free_bytes: sys::free_bytes(dir)?,
        max_file_size: kind.max_file_size(),
        name_limit: kind.name_limit(),
        device: device_id(dir)?,
        kind,
    })
}

/// Identifies the volume holding `path` (`st_dev` on Unix, the volume serial on Windows).
pub fn device_id(path: &Path) -> io::Result<u64> {
    sys::device_id(path)
}

fn probe_case_sensitive(dir: &Path) -> io::Result<bool> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let name = format!(".secopy-probe-{}-{nanos}", std::process::id());
    let path = dir.join(&name);
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let insensitive = fs::symlink_metadata(dir.join(name.to_uppercase())).is_ok();
    fs::remove_file(&path)?;
    Ok(!insensitive)
}

#[cfg(unix)]
mod sys {
    use std::io;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    use super::FsKind;
    use crate::os::c_path;

    #[cfg(target_os = "macos")]
    pub fn fs_kind(dir: &Path) -> io::Result<FsKind> {
        let c = c_path(dir)?;
        // SAFETY: an all-zero `statfs` is a valid value to be overwritten.
        let mut st: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is NUL-terminated and `st` is a valid out-pointer.
        if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the kernel NUL-terminates `f_fstypename`.
        let name = unsafe { std::ffi::CStr::from_ptr(st.f_fstypename.as_ptr()) };
        Ok(FsKind::from_name(&name.to_string_lossy()))
    }

    #[cfg(target_os = "linux")]
    pub fn fs_kind(dir: &Path) -> io::Result<FsKind> {
        let c = c_path(dir)?;
        // SAFETY: an all-zero `statfs` is a valid value to be overwritten.
        let mut st: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is NUL-terminated and `st` is a valid out-pointer.
        if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
            return Err(io::Error::last_os_error());
        }
        #[allow(clippy::unnecessary_cast)] // the field's type differs between targets
        Ok(FsKind::from_magic(st.f_type as u64))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn fs_kind(_dir: &Path) -> io::Result<FsKind> {
        Ok(FsKind::Other("unknown".into()))
    }

    pub fn free_bytes(dir: &Path) -> io::Result<u64> {
        let c = c_path(dir)?;
        // SAFETY: an all-zero `statvfs` is a valid value to be overwritten.
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is NUL-terminated and `st` is a valid out-pointer.
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return Err(io::Error::last_os_error());
        }
        #[allow(clippy::unnecessary_cast)] // the fields' types differ between targets
        Ok(st.f_bavail as u64 * st.f_frsize as u64)
    }

    pub fn device_id(path: &Path) -> io::Result<u64> {
        Ok(std::fs::metadata(path)?.dev())
    }
}

#[cfg(windows)]
mod sys {
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, GetDiskFreeSpaceExW, GetVolumeInformationByHandleW,
    };

    use super::FsKind;

    /// Directories can only be opened with backup semantics.
    fn open_dir(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
    }

    /// (file system name, volume serial number)
    fn volume(path: &Path) -> io::Result<(String, u32)> {
        let file = open_dir(path)?;
        let mut serial = 0u32;
        let mut name = [0u16; 64];
        // SAFETY: valid handle; the out-pointers live for the call; null for unused ones.
        let ok = unsafe {
            GetVolumeInformationByHandleW(
                file.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                &mut serial,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                name.as_mut_ptr(),
                name.len() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        Ok((String::from_utf16_lossy(&name[..len]), serial))
    }

    pub fn fs_kind(dir: &Path) -> io::Result<FsKind> {
        Ok(FsKind::from_name(&volume(dir)?.0))
    }

    pub fn free_bytes(dir: &Path) -> io::Result<u64> {
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
        let mut available = 0u64;
        // SAFETY: `wide` is NUL-terminated; unused out-pointers are null.
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut available,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(available)
    }

    pub fn device_id(path: &Path) -> io::Result<u64> {
        Ok(u64::from(volume(path)?.1))
    }
}
```

Change `crates/secopy-core/src/lib.rs`:

```diff
diff --git a/crates/secopy-core/src/lib.rs b/crates/secopy-core/src/lib.rs
index 85886e1..24a66e0 100644
--- a/crates/secopy-core/src/lib.rs
+++ b/crates/secopy-core/src/lib.rs
@@ -6,6 +6,7 @@ pub mod control;
 pub mod copy;
 pub mod error;
 pub mod filter;
+pub mod fsinfo;
 pub mod hash;
 pub mod hidden;
 pub mod job;
```

Change `crates/secopy-core/src/os.rs`:

```diff
diff --git a/crates/secopy-core/src/os.rs b/crates/secopy-core/src/os.rs
index 73db1d1..7370241 100644
--- a/crates/secopy-core/src/os.rs
+++ b/crates/secopy-core/src/os.rs
@@ -130,7 +130,7 @@ pub fn rename_noreplace(_from: &Path, _to: &Path) -> io::Result<()> {
 }
 
 #[cfg(any(target_os = "macos", target_os = "linux"))]
-fn c_path(path: &Path) -> io::Result<std::ffi::CString> {
+pub(crate) fn c_path(path: &Path) -> io::Result<std::ffi::CString> {
     use std::os::unix::ffi::OsStrExt;
     std::ffi::CString::new(path.as_os_str().as_bytes())
         .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

Also lint the other platforms' `cfg` code (install the targets once with
`rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`):
`cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` and
`cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`.
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): read destination file system facts

Kind, case sensitivity (probed), free space, the FAT 4 GiB limit, the
name-length rule and a device id, for pre-flight (FR-16, FR-17a) and
fatal-error detection (FR-21). Adds windows-sys for the Windows calls.
EOF
```


---

### Task 5: Names the destination can't store (`names`)

Windows naming rules (`< > : " / \ | ? *`, control characters, reserved names such as
`CON` or `LPT1.txt`, a trailing dot or space) apply on NTFS, ReFS, exFAT and FAT, and to
every destination when running on Windows. Every file system has a length limit, counted
in bytes or UTF-16 units. macOS itself accepts `:` and `?` on FAT/exFAT by mapping them to
private-use characters, but those names break when the card is read on Windows, so the
rules apply there too (RFD FR-16).

**Files:**
- Create: `crates/secopy-core/src/names.rs`
- Modify: `crates/secopy-core/src/lib.rs`

**Interfaces:**
- Consumes: Task 4's `FsInfo`, `NameLimit`.
- Produces: `names::{NameProblem::{InvalidChar(char), Reserved, TrailingDotOrSpace, TooLong { limit }}, check_name(&OsStr, &FsInfo), check_path(&Path, &FsInfo)}`
  with readable `Display` messages.

- [ ] **Step 1: Write the failing tests**

Create `crates/secopy-core/src/names.rs` with its unit tests; the implementation goes above them in the implementation step:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsinfo::FsKind;

    fn fs(kind: FsKind, name_limit: NameLimit) -> FsInfo {
        FsInfo {
            kind,
            case_sensitive: false,
            free_bytes: 0,
            max_file_size: None,
            name_limit,
            device: 0,
        }
    }

    fn exfat() -> FsInfo {
        fs(FsKind::ExFat, NameLimit::Utf16Units(255))
    }

    fn ext4() -> FsInfo {
        fs(FsKind::Ext4, NameLimit::Bytes(255))
    }

    fn check(name: &str, fs: &FsInfo) -> Result<(), NameProblem> {
        check_name(OsStr::new(name), fs)
    }

    #[test]
    fn windows_rules_apply_on_exfat() {
        let cases = [
            ("A001.mov", Ok(())),
            ("a:b.mov", Err(NameProblem::InvalidChar(':'))),
            ("what?.txt", Err(NameProblem::InvalidChar('?'))),
            ("tab\there", Err(NameProblem::InvalidChar('\t'))),
            ("CON", Err(NameProblem::Reserved)),
            ("con.txt", Err(NameProblem::Reserved)),
            ("LPT9.tar.gz", Err(NameProblem::Reserved)),
            ("CONSOLE.txt", Ok(())),
            ("COM10", Ok(())),
            ("trail.", Err(NameProblem::TrailingDotOrSpace)),
            ("trail ", Err(NameProblem::TrailingDotOrSpace)),
            ("日本語 クリップ.mov", Ok(())),
        ];
        for (name, expected) in cases {
            assert_eq!(check(name, &exfat()), expected, "{name}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_file_systems_only_check_length() {
        for name in ["a:b.mov", "CON", "trail.", "what?"] {
            assert_eq!(check(name, &ext4()), Ok(()), "{name}");
        }
    }

    #[test]
    fn length_is_counted_per_file_system() {
        let limit = NameLimit::Utf16Units(255);
        assert_eq!(check(&"a".repeat(255), &exfat()), Ok(()));
        assert_eq!(
            check(&"a".repeat(256), &exfat()),
            Err(NameProblem::TooLong { limit })
        );
        // 255 characters but 765 bytes: fine in UTF-16 units, too long in bytes.
        assert_eq!(check(&"漢".repeat(255), &exfat()), Ok(()));
        assert_eq!(
            check(&"漢".repeat(255), &ext4()),
            Err(NameProblem::TooLong {
                limit: NameLimit::Bytes(255)
            })
        );
    }

    #[test]
    fn every_path_component_is_checked() {
        let rel = Path::new("CARD").join("a:b").join("A001.mov");
        assert_eq!(
            check_path(&rel, &exfat()),
            Err(NameProblem::InvalidChar(':'))
        );
        assert_eq!(check_path(Path::new("CARD/A001.mov"), &exfat()), Ok(()));
    }

    #[test]
    fn messages_are_readable() {
        assert_eq!(
            NameProblem::InvalidChar(':').to_string(),
            "the name contains \":\", which this drive doesn't allow"
        );
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --lib names`
Expected: compile errors: `check_name`, `check_path` and `NameProblem` don't exist yet.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/lib.rs`:

```diff
diff --git a/crates/secopy-core/src/lib.rs b/crates/secopy-core/src/lib.rs
index 24a66e0..1b049f6 100644
--- a/crates/secopy-core/src/lib.rs
+++ b/crates/secopy-core/src/lib.rs
@@ -10,6 +10,7 @@ pub mod fsinfo;
 pub mod hash;
 pub mod hidden;
 pub mod job;
+pub mod names;
 mod os;
 pub mod scan;
 pub mod source;
```

Add the implementation at the top of `crates/secopy-core/src/names.rs`, above the tests:

```rust
//! File names the destination file system can't store (FR-16).

use std::ffi::OsStr;
use std::fmt;
use std::path::{Component, Path};

use crate::fsinfo::{FsInfo, NameLimit};

/// Why a name can't be created on the destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameProblem {
    InvalidChar(char),
    /// `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, with any extension.
    Reserved,
    TrailingDotOrSpace,
    TooLong {
        limit: NameLimit,
    },
}

impl fmt::Display for NameProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameProblem::InvalidChar(c) if c.is_control() => {
                write!(f, "the name contains a control character")
            }
            NameProblem::InvalidChar(c) => {
                write!(
                    f,
                    "the name contains \"{c}\", which this drive doesn't allow"
                )
            }
            NameProblem::Reserved => write!(f, "the name is reserved on Windows"),
            NameProblem::TrailingDotOrSpace => {
                write!(
                    f,
                    "the name ends with a dot or a space, which this drive doesn't allow"
                )
            }
            NameProblem::TooLong { limit } => {
                let n = match limit {
                    NameLimit::Bytes(n) | NameLimit::Utf16Units(n) => n,
                };
                write!(
                    f,
                    "the name is longer than this drive allows ({n} characters)"
                )
            }
        }
    }
}

const WINDOWS_INVALID: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Checks one file or folder name against the destination file system. Windows rules
/// apply on NTFS, ReFS, exFAT and FAT, and on every destination when running on Windows.
pub fn check_name(name: &OsStr, fs: &FsInfo) -> Result<(), NameProblem> {
    let text = name.to_string_lossy();
    if too_long(name, &text, fs.name_limit) {
        return Err(NameProblem::TooLong {
            limit: fs.name_limit,
        });
    }
    if !(cfg!(windows) || fs.kind.has_windows_names()) {
        return Ok(());
    }
    if let Some(c) = text
        .chars()
        .find(|&c| c.is_ascii_control() || WINDOWS_INVALID.contains(&c))
    {
        return Err(NameProblem::InvalidChar(c));
    }
    if text.ends_with(['.', ' ']) {
        return Err(NameProblem::TrailingDotOrSpace);
    }
    let stem = text.split('.').next().unwrap_or_default().trim_end();
    if WINDOWS_RESERVED
        .iter()
        .any(|r| r.eq_ignore_ascii_case(stem))
    {
        return Err(NameProblem::Reserved);
    }
    Ok(())
}

/// Checks every component of a relative path; returns the first problem.
pub fn check_path(rel: &Path, fs: &FsInfo) -> Result<(), NameProblem> {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .try_for_each(|name| check_name(name, fs))
}

fn too_long(name: &OsStr, text: &str, limit: NameLimit) -> bool {
    match limit {
        NameLimit::Bytes(n) => name.as_encoded_bytes().len() > n,
        NameLimit::Utf16Units(n) => text.encode_utf16().count() > n,
    }
}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): check names against the destination file system

Windows rules (reserved names, invalid characters, trailing dot or space)
on NTFS, ReFS, exFAT and FAT and on Windows itself, and the name-length
limit everywhere (FR-16).
EOF
```


---

### Task 6: Lock partial files, replace stale ones, short partial names

Three plan 1 carry-overs in one place, `copy.rs` and `os.rs`:

- **Stale partial files.** A partial file left by a crashed job made that file fail
  forever. Now every partial file is locked while its writer lives: `flock` on
  macOS/Linux (not `fcntl` locks, which are per process and can't separate two jobs in one
  app), and on Windows a share mode without `FILE_SHARE_DELETE`. The lock is held until the
  file is renamed or removed; `PartialCopy` owns the open file and gets `commit()` and
  `discard()` methods. A partial file that is already there is replaced only if no one
  holds its lock, it is at least 2 s old and it is still the file at that name. Otherwise
  the file fails with `PartialInUse`.
- **Why the 2 s rule:** on Unix, creating and locking a file are two steps. Without it, a
  cleanup can remove another job's brand-new file in that gap. The prototype's concurrency
  test (`concurrent_jobs_into_one_destination_never_report_foreign_bytes`) caught this in
  two forms: two cleanups deleting each other's files so both jobs gave up, and a writer
  renaming another writer's file into place. The creator also waits for the lock and checks
  it still owns the name.
- **Windows commit.** A no-replace `MoveFileExW` replaces plan 1's hard link. It fails on
  a name that is taken and on another writer's open file.
- **Long names.** `.<name>.secopy-partial` becomes `.secopy-<xxh64 of the lowercased
  name>.partial` when it would exceed 255 bytes or UTF-16 units.

**Files:**
- Modify: `crates/secopy-core/src/copy.rs`, `crates/secopy-core/src/error.rs`, `crates/secopy-core/src/job/mod.rs`, `crates/secopy-core/src/job/runner.rs`, `crates/secopy-core/src/os.rs`, `crates/secopy-core/tests/copy.rs`, `crates/secopy-core/tests/job.rs`

**Interfaces:**
- Consumes: Task 1's `copy_to_partial`.
- Produces: `PartialCopy { partial, hash, bytes, removed_stale, .. }` with
  `commit(self, final_path) -> Result<(), FileError>` (replaces `copy::commit`) and
  `discard(self)`; `FileError::PartialInUse`; `JobReport::removed_partials: u64`;
  `os::{create_locked, remove_stale}`; `os::rename_noreplace` on Windows.

- [ ] **Step 1: Write the failing tests**

Update the tests in `crates/secopy-core/tests/copy.rs`:

```diff
diff --git a/crates/secopy-core/tests/copy.rs b/crates/secopy-core/tests/copy.rs
index fee60de..48a2245 100644
--- a/crates/secopy-core/tests/copy.rs
+++ b/crates/secopy-core/tests/copy.rs
@@ -5,7 +5,7 @@ use std::fs;
 
 use common::pattern;
 use secopy_core::control::JobControl;
-use secopy_core::copy::{CopyConfig, commit, copy_to_partial, partial_path};
+use secopy_core::copy::{CopyConfig, copy_to_partial, partial_path};
 use secopy_core::error::FileError;
 use secopy_core::hash::hash_bytes;
 
@@ -41,9 +41,10 @@ fn small_file_is_copied_hashed_and_committed() {
     assert_eq!(pc.bytes, 12);
     assert!(!dst.exists(), "final name only appears on commit");
 
-    commit(&pc.partial, &dst).unwrap();
+    let partial = pc.partial.clone();
+    pc.commit(&dst).unwrap();
     assert_eq!(fs::read(&dst).unwrap(), b"hello secopy");
-    assert!(!pc.partial.exists());
+    assert!(!partial.exists());
 }
 
 #[test]
@@ -145,22 +146,86 @@ fn sizes_around_the_buffer_size_copy_exactly() {
 }
 
 #[test]
-fn another_writers_partial_file_is_never_truncated() {
+fn a_live_writers_partial_file_is_never_touched() {
     let dir = tempfile::tempdir().unwrap();
     let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
     fs::write(&src, b"mine").unwrap();
-    fs::write(partial_path(&dst), b"other writer").unwrap();
+    // The first writer creates and locks its partial file, then waits at the pause.
+    let paused = JobControl::new();
+    paused.pause();
+    std::thread::scope(|s| {
+        let first =
+            s.spawn(|| copy_to_partial(&src, &dst, &CopyConfig::default(), &|_| {}, &paused));
+        while !partial_path(&dst).exists() {
+            std::thread::yield_now();
+        }
+        let err = copy_to_partial(
+            &src,
+            &dst,
+            &CopyConfig::default(),
+            &|_| {},
+            &JobControl::new(),
+        )
+        .unwrap_err();
+        assert_eq!(err, FileError::PartialInUse);
+        assert!(
+            partial_path(&dst).exists(),
+            "the live partial file is left alone"
+        );
+        paused.resume();
+        let pc = first.join().unwrap().unwrap();
+        pc.commit(&dst).unwrap();
+    });
+    assert_eq!(fs::read(&dst).unwrap(), b"mine");
+}
 
-    let err = copy_to_partial(
+#[test]
+fn a_stale_partial_file_is_replaced() {
+    let dir = tempfile::tempdir().unwrap();
+    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
+    fs::write(&src, b"mine").unwrap();
+    fs::write(partial_path(&dst), b"left by a crashed job").unwrap();
+    age(&partial_path(&dst));
+    let pc = copy_to_partial(
         &src,
         &dst,
         &CopyConfig::default(),
         &|_| {},
         &JobControl::new(),
     )
-    .unwrap_err();
-    assert_eq!(err, FileError::NameClash);
-    assert_eq!(fs::read(partial_path(&dst)).unwrap(), b"other writer");
+    .unwrap();
+    assert!(pc.removed_stale);
+    assert_eq!(fs::read(&pc.partial).unwrap(), b"mine");
+}
+
+#[test]
+fn long_names_get_a_short_partial_name() {
+    let dir = tempfile::tempdir().unwrap();
+    // 250 bytes: the name fits, but `.<name>.secopy-partial` would not.
+    let long = format!("{}.mov", "a".repeat(246));
+    let (src, dst) = (dir.path().join("a.bin"), dir.path().join(&long));
+    fs::write(&src, b"clip").unwrap();
+    let partial = partial_path(&dst);
+    let name = partial.file_name().unwrap().to_str().unwrap();
+    assert!(
+        name.starts_with(".secopy-") && name.ends_with(".partial"),
+        "{name}"
+    );
+    assert_eq!(
+        partial,
+        partial_path(&dir.path().join(long.to_uppercase())),
+        "the short name ignores case"
+    );
+    let pc = copy_to_partial(
+        &src,
+        &dst,
+        &CopyConfig::default(),
+        &|_| {},
+        &JobControl::new(),
+    )
+    .unwrap();
+    pc.commit(&dst).unwrap();
+    assert_eq!(fs::read(&dst).unwrap(), b"clip");
 }
 
 #[test]
@@ -178,8 +243,24 @@ fn commit_never_replaces_an_existing_file() {
     .unwrap();
     fs::write(&dst, b"mine").unwrap();
 
-    assert_eq!(commit(&pc.partial, &dst), Err(FileError::AlreadyExists));
+    let partial = pc.partial.clone();
+    assert_eq!(pc.commit(&dst), Err(FileError::AlreadyExists));
     assert_eq!(fs::read(&dst).unwrap(), b"mine");
+    assert!(
+        !partial.exists(),
+        "a failed commit removes the partial file"
+    );
+}
+
+/// Makes a file look as if it was written a minute ago.
+fn age(path: &std::path::Path) {
+    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
+    fs::File::options()
+        .write(true)
+        .open(path)
+        .unwrap()
+        .set_modified(old)
+        .unwrap();
 }
 
 fn cancelled() -> JobControl {
```

Update the tests in `crates/secopy-core/tests/job.rs`:

```diff
diff --git a/crates/secopy-core/tests/job.rs b/crates/secopy-core/tests/job.rs
index 8462449..582688b 100644
--- a/crates/secopy-core/tests/job.rs
+++ b/crates/secopy-core/tests/job.rs
@@ -587,3 +587,23 @@ fn a_paused_job_does_no_io_until_resumed() {
     assert!(report.is_success(), "{report:?}");
     assert_eq!(read_tree(&dest.join("src")), read_tree(&src));
 }
+
+#[test]
+fn partial_files_left_by_an_interrupted_job_are_replaced() {
+    let f = fixture();
+    let leftover = f.dest.join("CARD/.notes.txt.secopy-partial");
+    write_files(&f.dest, &[("CARD/.notes.txt.secopy-partial", b"half")]);
+    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
+    fs::File::options()
+        .write(true)
+        .open(&leftover)
+        .unwrap()
+        .set_modified(old)
+        .unwrap();
+
+    let (report, _) = run(&select(&f.src), &f.dest, &opts(true));
+    assert!(report.is_success(), "{report:?}");
+    assert_eq!(report.removed_partials, 1);
+    assert!(!leftover.exists());
+    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
+}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test copy --test job`
Expected: compile errors: no method `commit` on `PartialCopy`, no field `removed_stale`, no variant `PartialInUse`.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/copy.rs`:

```diff
diff --git a/crates/secopy-core/src/copy.rs b/crates/secopy-core/src/copy.rs
index 646af62..fd2deac 100644
--- a/crates/secopy-core/src/copy.rs
+++ b/crates/secopy-core/src/copy.rs
@@ -1,7 +1,7 @@
 //! Copies one file to a temporary "partial" file while hashing it (FR-18, FR-20, RFD §7.2).
 
 use std::ffi::OsString;
-use std::fs::{self, File, OpenOptions};
+use std::fs::{self, File};
 use std::io::{self, Read, Write};
 use std::path::{Path, PathBuf};
 use std::sync::mpsc;
@@ -31,34 +31,84 @@ impl Default for CopyConfig {
     }
 }
 
-/// A fully written and flushed copy that still has its temporary name.
+/// A fully written and flushed copy that still has its temporary name. It stays open, and
+/// so locked, until it is committed or discarded: no other job can mistake it for a
+/// partial file left by an interrupted job (FR-18).
 #[derive(Debug)]
 pub struct PartialCopy {
     pub partial: PathBuf,
     /// xxHash64 of the bytes read from the source.
     pub hash: u64,
     pub bytes: u64,
+    /// A partial file left by an interrupted job was removed to make room for this one.
+    pub removed_stale: bool,
+    file: File,
 }
 
-/// Temporary name used while a file is written: `.<name>.secopy-partial`, same directory.
+impl PartialCopy {
+    /// Gives the file its final name without ever replacing an existing file (FR-18). The
+    /// file system itself decides whether the name is taken (case, Unicode normalization).
+    /// On failure the partial file is removed.
+    pub fn commit(self, final_path: &Path) -> Result<(), FileError> {
+        self.release(|partial| {
+            let result = commit_noreplace(partial, final_path);
+            if result.is_err() {
+                let _ = fs::remove_file(partial);
+            }
+            result
+        })
+    }
+
+    /// Deletes the partial file.
+    pub fn discard(self) {
+        self.release(|partial| {
+            let _ = fs::remove_file(partial);
+        });
+    }
+
+    /// Runs `op` on the partial path. On Unix the file stays open, and locked, until `op`
+    /// has renamed or removed it, so no other job can take the name in between. Windows
+    /// can't rename or delete a file that is open without delete sharing, so there it is
+    /// closed first; another writer's file can't be renamed or deleted in that gap either.
+    fn release<T>(self, op: impl FnOnce(&Path) -> T) -> T {
+        let PartialCopy { partial, file, .. } = self;
+        #[cfg(windows)]
+        drop(file);
+        let out = op(&partial);
+        #[cfg(not(windows))]
+        drop(file);
+        out
+    }
+}
+
+/// Longest file name every supported file system accepts, in bytes and in UTF-16 units.
+const MAX_NAME: usize = 255;
+
+/// Temporary name used while a file is written, in the same directory:
+/// `.<name>.secopy-partial`, or `.secopy-<hash>.partial` if that would be too long.
+/// The hash ignores case, so two names a case-insensitive drive treats as one file
+/// still map to one partial file.
 pub fn partial_path(final_path: &Path) -> PathBuf {
-    let mut name = OsString::from(".");
-    name.push(
-        final_path
-            .file_name()
-            .expect("destination path has a file name"),
-    );
-    name.push(".secopy-partial");
-    final_path.with_file_name(name)
+    let name = final_path
+        .file_name()
+        .expect("destination path has a file name");
+    let mut partial = OsString::from(".");
+    partial.push(name);
+    partial.push(".secopy-partial");
+    if partial.len() <= MAX_NAME && partial.to_string_lossy().encode_utf16().count() <= MAX_NAME {
+        return final_path.with_file_name(partial);
+    }
+    let key = hash::hash_bytes(name.to_string_lossy().to_lowercase().as_bytes());
+    final_path.with_file_name(format!(".secopy-{}.partial", hash::to_hex(key)))
 }
 
 /// Copies `src` to the partial path of `final_path`, hashing the bytes as they are read,
 /// then fsyncs it. On error or cancel the partial file is removed.
 /// `progress` receives the number of bytes written so far.
 ///
-/// The partial file is created with `create_new`: if it already exists, another writer
-/// (a clashing name in this job, or another job) owns it, and this file fails with
-/// `NameClash` instead of truncating the other writer's data.
+/// A partial file that is already there is replaced if it was left by an interrupted job.
+/// If a live writer holds it (another job, or a clashing name), this file fails with
+/// `PartialInUse` and the other writer's data is left alone.
 pub fn copy_to_partial(
     src: &Path,
     final_path: &Path,
@@ -68,37 +118,47 @@ pub fn copy_to_partial(
 ) -> Result<PartialCopy, FileError> {
     let partial = partial_path(final_path);
     let reader = File::open(src).map_err(FileError::read_source)?;
-    let mut writer = match OpenOptions::new()
-        .write(true)
-        .create_new(true)
-        .open(&partial)
-    {
-        Ok(file) => file,
-        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(FileError::NameClash),
-        Err(e) => return Err(FileError::write_dest(e)),
-    };
-    let result = copy_inner(reader, &mut writer, cfg, progress, control);
-    // Close before removing: Windows cannot delete an open file.
-    drop(writer);
-    match result {
+    let (mut writer, removed_stale) = create_partial(&partial)?;
+    match copy_inner(reader, &mut writer, cfg, progress, control) {
         Ok((hash, bytes)) => Ok(PartialCopy {
             partial,
             hash,
             bytes,
+            removed_stale,
+            file: writer,
         }),
         Err(e) => {
+            // Close before removing: Windows cannot delete an open file.
+            drop(writer);
             let _ = fs::remove_file(&partial);
             Err(e)
         }
     }
 }
 
-/// Gives a finished partial file its final name without ever replacing an existing file
-/// (FR-18). The file system itself decides whether the name is taken (case, Unicode
-/// normalization). Uses a no-replace rename where the OS has one (macOS, Linux), else a
-/// hard link (Windows); file systems with neither (FAT, exFAT) fall back to
-/// check-then-rename.
-pub fn commit(partial: &Path, final_path: &Path) -> Result<(), FileError> {
+/// Creates the partial file, replacing one left by an interrupted job. Returns whether a
+/// stale file was removed.
+fn create_partial(partial: &Path) -> Result<(File, bool), FileError> {
+    let mut removed = false;
+    // A few rounds: another job's cleanup can race with ours.
+    for _ in 0..3 {
+        match os::create_locked(partial) {
+            Ok(file) => return Ok((file, removed)),
+            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
+                if !os::remove_stale(partial).map_err(FileError::write_dest)? {
+                    return Err(FileError::PartialInUse);
+                }
+                removed = true;
+            }
+            Err(e) => return Err(FileError::write_dest(e)),
+        }
+    }
+    Err(FileError::PartialInUse)
+}
+
+/// Uses the OS's no-replace rename. File systems without one (FAT and exFAT on macOS and
+/// Linux) fall back to a hard link, and then to check-then-rename.
+fn commit_noreplace(partial: &Path, final_path: &Path) -> Result<(), FileError> {
     match os::rename_noreplace(partial, final_path) {
         Ok(()) => return Ok(()),
         Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(FileError::AlreadyExists),
```

Change `crates/secopy-core/src/error.rs`:

```diff
diff --git a/crates/secopy-core/src/error.rs b/crates/secopy-core/src/error.rs
index ed757f0..89da2d9 100644
--- a/crates/secopy-core/src/error.rs
+++ b/crates/secopy-core/src/error.rs
@@ -39,6 +39,8 @@ pub enum FileError {
     AlreadyExists,
     #[error("another file in this copy has the same name (names are compared ignoring case)")]
     NameClash,
+    #[error("another copy is writing this file")]
+    PartialInUse,
     #[error("cancelled")]
     Cancelled,
 }
```

Change `crates/secopy-core/src/job/mod.rs`:

```diff
diff --git a/crates/secopy-core/src/job/mod.rs b/crates/secopy-core/src/job/mod.rs
index 2d6e6da..4a1f954 100644
--- a/crates/secopy-core/src/job/mod.rs
+++ b/crates/secopy-core/src/job/mod.rs
@@ -98,6 +98,8 @@ pub struct JobReport {
     pub checksum_error: Option<String>,
     /// `None` when not verifying.
     pub cache_bypass: Option<CacheBypass>,
+    /// Partial files left by interrupted jobs that were removed (FR-18).
+    pub removed_partials: u64,
     pub fatal: Option<FileError>,
     pub cancelled: bool,
     pub elapsed: Duration,
@@ -204,6 +206,7 @@ pub fn run_job(
         } else {
             CacheBypass::Active
         }),
+        removed_partials: runner.removed_partials.load(Relaxed),
         cancelled: control.is_stopped() && fatal.is_none(),
         fatal,
         elapsed: started.elapsed(),
```

Change `crates/secopy-core/src/job/runner.rs`:

```diff
diff --git a/crates/secopy-core/src/job/runner.rs b/crates/secopy-core/src/job/runner.rs
index 1b65fbc..d4b21d6 100644
--- a/crates/secopy-core/src/job/runner.rs
+++ b/crates/secopy-core/src/job/runner.rs
@@ -68,6 +68,8 @@ pub(super) struct Runner<'a> {
     pub(super) outcomes: Mutex<Vec<FileOutcome>>,
     pub(super) fatal: Mutex<Option<FileError>>,
     pub(super) bypass_unavailable: AtomicBool,
+    /// Partial files left by interrupted jobs that were removed (FR-18).
+    pub(super) removed_partials: AtomicU64,
 }
 
 impl<'a> Runner<'a> {
@@ -98,6 +100,7 @@ impl<'a> Runner<'a> {
             outcomes: Mutex::new(Vec::new()),
             fatal: Mutex::new(None),
             bypass_unavailable: AtomicBool::new(false),
+            removed_partials: AtomicU64::new(0),
         }
     }
 
@@ -143,12 +146,9 @@ impl<'a> Runner<'a> {
             return;
         }
         let hash = partial.hash;
-        match copy::commit(&partial.partial, &final_path) {
+        match partial.commit(&final_path) {
             Ok(()) => self.finish(&slot, entry, Some(hash), FileStatus::Copied, started),
-            Err(e) => {
-                let _ = fs::remove_file(&partial.partial);
-                self.finish(&slot, entry, Some(hash), FileStatus::Failed(e), started)
-            }
+            Err(e) => self.finish(&slot, entry, Some(hash), FileStatus::Failed(e), started),
         }
     }
 
@@ -167,6 +167,9 @@ impl<'a> Runner<'a> {
             &|b| slot.bytes.store(b, Relaxed),
             self.control,
         )?;
+        if partial.removed_stale {
+            self.removed_partials.fetch_add(1, Relaxed);
+        }
         if let Some(hook) = self.opts.hooks.after_copy {
             hook(&partial.partial, attempt);
         }
@@ -184,8 +187,6 @@ impl<'a> Runner<'a> {
     }
 
     /// Verifies the partial file, re-copying once on mismatch (FR-27), then commits it.
-    /// Removes only partial files this lane created: a failed re-copy may have hit a
-    /// partial file that belongs to another writer.
     fn verify_one(&self, task: VerifyTask) {
         let VerifyTask {
             slot,
@@ -206,24 +207,21 @@ impl<'a> Runner<'a> {
             ) {
                 Ok(v) => v,
                 Err(e) => {
-                    let _ = fs::remove_file(&partial.partial);
+                    partial.discard();
                     break Err(e);
                 }
             };
             if bypass == CacheBypass::Unavailable {
                 self.bypass_unavailable.store(true, Relaxed);
             }
-            if actual == partial.hash {
-                let committed = copy::commit(&partial.partial, &final_path);
-                if committed.is_err() {
-                    let _ = fs::remove_file(&partial.partial);
-                }
-                break committed.map(|()| partial.hash);
+            let expected = partial.hash;
+            if actual == expected {
+                break partial.commit(&final_path).map(|()| expected);
             }
-            let _ = fs::remove_file(&partial.partial);
+            partial.discard();
             if attempt == 1 {
                 break Err(FileError::HashMismatch {
-                    expected: hash::to_hex(partial.hash),
+                    expected: hash::to_hex(expected),
                     actual: hash::to_hex(actual),
                 });
             }
```

Change `crates/secopy-core/src/os.rs`:

```diff
diff --git a/crates/secopy-core/src/os.rs b/crates/secopy-core/src/os.rs
index 7370241..170e779 100644
--- a/crates/secopy-core/src/os.rs
+++ b/crates/secopy-core/src/os.rs
@@ -1,6 +1,6 @@
 //! Platform-specific flushing and cache control (FR-18, FR-26, RFD §7.4).
 
-use std::fs::File;
+use std::fs::{self, File, OpenOptions};
 use std::io;
 use std::path::Path;
 
@@ -124,7 +124,23 @@ pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
     }
 }
 
-#[cfg(not(any(target_os = "macos", target_os = "linux")))]
+/// Windows: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` fails if the name is taken.
+/// It also fails on a file another writer has open without delete sharing.
+#[cfg(windows)]
+pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
+    use std::os::windows::ffi::OsStrExt;
+    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
+    let wide = |p: &Path| -> Vec<u16> { p.as_os_str().encode_wide().chain([0]).collect() };
+    let (from, to) = (wide(from), wide(to));
+    // SAFETY: both pointers are valid NUL-terminated wide strings for the call's duration.
+    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } != 0 {
+        Ok(())
+    } else {
+        Err(io::Error::last_os_error())
+    }
+}
+
+#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
 pub fn rename_noreplace(_from: &Path, _to: &Path) -> io::Result<()> {
     Err(io::ErrorKind::Unsupported.into())
 }
@@ -144,3 +160,108 @@ fn unsupported_or(e: io::Error) -> io::Error {
         _ => e,
     }
 }
+
+/// Creates a new partial file that stays locked while it is open, so another job can tell
+/// a live partial file from one left by an interrupted job (FR-18). On Unix this is an
+/// advisory `flock`; on Windows the file is opened without delete sharing, so nobody can
+/// delete or rename it while it is open.
+pub fn create_locked(path: &Path) -> io::Result<File> {
+    let mut opts = OpenOptions::new();
+    opts.write(true).create_new(true);
+    #[cfg(windows)]
+    {
+        use std::os::windows::fs::OpenOptionsExt;
+        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
+        opts.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
+    }
+    let file = opts.open(path)?;
+    // Only another job's cleanup can hold a brand-new file's lock, and only for a moment,
+    // so wait for it. That cleanup may have removed the new file: then the name is no
+    // longer ours. Lock errors (file systems without locks) leave the file unlocked.
+    #[cfg(unix)]
+    {
+        let _ = lock(&file, true);
+        if !is_at(&file, path)? {
+            return Err(io::ErrorKind::AlreadyExists.into());
+        }
+    }
+    Ok(file)
+}
+
+/// Removes a partial file left by an interrupted job. Returns `false`, and leaves the file
+/// alone, if a live writer still holds it.
+#[cfg(unix)]
+pub fn remove_stale(path: &Path) -> io::Result<bool> {
+    let file = match File::open(path) {
+        Ok(file) => file,
+        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(true),
+        Err(e) => return Err(e),
+    };
+    if let Ok(false) = lock(&file, false) {
+        return Ok(false);
+    }
+    // Creating and locking a file are two steps on Unix: a file this young may be one
+    // another writer has just created and not locked yet.
+    let age = file.metadata()?.modified()?.elapsed().unwrap_or_default();
+    if age < STALE_AFTER {
+        return Ok(false);
+    }
+    // The name may belong to another writer's new file by now.
+    if !is_at(&file, path)? {
+        return Ok(!path.exists());
+    }
+    match fs::remove_file(path) {
+        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
+        _ => Ok(true),
+    }
+}
+
+#[cfg(windows)]
+pub fn remove_stale(path: &Path) -> io::Result<bool> {
+    use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;
+    match fs::remove_file(path) {
+        Ok(()) => Ok(true),
+        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
+        Err(e) if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32) => Ok(false),
+        Err(e) => Err(e),
+    }
+}
+
+/// A partial file younger than this is never treated as left by an interrupted job.
+#[cfg(unix)]
+const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(2);
+
+/// Whether `path` still names the open `file`.
+#[cfg(unix)]
+fn is_at(file: &File, path: &Path) -> io::Result<bool> {
+    use std::os::unix::fs::MetadataExt;
+    let held = file.metadata()?;
+    match fs::symlink_metadata(path) {
+        Ok(now) => Ok((now.dev(), now.ino()) == (held.dev(), held.ino())),
+        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
+        Err(e) => Err(e),
+    }
+}
+
+/// Takes the exclusive advisory lock; without `wait`, `Ok(false)` if someone holds it.
+/// `flock`, not `fcntl` locks: those are per process, and two jobs in one app must see
+/// each other's locks.
+#[cfg(unix)]
+fn lock(file: &File, wait: bool) -> io::Result<bool> {
+    use std::os::fd::AsRawFd;
+    let op = if wait {
+        libc::LOCK_EX
+    } else {
+        libc::LOCK_EX | libc::LOCK_NB
+    };
+    // SAFETY: `flock` on a valid descriptor owned by `file`.
+    if unsafe { libc::flock(file.as_raw_fd(), op) } == 0 {
+        return Ok(true);
+    }
+    let e = io::Error::last_os_error();
+    if e.raw_os_error() == Some(libc::EWOULDBLOCK) {
+        Ok(false)
+    } else {
+        Err(e)
+    }
+}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

Also lint the other platforms' `cfg` code (install the targets once with
`rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`):
`cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` and
`cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`.
Expected: no warnings.

The concurrency tests are the point of this task. Run them repeatedly:
`for i in $(seq 1 30); do cargo test -q -p secopy-core --test job --test copy || break; done`
Expected: 30 clean runs.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
fix(core): lock partial files and replace ones left by interrupted jobs

A partial file left by a crashed job made that file fail forever. Now
partial files are locked while written (flock; on Windows no delete
sharing) until renamed or removed, and a leftover is replaced when no one
holds it, it is older than 2 s and still has that name (FR-18). On
Windows a no-replace MoveFileExW replaces the hard-link commit. Names
too long for the partial suffix get a short hashed partial name.
EOF
```


---

### Task 7: Pre-flight checks (`preflight`)

`preflight(&Source, &Selection, dest) -> Result<Preflight, Blocker>`. The destination
missing, unwritable, or the source or inside it are returned as the error. Everything else
is listed: per-file problems (invalid name, too large for FAT, same destination name as
another file in the job (FR-17a) with case ignored only on case-insensitive drives, a file
or folder in the way), conflicts (identical = same size and mtimes less than 2 s apart,
or differs), stale partial files, names the checksum file can't hold, and the source
roots with their device ids for Task 12.

**Files:**
- Create: `crates/secopy-core/src/preflight.rs`, `crates/secopy-core/tests/preflight.rs`
- Modify: `crates/secopy-core/src/lib.rs`

**Interfaces:**
- Consumes: Tasks 3–6 (`ScanEntry::mtime`, `fs_info`, `device_id`, `check_path`, `partial_path`).
- Produces: `preflight::{preflight, Preflight { dest, fs, file_problems, conflicts, stale_partials, checksum_omissions, source_roots }, Blocker::{DestMissing, DestNotWritable(IoFailure), DestInsideSource, NotEnoughSpace { needed, free }}, FileProblem { id, kind }, ProblemKind::{InvalidName, TooLarge { limit }, NameClash, InTheWay { path }}, Conflict { id, kind }, ConflictKind::{Identical, Differs { size, mtime }}, SourceRoot { path, device }, SAME_MTIME}`;
  `pub(crate) clash_key(&Path, case_sensitive) -> Vec<u8>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/secopy-core/src/preflight.rs` with its unit tests; the implementation goes above them in the implementation step:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsinfo::{FAT_MAX_FILE_SIZE, FsKind, NameLimit};

    fn entry(rel: &str, size: u64) -> ScanEntry {
        ScanEntry {
            source: PathBuf::from("/src").join(rel),
            rel: PathBuf::from(rel),
            size,
            ext: None,
            mtime: None,
        }
    }

    fn fat(case_sensitive: bool) -> FsInfo {
        FsInfo {
            kind: FsKind::Fat,
            case_sensitive,
            free_bytes: u64::MAX,
            max_file_size: Some(FAT_MAX_FILE_SIZE),
            name_limit: NameLimit::Utf16Units(255),
            device: 0,
        }
    }

    fn problems(files: Vec<ScanEntry>, fs: &FsInfo) -> Vec<(usize, ProblemKind)> {
        let dir = tempfile::tempdir().unwrap();
        let sel = Selection {
            files,
            ..Selection::default()
        };
        check_files(&sel, dir.path(), fs)
            .problems
            .into_iter()
            .map(|p| (p.id, p.kind))
            .collect()
    }

    #[test]
    fn per_file_problems_on_fat() {
        let files = vec![
            entry("ok.mov", 10),
            entry("a:b.mov", 10),
            entry("huge.mov", FAT_MAX_FILE_SIZE + 1),
            entry("OK.MOV", 10),
        ];
        assert_eq!(
            problems(files, &fat(false)),
            vec![
                (1, ProblemKind::InvalidName(NameProblem::InvalidChar(':'))),
                (
                    2,
                    ProblemKind::TooLarge {
                        limit: FAT_MAX_FILE_SIZE
                    }
                ),
                (3, ProblemKind::NameClash),
            ]
        );
    }

    #[test]
    fn names_differing_in_case_only_clash_on_case_insensitive_drives() {
        let files = || vec![entry("a.txt", 1), entry("A.TXT", 1)];
        assert_eq!(
            problems(files(), &fat(false)),
            vec![(1, ProblemKind::NameClash)]
        );
        assert_eq!(problems(files(), &fat(true)), vec![]);
    }

    #[test]
    fn identical_means_same_size_and_mtime_within_two_seconds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mov");
        fs::write(&path, b"12345").unwrap();
        let meta = fs::metadata(&path).unwrap();
        let t = meta.modified().unwrap();
        let at = |mtime: SystemTime, size| ScanEntry {
            mtime: Some(mtime),
            ..entry("a.mov", size)
        };
        assert_eq!(conflict_kind(&at(t, 5), &meta), ConflictKind::Identical);
        let later = t + Duration::from_millis(1500);
        assert_eq!(conflict_kind(&at(later, 5), &meta), ConflictKind::Identical);
        let differs = ConflictKind::Differs {
            size: 5,
            mtime: Some(t),
        };
        assert_eq!(
            conflict_kind(&at(t + Duration::from_secs(3), 5), &meta),
            differs
        );
        assert_eq!(conflict_kind(&at(t, 6), &meta), differs);
    }
}
```

Create `crates/secopy-core/tests/preflight.rs`:

```rust
mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::write_files;
use secopy_core::filter::ExtensionFilter;
use secopy_core::preflight::{Blocker, ConflictKind, ProblemKind, preflight};
use secopy_core::scan::{ScanOptions, Selection, scan};
use secopy_core::source::{DirMode, Source};

fn card(root: &Path) -> (Source, Selection) {
    let src = root.join("CARD");
    write_files(&src, &[("A001.mov", b"clip-a"), ("B002.mov", b"clip-b")]);
    let source = Source::Directory {
        path: src,
        mode: DirMode::FolderItself,
    };
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    (source, sel)
}

fn set_mtime(path: &Path, t: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn id_of(sel: &Selection, name: &str) -> usize {
    sel.files
        .iter()
        .position(|f| f.rel.ends_with(name))
        .unwrap()
}

#[test]
fn a_missing_destination_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let err = preflight(&source, &sel, &dir.path().join("nope")).unwrap_err();
    assert_eq!(err, Blocker::DestMissing);
}

#[test]
fn a_destination_inside_the_source_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let inside = dir.path().join("CARD/backup");
    fs::create_dir_all(&inside).unwrap();
    assert_eq!(
        preflight(&source, &sel, &inside).unwrap_err(),
        Blocker::DestInsideSource
    );
    assert_eq!(
        preflight(&source, &sel, &dir.path().join("CARD")).unwrap_err(),
        Blocker::DestInsideSource
    );
}

#[test]
fn a_clean_destination_has_nothing_to_report() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let pf = preflight(&source, &sel, &dest).unwrap();
    assert!(pf.file_problems.is_empty());
    assert!(pf.conflicts.is_empty());
    assert!(pf.stale_partials.is_empty());
    assert_eq!(pf.source_roots.len(), 1);
    assert!(pf.fs.free_bytes > 0);
}

#[test]
fn existing_files_are_identical_or_different() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    write_files(
        &dest,
        &[
            ("CARD/A001.mov", b"clip-a"),
            ("CARD/B002.mov", b"another clip"),
        ],
    );
    let (source, _) = card(dir.path());
    let t = SystemTime::now() - Duration::from_secs(3600);
    for name in ["A001.mov", "B002.mov"] {
        set_mtime(&dir.path().join("CARD").join(name), t);
        set_mtime(&dest.join("CARD").join(name), t);
    }
    // Scan after setting the source mtimes.
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &dest).unwrap();
    let kind = |name| {
        pf.conflicts
            .iter()
            .find(|c| c.id == id_of(&sel, name))
            .map(|c| c.kind.clone())
    };
    assert_eq!(kind("A001.mov"), Some(ConflictKind::Identical));
    assert!(matches!(
        kind("B002.mov"),
        Some(ConflictKind::Differs { size: 12, .. })
    ));
}

#[test]
fn a_folder_where_a_file_goes_is_in_the_way() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let dest = dir.path().join("dest");
    fs::create_dir_all(dest.join("CARD/A001.mov")).unwrap();
    let pf = preflight(&source, &sel, &dest).unwrap();
    assert_eq!(pf.file_problems.len(), 1);
    assert_eq!(pf.file_problems[0].id, id_of(&sel, "A001.mov"));
    assert!(matches!(
        pf.file_problems[0].kind,
        ProblemKind::InTheWay { .. }
    ));
}

#[test]
fn leftover_partial_files_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let dest = dir.path().join("dest");
    write_files(&dest, &[("CARD/.A001.mov.secopy-partial", b"half")]);
    let pf = preflight(&source, &sel, &dest).unwrap();
    assert_eq!(
        pf.stale_partials,
        vec![dest.join("CARD").join(".A001.mov.secopy-partial")]
    );
}

#[test]
fn loose_files_report_each_parent_folder_once() {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[("x/a.wav", b"a"), ("x/b.wav", b"b"), ("y/c.wav", b"c")],
    );
    let source = Source::Files(
        ["x/a.wav", "x/b.wav", "y/c.wav"]
            .map(|p| dir.path().join(p))
            .to_vec(),
    );
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let pf = preflight(&source, &sel, &dest).unwrap();
    let roots: Vec<PathBuf> = pf.source_roots.into_iter().map(|r| r.path).collect();
    assert_eq!(roots, vec![dir.path().join("x"), dir.path().join("y")]);
}

/// Linux file names are bytes; the checksum file can only hold UTF-8 (FR-31).
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_names_are_left_out_of_the_checksum_file() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join(OsStr::from_bytes(b"caf\xe9.wav")), b"x").unwrap();
    let source = Source::Directory {
        path: src,
        mode: DirMode::FolderItself,
    };
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let pf = preflight(&source, &sel, &dest).unwrap();
    assert_eq!(pf.checksum_omissions, vec![0]);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test preflight --lib preflight`
Expected: compile errors: module `preflight` doesn't exist.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/lib.rs`:

```diff
diff --git a/crates/secopy-core/src/lib.rs b/crates/secopy-core/src/lib.rs
index 1b049f6..01627f8 100644
--- a/crates/secopy-core/src/lib.rs
+++ b/crates/secopy-core/src/lib.rs
@@ -12,6 +12,7 @@ pub mod hidden;
 pub mod job;
 pub mod names;
 mod os;
+pub mod preflight;
 pub mod scan;
 pub mod source;
 pub mod verify;
```

Add the implementation at the top of `crates/secopy-core/src/preflight.rs`, above the tests:

```rust
//! Checks a selection against the destination before a job starts (FR-16, FR-17, FR-17a).

use std::collections::HashSet;
use std::fmt;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::copy::partial_path;
use crate::error::IoFailure;
use crate::fsinfo::{self, FsInfo};
use crate::names::{self, NameProblem};
use crate::scan::{ScanEntry, Selection};
use crate::source::Source;

/// Why the job can't start at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocker {
    DestMissing,
    DestNotWritable(IoFailure),
    DestInsideSource,
    NotEnoughSpace { needed: u64, free: u64 },
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Blocker::DestMissing => write!(f, "the destination is not an existing folder"),
            Blocker::DestNotWritable(e) => write!(f, "can't write to the destination: {e}"),
            Blocker::DestInsideSource => {
                write!(f, "the destination is the source folder or inside it")
            }
            Blocker::NotEnoughSpace { needed, free } => write!(
                f,
                "not enough free space: {needed} bytes needed, {free} bytes free"
            ),
        }
    }
}

/// A file that will fail if the job starts anyway (FR-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileProblem {
    /// Index in `Selection::files`.
    pub id: usize,
    pub kind: ProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemKind {
    InvalidName(NameProblem),
    /// Larger than the destination file system allows (FAT: 4 GiB).
    TooLarge {
        limit: u64,
    },
    /// Another file in this job has the same destination path (FR-17a).
    NameClash,
    /// A folder is where the file goes, or a file is where one of its folders goes.
    InTheWay {
        path: PathBuf,
    },
}

/// A file that already exists at the destination (FR-17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub id: usize,
    pub kind: ConflictKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// Same size, and modified less than [`SAME_MTIME`] apart: skipped without reading it.
    Identical,
    /// The existing file's size and modification time.
    Differs {
        size: u64,
        mtime: Option<SystemTime>,
    },
}

/// A source folder and the volume it is on, re-checked after I/O errors (FR-21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRoot {
    pub path: PathBuf,
    pub device: u64,
}

#[derive(Debug, Clone)]
pub struct Preflight {
    pub dest: PathBuf,
    pub fs: FsInfo,
    pub file_problems: Vec<FileProblem>,
    pub conflicts: Vec<Conflict>,
    /// Partial files left by an interrupted job where this job will write (FR-18).
    pub stale_partials: Vec<PathBuf>,
    /// Files whose names aren't valid UTF-8, so the checksum file can't list them (FR-31).
    pub checksum_omissions: Vec<usize>,
    pub source_roots: Vec<SourceRoot>,
}

/// Modification times closer than this count as equal: FAT stores times in 2 s steps,
/// and exFAT and SMB round them.
pub const SAME_MTIME: Duration = Duration::from_secs(2);

/// Checks `sel` against `dest`. Problems that stop the whole job are returned as the
/// error; everything else is listed so the user can decide.
pub fn preflight(source: &Source, sel: &Selection, dest: &Path) -> Result<Preflight, Blocker> {
    if !fs::metadata(dest).is_ok_and(|m| m.is_dir()) {
        return Err(Blocker::DestMissing);
    }
    if let Source::Directory { path, .. } = source
        && is_inside(dest, path)
    {
        return Err(Blocker::DestInsideSource);
    }
    let fs = fsinfo::fs_info(dest).map_err(|e| Blocker::DestNotWritable(e.into()))?;
    let checked = check_files(sel, dest, &fs);
    Ok(Preflight {
        dest: dest.to_path_buf(),
        fs,
        file_problems: checked.problems,
        conflicts: checked.conflicts,
        stale_partials: checked.stale_partials,
        checksum_omissions: checked.checksum_omissions,
        source_roots: source_roots(source),
    })
}

/// True if `dest` is `source` or inside it. Symlinks are resolved, so a link that
/// points into the source is caught too.
fn is_inside(dest: &Path, source: &Path) -> bool {
    let resolve = |p: &Path| fs::canonicalize(p).or_else(|_| std::path::absolute(p));
    match (resolve(dest), resolve(source)) {
        (Ok(dest), Ok(source)) => dest.starts_with(source),
        _ => false,
    }
}

fn source_roots(source: &Source) -> Vec<SourceRoot> {
    let paths: Vec<PathBuf> = match source {
        Source::Directory { path, .. } => vec![path.clone()],
        Source::Files(files) => {
            let mut seen = HashSet::new();
            files
                .iter()
                .filter_map(|f| f.parent().map(Path::to_path_buf))
                .filter(|p| seen.insert(p.clone()))
                .collect()
        }
    };
    paths
        .into_iter()
        .filter_map(|path| {
            let device = fsinfo::device_id(&path).ok()?;
            Some(SourceRoot { path, device })
        })
        .collect()
}

#[derive(Default)]
struct Checked {
    problems: Vec<FileProblem>,
    conflicts: Vec<Conflict>,
    stale_partials: Vec<PathBuf>,
    checksum_omissions: Vec<usize>,
}

fn check_files(sel: &Selection, dest: &Path, fs: &FsInfo) -> Checked {
    let mut out = Checked::default();
    let mut seen = HashSet::new();
    for (id, entry) in sel.files.iter().enumerate() {
        if entry.rel.to_str().is_none() {
            out.checksum_omissions.push(id);
        }
        let clash = !seen.insert(clash_key(&entry.rel, fs.case_sensitive));
        let final_path = dest.join(&entry.rel);
        let problem = if clash {
            Some(ProblemKind::NameClash)
        } else if let Err(e) = names::check_path(&entry.rel, fs) {
            Some(ProblemKind::InvalidName(e))
        } else if let Some(limit) = fs.max_file_size.filter(|&l| entry.size > l) {
            Some(ProblemKind::TooLarge { limit })
        } else {
            match fs::symlink_metadata(&final_path) {
                Ok(meta) if meta.is_file() => {
                    out.conflicts.push(Conflict {
                        id,
                        kind: conflict_kind(entry, &meta),
                    });
                    None
                }
                Ok(_) => Some(ProblemKind::InTheWay {
                    path: entry.rel.clone(),
                }),
                Err(e) if e.kind() == io::ErrorKind::NotADirectory => Some(ProblemKind::InTheWay {
                    path: entry.rel.parent().unwrap_or(&entry.rel).to_path_buf(),
                }),
                Err(_) => None,
            }
        };
        if let Some(kind) = problem {
            out.problems.push(FileProblem { id, kind });
        } else if fs::symlink_metadata(partial_path(&final_path)).is_ok() {
            out.stale_partials.push(partial_path(&final_path));
        }
    }
    out
}

fn conflict_kind(entry: &ScanEntry, existing: &Metadata) -> ConflictKind {
    let mtime = existing.modified().ok();
    let same_mtime = match (entry.mtime, mtime) {
        (Some(a), Some(b)) => a.duration_since(b).unwrap_or_else(|e| e.duration()) < SAME_MTIME,
        _ => false,
    };
    if entry.size == existing.len() && same_mtime {
        ConflictKind::Identical
    } else {
        ConflictKind::Differs {
            size: existing.len(),
            mtime,
        }
    }
}

/// Paths that land on the same destination file. Compared on raw bytes, not lossy UTF-8,
/// and ignoring case only where the file system does. Unicode normalization (NFC vs NFD
/// on APFS) isn't predicted; the no-replace commit catches it.
fn clash_key(rel: &Path, case_sensitive: bool) -> Vec<u8> {
    let bytes = rel.as_os_str().as_encoded_bytes();
    if case_sensitive {
        return bytes.to_vec();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_lowercase().into_bytes(),
        Err(_) => bytes.to_ascii_lowercase(),
    }
}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): add pre-flight checks

Blocks on a missing, unwritable or nested destination, and lists
per-file problems, conflicts (identical or different), leftover partial
files and names the checksum file can't hold (FR-16, FR-17, FR-17a).
EOF
```


---

### Task 8: Resolve a plan (`plan`)

`Plan::resolve(&Selection, &Preflight, DiffersPolicy)` gives every file one action. Keep
both picks `name (1).ext`, `(2)`, … free on disk and within the plan. `blockers()` adds
the free-space check, which depends on the policy: Skip writes less than Keep both. The
plan owns copies of the entries, so the app can keep it in state. `FileError` gains the
variants pre-flight problems turn into.

**Files:**
- Create: `crates/secopy-core/src/plan.rs`, `crates/secopy-core/tests/plan.rs`
- Modify: `crates/secopy-core/src/error.rs`, `crates/secopy-core/src/lib.rs`, `crates/secopy-core/src/preflight.rs`

**Interfaces:**
- Consumes: Task 7's `Preflight`, `clash_key`.
- Produces: `plan::{Plan { dest, files, dirs, fs, source_roots, stale_partials }, PlannedFile { entry, action, in_checksum_file }, Action::{Copy, Overwrite, KeepBoth { rel }, SkipIdentical, SkipDiffers, Fail(FileError)}, DiffersPolicy::{KeepBoth (default), Overwrite, Skip}, space_margin, numbered}`;
  `Plan::{bytes_to_write(), total_bytes(), blockers()}`, `PlannedFile::final_rel()`, `Action::writes()`;
  `ProblemKind::to_error()`; `FileError::{InvalidName(NameProblem), TooLarge { limit }, InTheWay { path }}`.
  (Task 9 adds `n` to `Action::KeepBoth` and moves `numbered` to `names`.)

- [ ] **Step 1: Write the failing tests**

Create `crates/secopy-core/tests/plan.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use secopy_core::error::FileError;
use secopy_core::fsinfo::{FsInfo, FsKind, NameLimit};
use secopy_core::plan::{Action, DiffersPolicy, Plan, numbered, space_margin};
use secopy_core::preflight::{
    Blocker, Conflict, ConflictKind, FileProblem, Preflight, ProblemKind,
};
use secopy_core::scan::{ScanEntry, Selection};

fn entry(rel: &str, size: u64) -> ScanEntry {
    ScanEntry {
        source: PathBuf::from("/src").join(rel),
        rel: PathBuf::from(rel),
        size,
        ext: None,
        mtime: None,
    }
}

fn fs_with(free_bytes: u64) -> FsInfo {
    FsInfo {
        kind: FsKind::Apfs,
        case_sensitive: false,
        free_bytes,
        max_file_size: None,
        name_limit: NameLimit::Utf16Units(255),
        device: 1,
    }
}

/// 0: new.mov (no conflict), 1: same.mov (identical), 2: clip.mov (differs),
/// 3: bad.mov (pre-flight problem)
fn setup(dest: &Path) -> (Selection, Preflight) {
    let sel = Selection {
        files: vec![
            entry("new.mov", 100),
            entry("same.mov", 200),
            entry("clip.mov", 300),
            entry("bad.mov", 400),
        ],
        dirs: vec![],
        total_bytes: 1000,
    };
    let pf = Preflight {
        dest: dest.to_path_buf(),
        fs: fs_with(u64::MAX),
        file_problems: vec![FileProblem {
            id: 3,
            kind: ProblemKind::NameClash,
        }],
        conflicts: vec![
            Conflict {
                id: 1,
                kind: ConflictKind::Identical,
            },
            Conflict {
                id: 2,
                kind: ConflictKind::Differs {
                    size: 1,
                    mtime: None,
                },
            },
        ],
        stale_partials: vec![],
        checksum_omissions: vec![],
        source_roots: vec![],
    };
    (sel, pf)
}

fn actions(plan: &Plan) -> Vec<Action> {
    plan.files.iter().map(|f| f.action.clone()).collect()
}

#[test]
fn each_policy_only_changes_files_that_differ() {
    let dir = tempfile::tempdir().unwrap();
    let (sel, pf) = setup(dir.path());
    let fail = Action::Fail(FileError::NameClash);
    let cases = [
        (
            DiffersPolicy::KeepBoth,
            Action::KeepBoth {
                rel: PathBuf::from("clip (1).mov"),
            },
        ),
        (DiffersPolicy::Overwrite, Action::Overwrite),
        (DiffersPolicy::Skip, Action::SkipDiffers),
    ];
    for (policy, differs) in cases {
        let plan = Plan::resolve(&sel, &pf, policy);
        assert_eq!(
            actions(&plan),
            vec![Action::Copy, Action::SkipIdentical, differs, fail.clone()],
            "{policy:?}"
        );
    }
}

#[test]
fn keep_both_is_the_default() {
    assert_eq!(DiffersPolicy::default(), DiffersPolicy::KeepBoth);
}

#[test]
fn keep_both_skips_names_taken_on_disk_and_in_the_plan() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("clip (1).mov"), b"x").unwrap();
    let (mut sel, mut pf) = setup(dir.path());
    // A second file named "clip (2).mov" is already part of this copy.
    sel.files.push(entry("clip (2).mov", 10));
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(
        plan.files[2].action,
        Action::KeepBoth {
            rel: PathBuf::from("clip (3).mov")
        }
    );
    assert_eq!(plan.files[2].final_rel(), Path::new("clip (3).mov"));
    // Case-insensitive destinations treat "CLIP (3).MOV" as taken too.
    pf.fs.case_sensitive = false;
    sel.files.push(entry("CLIP (3).MOV", 10));
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(
        plan.files[2].action,
        Action::KeepBoth {
            rel: PathBuf::from("clip (4).mov")
        }
    );
}

#[test]
fn numbered_names_keep_the_extension_and_folder() {
    assert_eq!(
        numbered(Path::new("clips/A001.mov"), 1),
        Path::new("clips/A001 (1).mov")
    );
    assert_eq!(numbered(Path::new("README"), 2), Path::new("README (2)"));
    assert_eq!(
        numbered(Path::new("backup.tar.gz"), 1),
        Path::new("backup.tar (1).gz")
    );
}

#[test]
fn free_space_counts_only_written_files_plus_a_margin() {
    let dir = tempfile::tempdir().unwrap();
    let (sel, mut pf) = setup(dir.path());
    // Keep both writes new.mov and clip.mov: 400 bytes.
    let written = 400;
    let needed = written + space_margin(written);
    pf.fs.free_bytes = needed;
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(plan.bytes_to_write(), written);
    assert!(plan.blockers().is_empty());

    pf.fs.free_bytes = needed - 1;
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(
        plan.blockers(),
        vec![Blocker::NotEnoughSpace {
            needed,
            free: needed - 1
        }]
    );
    // Skipping the file that differs needs less.
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::Skip);
    assert!(plan.blockers().is_empty());
}

#[test]
fn the_margin_is_one_percent_but_at_least_64_mib() {
    assert_eq!(space_margin(0), 64 << 20);
    assert_eq!(space_margin(100 << 30), (100 << 30) / 100);
}

#[test]
fn names_the_checksum_file_cant_hold_are_marked() {
    let dir = tempfile::tempdir().unwrap();
    let (sel, mut pf) = setup(dir.path());
    pf.checksum_omissions = vec![0];
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert!(!plan.files[0].in_checksum_file);
    assert!(plan.files[1].in_checksum_file);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test plan`
Expected: compile errors: module `plan` doesn't exist.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/error.rs`:

```diff
diff --git a/crates/secopy-core/src/error.rs b/crates/secopy-core/src/error.rs
index 89da2d9..a8ecbfc 100644
--- a/crates/secopy-core/src/error.rs
+++ b/crates/secopy-core/src/error.rs
@@ -1,7 +1,10 @@
 //! Per-file errors (FR-21).
 
+use std::path::PathBuf;
 use std::{fmt, io};
 
+use crate::names::NameProblem;
+
 /// An I/O error reduced to plain data, so outcomes can be cloned, compared and sent to the UI.
 #[derive(Debug, Clone, PartialEq, Eq)]
 pub struct IoFailure {
@@ -41,6 +44,12 @@ pub enum FileError {
     NameClash,
     #[error("another copy is writing this file")]
     PartialInUse,
+    #[error("{0}")]
+    InvalidName(NameProblem),
+    #[error("the file is larger than the destination drive allows ({limit} bytes)")]
+    TooLarge { limit: u64 },
+    #[error("something is in the way at {}", path.display())]
+    InTheWay { path: PathBuf },
     #[error("cancelled")]
     Cancelled,
 }
```

Change `crates/secopy-core/src/lib.rs`:

```diff
diff --git a/crates/secopy-core/src/lib.rs b/crates/secopy-core/src/lib.rs
index 01627f8..f74b30e 100644
--- a/crates/secopy-core/src/lib.rs
+++ b/crates/secopy-core/src/lib.rs
@@ -12,6 +12,7 @@ pub mod hidden;
 pub mod job;
 pub mod names;
 mod os;
+pub mod plan;
 pub mod preflight;
 pub mod scan;
 pub mod source;
```

Create `crates/secopy-core/src/plan.rs`:

```rust
//! Resolves one action per file from the pre-flight and the user's choice for files that
//! differ (FR-16, FR-17).

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::FileError;
use crate::fsinfo::FsInfo;
use crate::preflight::{Blocker, ConflictKind, Preflight, SourceRoot, clash_key};
use crate::scan::{DirEntry, ScanEntry, Selection};

/// What to do with files that exist at the destination but differ (FR-17).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DiffersPolicy {
    /// Copy under a new name: `name (1).ext`.
    #[default]
    KeepBoth,
    /// Replace the old file once the new copy is complete (and verified).
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Copy,
    Overwrite,
    /// Copy to this path instead, relative to the destination.
    KeepBoth {
        rel: PathBuf,
    },
    /// Already at the destination: same size and modification time. Not read.
    SkipIdentical,
    SkipDiffers,
    /// A pre-flight problem; the file fails without being read.
    Fail(FileError),
}

impl Action {
    /// True if the job writes this file.
    pub fn writes(&self) -> bool {
        matches!(
            self,
            Action::Copy | Action::Overwrite | Action::KeepBoth { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub entry: ScanEntry,
    pub action: Action,
    /// False if the name can't be written to the checksum file (FR-31).
    pub in_checksum_file: bool,
}

impl PlannedFile {
    /// Where the copy lands, relative to the destination.
    pub fn final_rel(&self) -> &Path {
        match &self.action {
            Action::KeepBoth { rel } => rel,
            _ => &self.entry.rel,
        }
    }
}

/// Everything a job needs, resolved before it starts.
#[derive(Debug, Clone)]
pub struct Plan {
    pub dest: PathBuf,
    /// Same order as `Selection::files`, so ids stay stable.
    pub files: Vec<PlannedFile>,
    pub dirs: Vec<DirEntry>,
    pub fs: FsInfo,
    pub source_roots: Vec<SourceRoot>,
    pub stale_partials: Vec<PathBuf>,
}

/// Free space kept on top of the bytes to write: 1 % of them, at least 64 MiB.
pub fn space_margin(bytes: u64) -> u64 {
    (bytes / 100).max(64 << 20)
}

impl Plan {
    pub fn resolve(sel: &Selection, pf: &Preflight, policy: DiffersPolicy) -> Plan {
        let problems: HashMap<usize, FileError> = pf
            .file_problems
            .iter()
            .map(|p| (p.id, p.kind.to_error()))
            .collect();
        let conflicts: HashMap<usize, &ConflictKind> =
            pf.conflicts.iter().map(|c| (c.id, &c.kind)).collect();
        let omitted: HashSet<usize> = pf.checksum_omissions.iter().copied().collect();
        let case_sensitive = pf.fs.case_sensitive;
        let mut taken: HashSet<Vec<u8>> = sel
            .files
            .iter()
            .map(|f| clash_key(&f.rel, case_sensitive))
            .collect();
        let files = sel
            .files
            .iter()
            .enumerate()
            .map(|(id, entry)| {
                let action = if let Some(e) = problems.get(&id) {
                    Action::Fail(e.clone())
                } else {
                    match (conflicts.get(&id), policy) {
                        (None, _) => Action::Copy,
                        (Some(ConflictKind::Identical), _) => Action::SkipIdentical,
                        (Some(ConflictKind::Differs { .. }), DiffersPolicy::Skip) => {
                            Action::SkipDiffers
                        }
                        (Some(ConflictKind::Differs { .. }), DiffersPolicy::Overwrite) => {
                            Action::Overwrite
                        }
                        (Some(ConflictKind::Differs { .. }), DiffersPolicy::KeepBoth) => {
                            Action::KeepBoth {
                                rel: free_name(&pf.dest, &entry.rel, &mut taken, case_sensitive),
                            }
                        }
                    }
                };
                PlannedFile {
                    entry: entry.clone(),
                    action,
                    in_checksum_file: !omitted.contains(&id),
                }
            })
            .collect();
        Plan {
            dest: pf.dest.clone(),
            files,
            dirs: sel.dirs.clone(),
            fs: pf.fs.clone(),
            source_roots: pf.source_roots.clone(),
            stale_partials: pf.stale_partials.clone(),
        }
    }

    /// Bytes the job will write.
    pub fn bytes_to_write(&self) -> u64 {
        self.files
            .iter()
            .filter(|f| f.action.writes())
            .map(|f| f.entry.size)
            .sum()
    }

    /// Bytes of every file in the plan, written or not.
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.entry.size).sum()
    }

    /// Problems that stop the job. Empty means Start can be enabled (FR-16).
    pub fn blockers(&self) -> Vec<Blocker> {
        let bytes = self.bytes_to_write();
        let needed = bytes.saturating_add(space_margin(bytes));
        if bytes > 0 && needed > self.fs.free_bytes {
            vec![Blocker::NotEnoughSpace {
                needed,
                free: self.fs.free_bytes,
            }]
        } else {
            vec![]
        }
    }
}

/// `name (1).ext`, `name (2).ext`, …: the first name that is free on disk and not used by
/// another file in the plan.
fn free_name(
    dest: &Path,
    rel: &Path,
    taken: &mut HashSet<Vec<u8>>,
    case_sensitive: bool,
) -> PathBuf {
    (1u32..)
        .map(|n| numbered(rel, n))
        .find(|candidate| {
            let key = clash_key(candidate, case_sensitive);
            let free = !taken.contains(&key) && fs::symlink_metadata(dest.join(candidate)).is_err();
            if free {
                taken.insert(key);
            }
            free
        })
        .expect("some numbered name is free")
}

/// `clips/A001.mov` → `clips/A001 (n).mov`
pub fn numbered(rel: &Path, n: u32) -> PathBuf {
    let stem = rel.file_stem().unwrap_or_default();
    let mut name = OsString::from(stem);
    name.push(format!(" ({n})"));
    if let Some(ext) = rel.extension() {
        name.push(".");
        name.push(ext);
    }
    rel.with_file_name(name)
}
```

Change `crates/secopy-core/src/preflight.rs`:

```diff
diff --git a/crates/secopy-core/src/preflight.rs b/crates/secopy-core/src/preflight.rs
index e79c5a4..ccd5270 100644
--- a/crates/secopy-core/src/preflight.rs
+++ b/crates/secopy-core/src/preflight.rs
@@ -8,7 +8,7 @@ use std::path::{Path, PathBuf};
 use std::time::{Duration, SystemTime};
 
 use crate::copy::partial_path;
-use crate::error::IoFailure;
+use crate::error::{FileError, IoFailure};
 use crate::fsinfo::{self, FsInfo};
 use crate::names::{self, NameProblem};
 use crate::scan::{ScanEntry, Selection};
@@ -62,6 +62,18 @@ pub enum ProblemKind {
     },
 }
 
+impl ProblemKind {
+    /// How the file fails if the job starts anyway.
+    pub fn to_error(&self) -> FileError {
+        match self {
+            ProblemKind::InvalidName(p) => FileError::InvalidName(p.clone()),
+            ProblemKind::TooLarge { limit } => FileError::TooLarge { limit: *limit },
+            ProblemKind::NameClash => FileError::NameClash,
+            ProblemKind::InTheWay { path } => FileError::InTheWay { path: path.clone() },
+        }
+    }
+}
+
 /// A file that already exists at the destination (FR-17).
 #[derive(Debug, Clone, PartialEq, Eq)]
 pub struct Conflict {
@@ -228,7 +240,7 @@ fn conflict_kind(entry: &ScanEntry, existing: &Metadata) -> ConflictKind {
 /// Paths that land on the same destination file. Compared on raw bytes, not lossy UTF-8,
 /// and ignoring case only where the file system does. Unicode normalization (NFC vs NFD
 /// on APFS) isn't predicted; the no-replace commit catches it.
-fn clash_key(rel: &Path, case_sensitive: bool) -> Vec<u8> {
+pub(crate) fn clash_key(rel: &Path, case_sensitive: bool) -> Vec<u8> {
     let bytes = rel.as_os_str().as_encoded_bytes();
     if case_sensitive {
         return bytes.to_vec();
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): resolve a plan with one action per file

One action per file from the pre-flight and the user's choice for files
that differ: Keep both (default), Overwrite or Skip. Identical files are
always skipped (FR-17). blockers() adds the free-space check (FR-16).
EOF
```


---

### Task 9: Commit modes: replace, keep both

`PartialCopy::commit` takes a `Commit`: `NoReplace` (as before), `Replace` (atomic
replacing rename; on Windows a read-only target is made writable first), or
`KeepBoth { original, n }`. If the planned `name (n)` was taken since pre-flight, it tries
`(n+1)`, `(n+2)`, …. It returns where the file landed. `Action::KeepBoth` carries `n`, and
`numbered` moves to `names` so `copy` can use it. The job keeps committing with
`NoReplace` until Task 10.

**Files:**
- Modify: `crates/secopy-core/src/copy.rs`, `crates/secopy-core/src/job/runner.rs`, `crates/secopy-core/src/names.rs`, `crates/secopy-core/src/plan.rs`, `crates/secopy-core/tests/copy.rs`, `crates/secopy-core/tests/plan.rs`

**Interfaces:**
- Consumes: Task 6's `PartialCopy`, Task 8's `numbered`.
- Produces: `copy::Commit::{NoReplace, Replace, KeepBoth { original: &Path, n: u32 }}`;
  `PartialCopy::commit(self, final_path, how: Commit) -> Result<PathBuf, FileError>`;
  `Action::KeepBoth { rel: PathBuf, n: u32 }`; `names::numbered(&Path, u32) -> PathBuf`.

- [ ] **Step 1: Write the failing tests**

Update the tests in `crates/secopy-core/tests/copy.rs`:

```diff
diff --git a/crates/secopy-core/tests/copy.rs b/crates/secopy-core/tests/copy.rs
index 48a2245..8790264 100644
--- a/crates/secopy-core/tests/copy.rs
+++ b/crates/secopy-core/tests/copy.rs
@@ -5,7 +5,7 @@ use std::fs;
 
 use common::pattern;
 use secopy_core::control::JobControl;
-use secopy_core::copy::{CopyConfig, copy_to_partial, partial_path};
+use secopy_core::copy::{Commit, CopyConfig, copy_to_partial, partial_path};
 use secopy_core::error::FileError;
 use secopy_core::hash::hash_bytes;
 
@@ -42,7 +42,7 @@ fn small_file_is_copied_hashed_and_committed() {
     assert!(!dst.exists(), "final name only appears on commit");
 
     let partial = pc.partial.clone();
-    pc.commit(&dst).unwrap();
+    pc.commit(&dst, Commit::NoReplace).unwrap();
     assert_eq!(fs::read(&dst).unwrap(), b"hello secopy");
     assert!(!partial.exists());
 }
@@ -174,7 +174,7 @@ fn a_live_writers_partial_file_is_never_touched() {
         );
         paused.resume();
         let pc = first.join().unwrap().unwrap();
-        pc.commit(&dst).unwrap();
+        pc.commit(&dst, Commit::NoReplace).unwrap();
     });
     assert_eq!(fs::read(&dst).unwrap(), b"mine");
 }
@@ -224,10 +224,63 @@ fn long_names_get_a_short_partial_name() {
         &JobControl::new(),
     )
     .unwrap();
-    pc.commit(&dst).unwrap();
+    pc.commit(&dst, Commit::NoReplace).unwrap();
     assert_eq!(fs::read(&dst).unwrap(), b"clip");
 }
 
+#[test]
+fn replace_swaps_in_the_new_file() {
+    let dir = tempfile::tempdir().unwrap();
+    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
+    fs::write(&src, b"new").unwrap();
+    fs::write(&dst, b"old").unwrap();
+    let mut perms = fs::metadata(&dst).unwrap().permissions();
+    perms.set_readonly(true);
+    fs::set_permissions(&dst, perms).unwrap();
+    let pc = copy_to_partial(
+        &src,
+        &dst,
+        &CopyConfig::default(),
+        &|_| {},
+        &JobControl::new(),
+    )
+    .unwrap();
+    assert_eq!(pc.commit(&dst, Commit::Replace), Ok(dst.clone()));
+    assert_eq!(fs::read(&dst).unwrap(), b"new");
+}
+
+#[test]
+fn keep_both_moves_on_to_the_next_free_number() {
+    let dir = tempfile::tempdir().unwrap();
+    let src = dir.path().join("a.bin");
+    let original = dir.path().join("clip.mov");
+    let planned = dir.path().join("clip (1).mov");
+    fs::write(&src, b"new").unwrap();
+    // Someone took the planned name, and the next one, after pre-flight.
+    fs::write(&planned, b"theirs").unwrap();
+    fs::write(dir.path().join("clip (2).mov"), b"theirs too").unwrap();
+    let pc = copy_to_partial(
+        &src,
+        &planned,
+        &CopyConfig::default(),
+        &|_| {},
+        &JobControl::new(),
+    )
+    .unwrap();
+    let got = pc
+        .commit(
+            &planned,
+            Commit::KeepBoth {
+                original: &original,
+                n: 1,
+            },
+        )
+        .unwrap();
+    assert_eq!(got, dir.path().join("clip (3).mov"));
+    assert_eq!(fs::read(&got).unwrap(), b"new");
+    assert_eq!(fs::read(&planned).unwrap(), b"theirs");
+}
+
 #[test]
 fn commit_never_replaces_an_existing_file() {
     let dir = tempfile::tempdir().unwrap();
@@ -244,7 +297,10 @@ fn commit_never_replaces_an_existing_file() {
     fs::write(&dst, b"mine").unwrap();
 
     let partial = pc.partial.clone();
-    assert_eq!(pc.commit(&dst), Err(FileError::AlreadyExists));
+    assert_eq!(
+        pc.commit(&dst, Commit::NoReplace),
+        Err(FileError::AlreadyExists)
+    );
     assert_eq!(fs::read(&dst).unwrap(), b"mine");
     assert!(
         !partial.exists(),
```

Update the tests in `crates/secopy-core/tests/plan.rs`:

```diff
diff --git a/crates/secopy-core/tests/plan.rs b/crates/secopy-core/tests/plan.rs
index 1df2ee9..07a0ec2 100644
--- a/crates/secopy-core/tests/plan.rs
+++ b/crates/secopy-core/tests/plan.rs
@@ -3,7 +3,8 @@ use std::path::{Path, PathBuf};
 
 use secopy_core::error::FileError;
 use secopy_core::fsinfo::{FsInfo, FsKind, NameLimit};
-use secopy_core::plan::{Action, DiffersPolicy, Plan, numbered, space_margin};
+use secopy_core::names::numbered;
+use secopy_core::plan::{Action, DiffersPolicy, Plan, space_margin};
 use secopy_core::preflight::{
     Blocker, Conflict, ConflictKind, FileProblem, Preflight, ProblemKind,
 };
@@ -84,6 +85,7 @@ fn each_policy_only_changes_files_that_differ() {
             DiffersPolicy::KeepBoth,
             Action::KeepBoth {
                 rel: PathBuf::from("clip (1).mov"),
+                n: 1,
             },
         ),
         (DiffersPolicy::Overwrite, Action::Overwrite),
@@ -115,7 +117,8 @@ fn keep_both_skips_names_taken_on_disk_and_in_the_plan() {
     assert_eq!(
         plan.files[2].action,
         Action::KeepBoth {
-            rel: PathBuf::from("clip (3).mov")
+            rel: PathBuf::from("clip (3).mov"),
+            n: 3,
         }
     );
     assert_eq!(plan.files[2].final_rel(), Path::new("clip (3).mov"));
@@ -126,7 +129,8 @@ fn keep_both_skips_names_taken_on_disk_and_in_the_plan() {
     assert_eq!(
         plan.files[2].action,
         Action::KeepBoth {
-            rel: PathBuf::from("clip (4).mov")
+            rel: PathBuf::from("clip (4).mov"),
+            n: 4,
         }
     );
 }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test copy --test plan`
Expected: compile errors: `Commit` doesn't exist; `KeepBoth` has no field `n`.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/copy.rs`:

```diff
diff --git a/crates/secopy-core/src/copy.rs b/crates/secopy-core/src/copy.rs
index fd2deac..f39742c 100644
--- a/crates/secopy-core/src/copy.rs
+++ b/crates/secopy-core/src/copy.rs
@@ -45,13 +45,36 @@ pub struct PartialCopy {
     file: File,
 }
 
+/// How a finished copy takes its final name (FR-17, FR-18).
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum Commit<'a> {
+    /// Never replace an existing file.
+    NoReplace,
+    /// Replace an existing file, atomically.
+    Replace,
+    /// Like `NoReplace`; if the name was taken since pre-flight, try `original (n+1)`,
+    /// `original (n+2)`, … instead.
+    KeepBoth { original: &'a Path, n: u32 },
+}
+
+/// Names tried when a Keep-both name is taken at commit time.
+const KEEP_BOTH_TRIES: u32 = 100;
+
 impl PartialCopy {
-    /// Gives the file its final name without ever replacing an existing file (FR-18). The
-    /// file system itself decides whether the name is taken (case, Unicode normalization).
-    /// On failure the partial file is removed.
-    pub fn commit(self, final_path: &Path) -> Result<(), FileError> {
+    /// Gives the file its final name and returns the path it got. The file system itself
+    /// decides whether a name is taken (case, Unicode normalization). On failure the
+    /// partial file is removed.
+    pub fn commit(self, final_path: &Path, how: Commit) -> Result<PathBuf, FileError> {
         self.release(|partial| {
-            let result = commit_noreplace(partial, final_path);
+            let result = match how {
+                Commit::NoReplace => {
+                    commit_noreplace(partial, final_path).map(|()| final_path.to_path_buf())
+                }
+                Commit::Replace => commit_replace(partial, final_path),
+                Commit::KeepBoth { original, n } => {
+                    commit_keep_both(partial, final_path, original, n)
+                }
+            };
             if result.is_err() {
                 let _ = fs::remove_file(partial);
             }
@@ -156,6 +179,37 @@ fn create_partial(partial: &Path) -> Result<(File, bool), FileError> {
     Err(FileError::PartialInUse)
 }
 
+fn commit_replace(partial: &Path, final_path: &Path) -> Result<PathBuf, FileError> {
+    // Windows refuses to replace a read-only file.
+    #[cfg(windows)]
+    if let Ok(meta) = fs::metadata(final_path)
+        && meta.permissions().readonly()
+    {
+        let mut perms = meta.permissions();
+        #[allow(clippy::permissions_set_readonly_false)] // clears the Windows attribute only
+        perms.set_readonly(false);
+        fs::set_permissions(final_path, perms).map_err(FileError::write_dest)?;
+    }
+    fs::rename(partial, final_path).map_err(FileError::write_dest)?;
+    Ok(final_path.to_path_buf())
+}
+
+fn commit_keep_both(
+    partial: &Path,
+    planned: &Path,
+    original: &Path,
+    n: u32,
+) -> Result<PathBuf, FileError> {
+    let later = (n + 1..n + KEEP_BOTH_TRIES).map(|m| crate::names::numbered(original, m));
+    for candidate in std::iter::once(planned.to_path_buf()).chain(later) {
+        match commit_noreplace(partial, &candidate) {
+            Err(FileError::AlreadyExists) => continue,
+            other => return other.map(|()| candidate),
+        }
+    }
+    Err(FileError::AlreadyExists)
+}
+
 /// Uses the OS's no-replace rename. File systems without one (FAT and exFAT on macOS and
 /// Linux) fall back to a hard link, and then to check-then-rename.
 fn commit_noreplace(partial: &Path, final_path: &Path) -> Result<(), FileError> {
```

Change `crates/secopy-core/src/job/runner.rs`:

```diff
diff --git a/crates/secopy-core/src/job/runner.rs b/crates/secopy-core/src/job/runner.rs
index d4b21d6..341c36a 100644
--- a/crates/secopy-core/src/job/runner.rs
+++ b/crates/secopy-core/src/job/runner.rs
@@ -8,7 +8,7 @@ use std::time::Instant;
 
 use super::progress::{Phase, Slot};
 use super::{Event, FileOutcome, FileStatus, JobControl, JobOptions};
-use crate::copy::{self, CopyConfig, PartialCopy};
+use crate::copy::{self, Commit, CopyConfig, PartialCopy};
 use crate::error::FileError;
 use crate::hash;
 use crate::scan::{ScanEntry, Selection};
@@ -146,8 +146,8 @@ impl<'a> Runner<'a> {
             return;
         }
         let hash = partial.hash;
-        match partial.commit(&final_path) {
-            Ok(()) => self.finish(&slot, entry, Some(hash), FileStatus::Copied, started),
+        match partial.commit(&final_path, Commit::NoReplace) {
+            Ok(_) => self.finish(&slot, entry, Some(hash), FileStatus::Copied, started),
             Err(e) => self.finish(&slot, entry, Some(hash), FileStatus::Failed(e), started),
         }
     }
@@ -216,7 +216,9 @@ impl<'a> Runner<'a> {
             }
             let expected = partial.hash;
             if actual == expected {
-                break partial.commit(&final_path).map(|()| expected);
+                break partial
+                    .commit(&final_path, Commit::NoReplace)
+                    .map(|_| expected);
             }
             partial.discard();
             if attempt == 1 {
```

Change `crates/secopy-core/src/names.rs`:

```diff
diff --git a/crates/secopy-core/src/names.rs b/crates/secopy-core/src/names.rs
index a2f0727..32ee052 100644
--- a/crates/secopy-core/src/names.rs
+++ b/crates/secopy-core/src/names.rs
@@ -1,8 +1,8 @@
 //! File names the destination file system can't store (FR-16).
 
-use std::ffi::OsStr;
+use std::ffi::{OsStr, OsString};
 use std::fmt;
-use std::path::{Component, Path};
+use std::path::{Component, Path, PathBuf};
 
 use crate::fsinfo::{FsInfo, NameLimit};
 
@@ -104,6 +104,18 @@ fn too_long(name: &OsStr, text: &str, limit: NameLimit) -> bool {
     }
 }
 
+/// `clips/A001.mov` → `clips/A001 (n).mov`
+pub fn numbered(rel: &Path, n: u32) -> PathBuf {
+    let stem = rel.file_stem().unwrap_or_default();
+    let mut name = OsString::from(stem);
+    name.push(format!(" ({n})"));
+    if let Some(ext) = rel.extension() {
+        name.push(".");
+        name.push(ext);
+    }
+    rel.with_file_name(name)
+}
+
 #[cfg(test)]
 mod tests {
     use super::*;
```

Change `crates/secopy-core/src/plan.rs`:

```diff
diff --git a/crates/secopy-core/src/plan.rs b/crates/secopy-core/src/plan.rs
index e22eb41..81c6549 100644
--- a/crates/secopy-core/src/plan.rs
+++ b/crates/secopy-core/src/plan.rs
@@ -2,12 +2,12 @@
 //! differ (FR-16, FR-17).
 
 use std::collections::{HashMap, HashSet};
-use std::ffi::OsString;
 use std::fs;
 use std::path::{Path, PathBuf};
 
 use crate::error::FileError;
 use crate::fsinfo::FsInfo;
+use crate::names::numbered;
 use crate::preflight::{Blocker, ConflictKind, Preflight, SourceRoot, clash_key};
 use crate::scan::{DirEntry, ScanEntry, Selection};
 
@@ -26,9 +26,11 @@ pub enum DiffersPolicy {
 pub enum Action {
     Copy,
     Overwrite,
-    /// Copy to this path instead, relative to the destination.
+    /// Copy to this path instead, relative to the destination: the original name
+    /// numbered `n`, as in `clip (n).mov`.
     KeepBoth {
         rel: PathBuf,
+        n: u32,
     },
     /// Already at the destination: same size and modification time. Not read.
     SkipIdentical,
@@ -59,7 +61,7 @@ impl PlannedFile {
     /// Where the copy lands, relative to the destination.
     pub fn final_rel(&self) -> &Path {
         match &self.action {
-            Action::KeepBoth { rel } => rel,
+            Action::KeepBoth { rel, .. } => rel,
             _ => &self.entry.rel,
         }
     }
@@ -116,9 +118,9 @@ impl Plan {
                             Action::Overwrite
                         }
                         (Some(ConflictKind::Differs { .. }), DiffersPolicy::KeepBoth) => {
-                            Action::KeepBoth {
-                                rel: free_name(&pf.dest, &entry.rel, &mut taken, case_sensitive),
-                            }
+                            let (rel, n) =
+                                free_name(&pf.dest, &entry.rel, &mut taken, case_sensitive);
+                            Action::KeepBoth { rel, n }
                         }
                     }
                 };
@@ -175,10 +177,10 @@ fn free_name(
     rel: &Path,
     taken: &mut HashSet<Vec<u8>>,
     case_sensitive: bool,
-) -> PathBuf {
+) -> (PathBuf, u32) {
     (1u32..)
-        .map(|n| numbered(rel, n))
-        .find(|candidate| {
+        .map(|n| (numbered(rel, n), n))
+        .find(|(candidate, _)| {
             let key = clash_key(candidate, case_sensitive);
             let free = !taken.contains(&key) && fs::symlink_metadata(dest.join(candidate)).is_err();
             if free {
@@ -188,15 +190,3 @@ fn free_name(
         })
         .expect("some numbered name is free")
 }
-
-/// `clips/A001.mov` → `clips/A001 (n).mov`
-pub fn numbered(rel: &Path, n: u32) -> PathBuf {
-    let stem = rel.file_stem().unwrap_or_default();
-    let mut name = OsString::from(stem);
-    name.push(format!(" ({n})"));
-    if let Some(ext) = rel.extension() {
-        name.push(".");
-        name.push(ext);
-    }
-    rel.with_file_name(name)
-}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

Also lint the other platforms' `cfg` code (install the targets once with
`rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`):
`cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` and
`cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`.
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): commit copies by replacing or under a numbered name

Replace swaps in the new file atomically; Keep both moves on to the next
free number if the planned name was taken after pre-flight (FR-17).
EOF
```


---

### Task 10: Run jobs from a plan

`run_job(&Plan, &JobOptions, &JobControl, on_event)`:

- Skipped and pre-flight-failed files finish at once, without I/O.
- Written files commit with the mode their action asks for.
- Parent folders are created when their first file starts, so a cancel or failure leaves no
  empty folders (FR-10, carry-over). Empty source folders are created at the end.
- The checksum file lists copies under their final names, without names that aren't UTF-8.
- Name clashes now come from pre-flight, so `find_name_clashes` goes.
- `small_file_threshold` defaults to the 4 MiB buffer size (memory carry-over).
- The CLI keeps working with Keep both until Task 17.

Existing tests move from `Selection` to `Plan`. Two change meaning:
`an_existing_destination_file_is_never_overwritten` becomes "a file that appears after
pre-flight is never overwritten", and the name-clash test expects both files on
case-sensitive drives.

**Files:**
- Modify: `crates/secopy-cli/src/main.rs`, `crates/secopy-cli/tests/cli.rs`, `crates/secopy-core/src/job/mod.rs`, `crates/secopy-core/src/job/progress.rs`, `crates/secopy-core/src/job/runner.rs`, `crates/secopy-core/tests/job.rs`

**Interfaces:**
- Consumes: Tasks 7–9.
- Produces: `run_job(plan: &Plan, opts, control, on_event) -> JobReport`;
  `FileStatus::Skipped(SkipReason::{Identical, Differs})`;
  `FileOutcome { id, rel, final_rel, size, hash, status, in_checksum_file, elapsed }`;
  `JobReport::skipped()`; `Progress::files_skipped`, and `Progress::total_bytes` = bytes to write.

- [ ] **Step 1: Write the failing tests**

Update the tests in `crates/secopy-cli/tests/cli.rs`:

```diff
diff --git a/crates/secopy-cli/tests/cli.rs b/crates/secopy-cli/tests/cli.rs
index e0d68e0..19297ee 100644
--- a/crates/secopy-cli/tests/cli.rs
+++ b/crates/secopy-cli/tests/cli.rs
@@ -72,20 +72,32 @@ fn mixing_a_folder_and_files_is_a_usage_error() {
     assert_eq!(out.status.code(), Some(2));
 }
 
+#[cfg(unix)]
 #[test]
 fn a_failed_file_gives_exit_code_one() {
+    use std::os::unix::fs::PermissionsExt;
     let dir = tempfile::tempdir().unwrap();
     let dest = dir.path().join("dest");
     fs::create_dir_all(&dest).unwrap();
-    fs::write(dir.path().join("a.wav"), b"new").unwrap();
-    fs::write(dest.join("a.wav"), b"old").unwrap();
+    let locked = dir.path().join("a.wav");
+    fs::write(&locked, b"new").unwrap();
+    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
+    if fs::read(&locked).is_ok() {
+        return; // running as root: permissions are not enforced
+    }
+    let out = cli().arg(&locked).arg("--to").arg(&dest).output().unwrap();
+    assert_eq!(out.status.code(), Some(1));
+}
 
+#[test]
+fn a_missing_destination_is_a_usage_error() {
+    let dir = tempfile::tempdir().unwrap();
+    fs::write(dir.path().join("a.wav"), b"a").unwrap();
     let out = cli()
         .arg(dir.path().join("a.wav"))
         .arg("--to")
-        .arg(&dest)
+        .arg(dir.path().join("nope"))
         .output()
         .unwrap();
-    assert_eq!(out.status.code(), Some(1));
-    assert_eq!(fs::read(dest.join("a.wav")).unwrap(), b"old");
+    assert_eq!(out.status.code(), Some(2));
 }
```

Update the tests in `crates/secopy-core/tests/job.rs`:

```diff
diff --git a/crates/secopy-core/tests/job.rs b/crates/secopy-core/tests/job.rs
index 582688b..f6f5932 100644
--- a/crates/secopy-core/tests/job.rs
+++ b/crates/secopy-core/tests/job.rs
@@ -10,9 +10,11 @@ use secopy_core::error::FileError;
 use secopy_core::filter::ExtensionFilter;
 use secopy_core::hash::{hash_bytes, to_hex};
 use secopy_core::job::{
-    Event, FileStatus, Hooks, JobControl, JobOptions, JobReport, Progress, run_job,
+    Event, FileStatus, Hooks, JobControl, JobOptions, JobReport, Progress, SkipReason, run_job,
 };
-use secopy_core::scan::{ScanOptions, Selection, scan};
+use secopy_core::plan::{Action, DiffersPolicy, Plan};
+use secopy_core::preflight::preflight;
+use secopy_core::scan::{ScanOptions, scan};
 use secopy_core::source::{DirMode, Source};
 
 struct Fixture {
@@ -44,14 +46,26 @@ fn fixture() -> Fixture {
     }
 }
 
-fn select(src: &Path) -> Selection {
+/// Scan, pre-flight and resolve with `policy` for files that differ.
+fn plan_with(source: &Source, dest: &Path, policy: DiffersPolicy) -> Plan {
+    let sel = scan(source, &ScanOptions::default())
+        .unwrap()
+        .select(&ExtensionFilter::All);
+    let pf = preflight(source, &sel, dest).unwrap();
+    Plan::resolve(&sel, &pf, policy)
+}
+
+fn plan_of(source: &Source, dest: &Path) -> Plan {
+    plan_with(source, dest, DiffersPolicy::KeepBoth)
+}
+
+/// Copies the folder `src` itself into `dest`.
+fn plan(src: &Path, dest: &Path) -> Plan {
     let source = Source::Directory {
         path: src.to_path_buf(),
         mode: DirMode::FolderItself,
     };
-    scan(&source, &ScanOptions::default())
-        .unwrap()
-        .select(&ExtensionFilter::All)
+    plan_of(&source, dest)
 }
 
 /// Small buffers and a low threshold so both lanes and the pipeline are exercised.
@@ -70,9 +84,9 @@ fn opts(verify: bool) -> JobOptions {
     }
 }
 
-fn run(sel: &Selection, dest: &Path, opts: &JobOptions) -> (JobReport, Vec<Event>) {
+fn run(plan: &Plan, opts: &JobOptions) -> (JobReport, Vec<Event>) {
     let events = Mutex::new(Vec::new());
-    let report = run_job(sel, dest, opts, &JobControl::new(), &|e| {
+    let report = run_job(plan, opts, &JobControl::new(), &|e| {
         events.lock().unwrap().push(e)
     });
     (report, events.into_inner().unwrap())
@@ -85,8 +99,7 @@ fn expected_tree(src: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
 #[test]
 fn copy_mode_copies_everything_and_writes_the_checksum_file() {
     let f = fixture();
-    let sel = select(&f.src);
-    let (report, _) = run(&sel, &f.dest, &opts(false));
+    let (report, _) = run(&plan(&f.src, &f.dest), &opts(false));
 
     assert!(report.is_success(), "{report:?}");
     assert!(
@@ -107,7 +120,7 @@ fn copy_mode_copies_everything_and_writes_the_checksum_file() {
 #[test]
 fn verify_mode_marks_files_verified() {
     let f = fixture();
-    let (report, _) = run(&select(&f.src), &f.dest, &opts(true));
+    let (report, _) = run(&plan(&f.src, &f.dest), &opts(true));
     assert!(report.is_success(), "{report:?}");
     assert!(
         report
@@ -123,7 +136,7 @@ fn verify_mode_marks_files_verified() {
 fn empty_source_folders_are_recreated() {
     let f = fixture();
     fs::create_dir_all(f.src.join("EMPTY")).unwrap();
-    run(&select(&f.src), &f.dest, &opts(false));
+    run(&plan(&f.src, &f.dest), &opts(false));
     assert!(f.dest.join("CARD/EMPTY").is_dir());
 }
 
@@ -144,7 +157,7 @@ fn a_corrupted_copy_is_recopied_once_and_then_verifies() {
             }
         }),
     };
-    let (report, _) = run(&select(&f.src), &f.dest, &o);
+    let (report, _) = run(&plan(&f.src, &f.dest), &o);
     assert!(report.is_success(), "{report:?}");
     assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
 }
@@ -160,7 +173,7 @@ fn a_copy_that_stays_corrupted_fails_and_leaves_no_file() {
             }
         }),
     };
-    let (report, _) = run(&select(&f.src), &f.dest, &o);
+    let (report, _) = run(&plan(&f.src, &f.dest), &o);
 
     let failed: Vec<_> = report.failed().collect();
     assert_eq!(failed.len(), 1);
@@ -182,10 +195,11 @@ fn a_copy_that_stays_corrupted_fails_and_leaves_no_file() {
 }
 
 #[test]
-fn an_existing_destination_file_is_never_overwritten() {
+fn a_file_that_appears_after_preflight_is_never_overwritten() {
     let f = fixture();
+    let plan = plan(&f.src, &f.dest);
     write_files(&f.dest, &[("CARD/notes.txt", b"mine")]);
-    let (report, _) = run(&select(&f.src), &f.dest, &opts(false));
+    let (report, _) = run(&plan, &opts(false));
 
     let failed: Vec<_> = report.failed().collect();
     assert_eq!(failed.len(), 1);
@@ -199,14 +213,18 @@ fn an_existing_destination_file_is_never_overwritten() {
 #[test]
 fn cancelling_before_start_copies_nothing() {
     let f = fixture();
-    let sel = select(&f.src);
+    let plan = plan(&f.src, &f.dest);
     let control = JobControl::new();
     control.cancel();
-    let report = run_job(&sel, &f.dest, &opts(true), &control, &|_| {});
+    let report = run_job(&plan, &opts(true), &control, &|_| {});
 
     assert!(report.cancelled);
-    assert_eq!(report.not_started, sel.files.len() as u64);
-    assert!(read_tree(&f.dest).is_empty());
+    assert_eq!(report.not_started, plan.files.len() as u64);
+    assert_eq!(
+        fs::read_dir(&f.dest).unwrap().count(),
+        0,
+        "not even empty folders are left (FR-10)"
+    );
     assert_eq!(report.checksum_file, None);
 }
 
@@ -215,7 +233,7 @@ fn checksum_file_can_be_turned_off() {
     let f = fixture();
     let mut o = opts(false);
     o.write_checksum_file = false;
-    let (report, _) = run(&select(&f.src), &f.dest, &o);
+    let (report, _) = run(&plan(&f.src, &f.dest), &o);
     assert_eq!(report.checksum_file, None);
     assert!(
         fs::read_dir(&f.dest)
@@ -227,8 +245,9 @@ fn checksum_file_can_be_turned_off() {
 #[test]
 fn final_progress_event_is_complete() {
     let f = fixture();
-    let sel = select(&f.src);
-    let (_, events) = run(&sel, &f.dest, &opts(true));
+    let plan = plan(&f.src, &f.dest);
+    let total = plan.total_bytes();
+    let (_, events) = run(&plan, &opts(true));
 
     let last = events
         .iter()
@@ -242,10 +261,11 @@ fn final_progress_event_is_complete() {
         last,
         Progress {
             total_files: 4,
-            total_bytes: sel.total_bytes,
+            total_bytes: total,
             files_done: 4,
-            copied_bytes: sel.total_bytes,
-            verified_bytes: sel.total_bytes,
+            files_skipped: 0,
+            copied_bytes: total,
+            verified_bytes: total,
             active: vec![],
             paused: false,
         }
@@ -266,7 +286,7 @@ fn many_small_files_are_all_copied() {
     for i in 0..300 {
         write_files(&src, &[(&format!("d{}/f{i}.bin", i % 7), &pattern(i))]);
     }
-    let (report, _) = run(&select(&src), &dest, &opts(true));
+    let (report, _) = run(&plan(&src, &dest), &opts(true));
     assert!(report.is_success(), "{report:?}");
     assert_eq!(read_tree(&dest.join("src")), read_tree(&src));
 }
@@ -281,7 +301,7 @@ fn an_unreadable_file_fails_alone() {
     if fs::read(&locked).is_ok() {
         return; // running as root: permissions are not enforced
     }
-    let (report, _) = run(&select(&f.src), &f.dest, &opts(true));
+    let (report, _) = run(&plan(&f.src, &f.dest), &opts(true));
 
     let failed: Vec<_> = report.failed().collect();
     assert_eq!(failed.len(), 1);
@@ -302,24 +322,30 @@ fn files_that_map_to_the_same_name_never_mix() {
         &[("x/a.txt", &pattern(500)), ("y/A.TXT", &pattern(900))],
     );
     let source = Source::Files(vec![dir.path().join("x/a.txt"), dir.path().join("y/A.TXT")]);
-    let sel = scan(&source, &ScanOptions::default())
-        .unwrap()
-        .select(&ExtensionFilter::All);
-    let (report, _) = run(&sel, &dest, &opts(false));
+    let plan = plan_of(&source, &dest);
+    let (report, _) = run(&plan, &opts(false));
 
-    let failed: Vec<_> = report.failed().collect();
-    assert_eq!(failed.len(), 1);
-    assert_eq!(failed[0].rel, Path::new("A.TXT"));
-    assert_eq!(failed[0].status, FileStatus::Failed(FileError::NameClash));
     assert_eq!(fs::read(dest.join("a.txt")).unwrap(), pattern(500));
+    if plan.fs.case_sensitive {
+        // Linux: two different files (FR-17a applies to case-insensitive drives only).
+        assert!(report.is_success(), "{report:?}");
+        assert_eq!(fs::read(dest.join("A.TXT")).unwrap(), pattern(900));
+    } else {
+        let failed: Vec<_> = report.failed().collect();
+        assert_eq!(failed.len(), 1);
+        assert_eq!(failed[0].rel, Path::new("A.TXT"));
+        assert_eq!(failed[0].status, FileStatus::Failed(FileError::NameClash));
+    }
 }
 
 #[test]
 fn an_unwritable_destination_fails_every_file_without_hanging() {
     let f = fixture();
-    let not_a_dir = f.dest.join("file");
-    fs::write(&not_a_dir, b"x").unwrap();
-    let (report, _) = run(&select(&f.src), &not_a_dir, &opts(true));
+    let plan = plan(&f.src, &f.dest);
+    // The destination turns into a file after pre-flight.
+    fs::remove_dir(&f.dest).unwrap();
+    fs::write(&f.dest, b"x").unwrap();
+    let (report, _) = run(&plan, &opts(true));
 
     assert_eq!(report.outcomes.len(), 4);
     assert!(
@@ -346,7 +372,7 @@ fn unicode_names_round_trip() {
     for n in names {
         write_files(&src, &[(n, n.as_bytes())]);
     }
-    let (report, _) = run(&select(&src), &dest, &opts(true));
+    let (report, _) = run(&plan(&src, &dest), &opts(true));
 
     assert!(report.is_success(), "{report:?}");
     assert_eq!(read_tree(&dest.join("Tomas")), read_tree(&src));
@@ -372,7 +398,7 @@ fn cancelling_mid_job_keeps_finished_files_and_removes_partials() {
     o.small_file_lanes = 1;
     o.large_file_lanes = 1;
     let control = JobControl::new();
-    let report = run_job(&select(&src), &dest, &o, &control, &|e| {
+    let report = run_job(&plan(&src, &dest), &o, &control, &|e| {
         if let Event::FileFinished(_) = e {
             control.cancel();
         }
@@ -410,23 +436,22 @@ fn concurrent_jobs_into_one_destination_never_report_foreign_bytes() {
         write_files(&card_a, &[(&name, &[b'A'; 3000])]);
         write_files(&card_b, &[(&name, &[b'B'; 1000])]);
     }
-    let contents = |p: &Path| {
+    let contents = |p: &Path, dest: &Path| {
         let source = Source::Directory {
             path: p.to_path_buf(),
             mode: DirMode::ContentsOnly,
         };
-        scan(&source, &ScanOptions::default())
-            .unwrap()
-            .select(&ExtensionFilter::All)
+        plan_of(&source, dest)
     };
-    let (sel_a, sel_b) = (contents(&card_a), contents(&card_b));
     for verify in [false, true] {
         let dest = dest.join(if verify { "verify" } else { "copy" });
         fs::create_dir_all(&dest).unwrap();
+        // Both plans see an empty destination, so both try every name.
+        let (plan_a, plan_b) = (contents(&card_a, &dest), contents(&card_b, &dest));
         let o = opts(verify);
         let (ra, rb) = std::thread::scope(|s| {
-            let a = s.spawn(|| run_job(&sel_a, &dest, &o, &JobControl::new(), &|_| {}));
-            let b = s.spawn(|| run_job(&sel_b, &dest, &o, &JobControl::new(), &|_| {}));
+            let a = s.spawn(|| run_job(&plan_a, &o, &JobControl::new(), &|_| {}));
+            let b = s.spawn(|| run_job(&plan_b, &o, &JobControl::new(), &|_| {}));
             (a.join().unwrap(), b.join().unwrap())
         });
         let mut claimed = std::collections::HashSet::new();
@@ -467,14 +492,11 @@ fn names_equal_after_unicode_normalization_never_mix() {
         dir.path().join("x").join(nfc),
         dir.path().join("y").join(nfd),
     ]);
-    let sel = scan(&source, &ScanOptions::default())
-        .unwrap()
-        .select(&ExtensionFilter::All);
     for verify in [false, true] {
         for round in 0..5 {
             let dest = dir.path().join(format!("dest-{verify}-{round}"));
             fs::create_dir_all(&dest).unwrap();
-            let (report, _) = run(&sel, &dest, &opts(verify));
+            let (report, _) = run(&plan_of(&source, &dest), &opts(verify));
             let ok: Vec<_> = report
                 .outcomes
                 .iter()
@@ -490,12 +512,11 @@ fn names_equal_after_unicode_normalization_never_mix() {
 #[test]
 fn a_panicking_event_handler_ends_the_job_instead_of_hanging() {
     let f = fixture();
-    let sel = select(&f.src);
-    let dest = f.dest.clone();
+    let plan = plan(&f.src, &f.dest);
     let (tx, rx) = std::sync::mpsc::channel();
     std::thread::spawn(move || {
         let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
-            run_job(&sel, &dest, &opts(false), &JobControl::new(), &|e| {
+            run_job(&plan, &opts(false), &JobControl::new(), &|e| {
                 if let Event::FileFinished(_) = e {
                     panic!("event handler failed");
                 }
@@ -527,7 +548,7 @@ fn copying_never_runs_far_ahead_of_verification() {
         ..JobOptions::default()
     };
     let max_active = std::sync::atomic::AtomicUsize::new(0);
-    let report = run_job(&select(&src), &dest, &o, &JobControl::new(), &|e| {
+    let report = run_job(&plan(&src, &dest), &o, &JobControl::new(), &|e| {
         if let Event::Progress(p) = e {
             max_active.fetch_max(p.active.len(), std::sync::atomic::Ordering::Relaxed);
         }
@@ -547,13 +568,13 @@ fn a_paused_job_does_no_io_until_resumed() {
     for i in 0..40 {
         write_files(&src, &[(&format!("f{i:02}.bin"), &pattern(2000))]);
     }
-    let sel = select(&src);
+    let plan = plan(&src, &dest);
     let control = JobControl::new();
     let finished = std::sync::atomic::AtomicUsize::new(0);
     let saw_paused = std::sync::atomic::AtomicBool::new(false);
     let report = std::thread::scope(|s| {
         let job = s.spawn(|| {
-            run_job(&sel, &dest, &opts(true), &control, &|e| match e {
+            run_job(&plan, &opts(true), &control, &|e| match e {
                 Event::FileFinished(_) => {
                     if finished.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                         control.pause();
@@ -601,9 +622,174 @@ fn partial_files_left_by_an_interrupted_job_are_replaced() {
         .set_modified(old)
         .unwrap();
 
-    let (report, _) = run(&select(&f.src), &f.dest, &opts(true));
+    let (report, _) = run(&plan(&f.src, &f.dest), &opts(true));
     assert!(report.is_success(), "{report:?}");
     assert_eq!(report.removed_partials, 1);
     assert!(!leftover.exists());
     assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
 }
+
+/// Gives `rel` under `dest` the same contents and modification time as under `src`.
+fn copy_like(src: &Path, dest: &Path, rel: &str) {
+    let (from, to) = (src.join(rel), dest.join(rel));
+    fs::create_dir_all(to.parent().unwrap()).unwrap();
+    fs::copy(&from, &to).unwrap();
+    let mtime = fs::metadata(&from).unwrap().modified().unwrap();
+    fs::File::options()
+        .write(true)
+        .open(&to)
+        .unwrap()
+        .set_modified(mtime)
+        .unwrap();
+}
+
+#[test]
+fn identical_files_are_skipped_and_left_out_of_the_checksum_file() {
+    let f = fixture();
+    copy_like(f.src.parent().unwrap(), &f.dest, "CARD/A001.mov");
+    let plan = plan(&f.src, &f.dest);
+    let (report, events) = run(&plan, &opts(true));
+
+    assert!(report.is_success(), "{report:?}");
+    let skipped: Vec<_> = report.skipped().collect();
+    assert_eq!(skipped.len(), 1);
+    assert_eq!(
+        skipped[0].status,
+        FileStatus::Skipped(SkipReason::Identical)
+    );
+    assert_eq!(skipped[0].hash, None, "a skipped file is not read");
+    assert!(!skipped[0].in_checksum_file);
+    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
+    assert!(!sums.contains("A001"), "{sums}");
+    assert_eq!(sums.lines().count(), 3);
+    let last = events
+        .iter()
+        .rev()
+        .find_map(|e| match e {
+            Event::Progress(p) => Some(p.clone()),
+            _ => None,
+        })
+        .unwrap();
+    assert_eq!(last.files_skipped, 1);
+    assert_eq!(last.total_bytes, plan.bytes_to_write());
+}
+
+#[test]
+fn keep_both_copies_under_a_new_name_and_lists_it() {
+    let f = fixture();
+    write_files(&f.dest, &[("CARD/notes.txt", b"mine, and different")]);
+    let (report, _) = run(&plan(&f.src, &f.dest), &opts(true));
+
+    assert!(report.is_success(), "{report:?}");
+    assert_eq!(
+        fs::read(f.dest.join("CARD/notes.txt")).unwrap(),
+        b"mine, and different"
+    );
+    assert_eq!(
+        fs::read(f.dest.join("CARD/notes (1).txt")).unwrap(),
+        b"hello"
+    );
+    let notes = report
+        .outcomes
+        .iter()
+        .find(|o| o.rel.ends_with("notes.txt"))
+        .unwrap();
+    assert_eq!(notes.final_rel, Path::new("CARD").join("notes (1).txt"));
+    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
+    assert!(sums.contains("  CARD/notes (1).txt\n"), "{sums}");
+}
+
+#[test]
+fn overwrite_replaces_a_different_file_after_verifying() {
+    let f = fixture();
+    write_files(&f.dest, &[("CARD/notes.txt", b"old")]);
+    let source = Source::Directory {
+        path: f.src.clone(),
+        mode: DirMode::FolderItself,
+    };
+    let plan = plan_with(&source, &f.dest, DiffersPolicy::Overwrite);
+    let (report, _) = run(&plan, &opts(true));
+
+    assert!(report.is_success(), "{report:?}");
+    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
+}
+
+#[test]
+fn a_copy_that_fails_verification_never_replaces_the_old_file() {
+    let f = fixture();
+    write_files(&f.dest, &[("CARD/A001.mov", b"old")]);
+    let source = Source::Directory {
+        path: f.src.clone(),
+        mode: DirMode::FolderItself,
+    };
+    let plan = plan_with(&source, &f.dest, DiffersPolicy::Overwrite);
+    let mut o = opts(true);
+    o.hooks = Hooks {
+        after_copy: Some(|p, _| {
+            if p.to_string_lossy().contains("A001") {
+                flip_first_byte(p);
+            }
+        }),
+    };
+    let (report, _) = run(&plan, &o);
+
+    assert_eq!(report.failed().count(), 1);
+    assert_eq!(fs::read(f.dest.join("CARD/A001.mov")).unwrap(), b"old");
+}
+
+#[test]
+fn skip_leaves_a_different_file_alone() {
+    let f = fixture();
+    write_files(&f.dest, &[("CARD/notes.txt", b"old")]);
+    let source = Source::Directory {
+        path: f.src.clone(),
+        mode: DirMode::FolderItself,
+    };
+    let plan = plan_with(&source, &f.dest, DiffersPolicy::Skip);
+    let (report, _) = run(&plan, &opts(false));
+
+    assert!(report.is_success(), "{report:?}");
+    assert_eq!(fs::read(f.dest.join("CARD/notes.txt")).unwrap(), b"old");
+    let skipped: Vec<_> = report.skipped().collect();
+    assert_eq!(skipped[0].status, FileStatus::Skipped(SkipReason::Differs));
+}
+
+#[test]
+fn failed_files_can_be_retried() {
+    let f = fixture();
+    let source = Source::Directory {
+        path: f.src.clone(),
+        mode: DirMode::FolderItself,
+    };
+    let sel = scan(&source, &ScanOptions::default())
+        .unwrap()
+        .select(&ExtensionFilter::All);
+    let pf = preflight(&source, &sel, &f.dest).unwrap();
+    let first = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
+    let mut o = opts(true);
+    o.hooks = Hooks {
+        after_copy: Some(|p, _| {
+            if p.to_string_lossy().contains("A001") {
+                flip_first_byte(p);
+            }
+        }),
+    };
+    let (report, _) = run(&first, &o);
+    let failed: Vec<usize> = report.failed().map(|o| o.id).collect();
+    assert_eq!(failed.len(), 1);
+
+    // "Retry failed": the same selection, only the failed files, checked again.
+    let retry = sel.subset(&failed);
+    let pf = preflight(&source, &retry, &f.dest).unwrap();
+    let second = Plan::resolve(&retry, &pf, DiffersPolicy::KeepBoth);
+    assert_eq!(second.files[0].action, Action::Copy);
+    let (report, _) = run(&second, &opts(true));
+    assert!(report.is_success(), "{report:?}");
+    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
+}
+
+#[test]
+fn small_files_never_take_the_pipelined_path_by_default() {
+    let o = JobOptions::default();
+    assert_eq!(o.small_file_threshold, o.copy.buffer_size as u64);
+}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test job`
Expected: compile errors: `run_job` takes a `&Selection`; no `SkipReason`, no `files_skipped`.

- [ ] **Step 3: Implement**

Change `crates/secopy-cli/src/main.rs`:

```diff
diff --git a/crates/secopy-cli/src/main.rs b/crates/secopy-cli/src/main.rs
index 5848373..ae3030d 100644
--- a/crates/secopy-cli/src/main.rs
+++ b/crates/secopy-cli/src/main.rs
@@ -8,6 +8,8 @@ use std::time::{Duration, Instant};
 use clap::Parser;
 use secopy_core::filter::ExtensionFilter;
 use secopy_core::job::{self, Event, FileStatus, JobControl, JobOptions, JobReport, Progress};
+use secopy_core::plan::{DiffersPolicy, Plan};
+use secopy_core::preflight::preflight;
 use secopy_core::scan::{self, ScanOptions};
 use secopy_core::source::{DirMode, Source};
 
@@ -84,6 +86,11 @@ fn run(args: Args) -> Result<ExitCode, String> {
         fmt_bytes(selection.total_bytes),
         scan.skipped_hidden
     );
+    let pf = preflight(&source, &selection, &args.to).map_err(|e| e.to_string())?;
+    let plan = Plan::resolve(&selection, &pf, DiffersPolicy::KeepBoth);
+    if let Some(blocker) = plan.blockers().first() {
+        return Err(blocker.to_string());
+    }
 
     let opts = JobOptions {
         verify: args.verify,
@@ -96,28 +103,22 @@ fn run(args: Args) -> Result<ExitCode, String> {
 
     let started = Instant::now();
     let last_print = Mutex::new(Instant::now());
-    let report = job::run_job(
-        &selection,
-        &args.to,
-        &opts,
-        &control,
-        &|event| match event {
-            Event::Progress(p) => {
-                let mut last = last_print.lock().unwrap();
-                if last.elapsed() >= Duration::from_millis(500) {
-                    *last = Instant::now();
-                    eprint!("\r{}", progress_line(&p, started.elapsed()));
-                }
+    let report = job::run_job(&plan, &opts, &control, &|event| match event {
+        Event::Progress(p) => {
+            let mut last = last_print.lock().unwrap();
+            if last.elapsed() >= Duration::from_millis(500) {
+                *last = Instant::now();
+                eprint!("\r{}", progress_line(&p, started.elapsed()));
             }
-            Event::FileFinished(o) => {
-                if let FileStatus::Failed(e) = &o.status {
-                    eprintln!("\rFAILED {}: {e}", o.rel.display());
-                }
+        }
+        Event::FileFinished(o) => {
+            if let FileStatus::Failed(e) = &o.status {
+                eprintln!("\rFAILED {}: {e}", o.rel.display());
             }
-        },
-    );
+        }
+    });
     eprintln!();
-    print_summary(&report, selection.total_bytes);
+    print_summary(&report, plan.bytes_to_write());
     Ok(if report.is_success() {
         ExitCode::SUCCESS
     } else {
```

Change `crates/secopy-core/src/job/mod.rs`:

```diff
diff --git a/crates/secopy-core/src/job/mod.rs b/crates/secopy-core/src/job/mod.rs
index 4a1f954..d6a7767 100644
--- a/crates/secopy-core/src/job/mod.rs
+++ b/crates/secopy-core/src/job/mod.rs
@@ -16,7 +16,8 @@ use crate::checksum_file;
 use crate::copy::CopyConfig;
 use crate::error::FileError;
 use crate::os;
-use crate::scan::{DirEntry, Selection};
+use crate::plan::Plan;
+use crate::scan::DirEntry;
 use crate::verify::CacheBypass;
 
 pub use crate::control::JobControl;
@@ -47,7 +48,9 @@ impl Default for JobOptions {
             verify: true,
             write_checksum_file: true,
             copy: CopyConfig::default(),
-            small_file_threshold: 8 << 20,
+            // Equal to the buffer size, so small-file lanes never take the pipelined path
+            // and buffer memory stays bounded (NFR-4).
+            small_file_threshold: 4 << 20,
             small_file_lanes: 8,
             large_file_lanes: 1,
             verify_lanes: 2,
@@ -69,16 +72,33 @@ pub struct Hooks {
 pub enum FileStatus {
     Copied,
     Verified,
+    /// Not read or written (FR-17).
+    Skipped(SkipReason),
     Failed(FileError),
 }
 
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum SkipReason {
+    /// Already at the destination with the same size and modification time. Not checked.
+    Identical,
+    /// A different file has this name, and the user chose Skip.
+    Differs,
+}
+
 #[derive(Debug, Clone, PartialEq, Eq)]
 pub struct FileOutcome {
+    /// Index in `Plan::files`.
+    pub id: usize,
+    /// Path relative to the destination, as selected.
     pub rel: PathBuf,
+    /// Where the copy landed; differs from `rel` for Keep both.
+    pub final_rel: PathBuf,
     pub size: u64,
-    /// Source hash; `None` if the file failed before it was hashed.
+    /// Source hash; `None` if the file wasn't read or failed before it was hashed.
     pub hash: Option<u64>,
     pub status: FileStatus,
+    /// Listed in this job's checksum file.
+    pub in_checksum_file: bool,
     pub elapsed: Duration,
 }
 
@@ -112,6 +132,12 @@ impl JobReport {
             .filter(|o| matches!(o.status, FileStatus::Failed(_)))
     }
 
+    pub fn skipped(&self) -> impl Iterator<Item = &FileOutcome> {
+        self.outcomes
+            .iter()
+            .filter(|o| matches!(o.status, FileStatus::Skipped(_)))
+    }
+
     pub fn is_success(&self) -> bool {
         self.fatal.is_none()
             && !self.cancelled
@@ -120,24 +146,25 @@ impl JobReport {
     }
 }
 
-/// Copies every file in `sel` into `dest`. Blocks until done; call from a worker thread.
+/// Carries out `plan`. Blocks until done; call from a worker thread.
 /// `on_event` is called from several threads.
 pub fn run_job(
-    sel: &Selection,
-    dest: &Path,
+    plan: &Plan,
     opts: &JobOptions,
     control: &JobControl,
     on_event: &(dyn Fn(Event) + Sync),
 ) -> JobReport {
     let started = Instant::now();
-    for dir in &sel.dirs {
-        // A failure here surfaces as a per-file write error.
-        let _ = fs::create_dir_all(dest.join(&dir.rel));
+    let dest = plan.dest.as_path();
+    let runner = Runner::new(plan, opts, control, on_event);
+    let (write, unwritten): (Vec<usize>, Vec<usize>) =
+        (0..plan.files.len()).partition(|&i| plan.files[i].action.writes());
+    for idx in unwritten {
+        runner.finish_unwritten(idx);
     }
-    let clashes = find_name_clashes(sel);
-    let runner = Runner::new(sel, dest, opts, control, on_event, clashes);
-    let (small, large): (Vec<usize>, Vec<usize>) =
-        (0..sel.files.len()).partition(|&i| sel.files[i].size <= opts.small_file_threshold);
+    let (small, large): (Vec<usize>, Vec<usize>) = write
+        .into_iter()
+        .partition(|&i| plan.files[i].entry.size <= opts.small_file_threshold);
     let small = Queue::new(small);
     let large = Queue::new(large);
     let finished = AtomicBool::new(false);
@@ -190,14 +217,17 @@ pub fn run_job(
         .into_inner()
         .expect("outcomes lock poisoned");
     let fatal = runner.fatal.into_inner().expect("fatal lock poisoned");
+    if !control.is_stopped() {
+        create_empty_dirs(plan);
+    }
     let (checksum_file, checksum_error) = if opts.write_checksum_file {
         write_checksum(dest, &outcomes)
     } else {
         (None, None)
     };
-    make_durable(dest, &sel.dirs);
+    make_durable(dest, &plan.dirs);
     JobReport {
-        not_started: (sel.files.len() - outcomes.len()) as u64,
+        not_started: (plan.files.len() - outcomes.len()) as u64,
         outcomes,
         checksum_file,
         checksum_error,
@@ -213,22 +243,31 @@ pub fn run_job(
     }
 }
 
-/// Marks every file whose destination path repeats an earlier one, ignoring case:
-/// two lanes writing the same name would corrupt each other, and case-insensitive
-/// destinations (macOS, Windows, exFAT) treat `a.txt` and `A.TXT` as one file.
-fn find_name_clashes(sel: &Selection) -> Vec<bool> {
-    let mut seen = HashSet::new();
-    sel.files
+/// Source folders with no files in the plan are created at the end (FR-6). Folders with
+/// files were created when their first file started.
+fn create_empty_dirs(plan: &Plan) {
+    let mut with_files = HashSet::new();
+    for file in &plan.files {
+        for dir in file.entry.rel.ancestors().skip(1) {
+            if !with_files.insert(dir) {
+                break;
+            }
+        }
+    }
+    for dir in plan
+        .dirs
         .iter()
-        .map(|f| !seen.insert(checksum_file::slash_path(&f.rel).to_lowercase()))
-        .collect()
+        .filter(|d| !with_files.contains(d.rel.as_path()))
+    {
+        let _ = fs::create_dir_all(plan.dest.join(&dir.rel));
+    }
 }
 
 fn write_checksum(dest: &Path, outcomes: &[FileOutcome]) -> (Option<PathBuf>, Option<String>) {
     let entries: Vec<(PathBuf, u64)> = outcomes
         .iter()
-        .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
-        .filter_map(|o| o.hash.map(|h| (o.rel.clone(), h)))
+        .filter(|o| o.in_checksum_file)
+        .filter_map(|o| o.hash.map(|h| (o.final_rel.clone(), h)))
         .collect();
     if entries.is_empty() {
         return (None, None);
@@ -242,6 +281,7 @@ fn write_checksum(dest: &Path, outcomes: &[FileOutcome]) -> (Option<PathBuf>, Op
 /// Makes the job durable: one fsync per directory for the renames, then one
 /// drive-cache flush for the whole volume (RFD §7.4).
 fn make_durable(dest: &Path, dirs: &[DirEntry]) {
+    // Folders that were never created fail to open and are skipped.
     #[cfg(unix)]
     for dir in dirs
         .iter()
```

Change `crates/secopy-core/src/job/progress.rs`:

```diff
diff --git a/crates/secopy-core/src/job/progress.rs b/crates/secopy-core/src/job/progress.rs
index 5e2b3f2..d5d71cf 100644
--- a/crates/secopy-core/src/job/progress.rs
+++ b/crates/secopy-core/src/job/progress.rs
@@ -16,7 +16,7 @@ pub enum Phase {
 /// A file currently being copied or verified (RFD §5.3, "Active files").
 #[derive(Debug, Clone, PartialEq, Eq)]
 pub struct ActiveFile {
-    /// Index in `Selection::files`; stable for the whole job.
+    /// Index in `Plan::files`; stable for the whole job.
     pub id: usize,
     pub rel: PathBuf,
     pub size: u64,
@@ -29,8 +29,12 @@ pub struct ActiveFile {
 #[derive(Debug, Clone, Default, PartialEq, Eq)]
 pub struct Progress {
     pub total_files: u64,
+    /// Bytes the job writes; skipped files are not included.
     pub total_bytes: u64,
+    /// Finished files, including skipped and failed ones.
     pub files_done: u64,
+    /// Files skipped because they were already at the destination (FR-17).
+    pub files_skipped: u64,
     pub copied_bytes: u64,
     pub verified_bytes: u64,
     pub active: Vec<ActiveFile>,
@@ -92,11 +96,12 @@ impl Runner<'_> {
                 }
             })
             .collect();
-        let total_bytes = self.sel.total_bytes;
+        let total_bytes = self.bytes_to_write;
         Progress {
-            total_files: self.sel.files.len() as u64,
+            total_files: self.plan.files.len() as u64,
             total_bytes,
             files_done: self.files_done.load(Relaxed),
+            files_skipped: self.files_skipped.load(Relaxed),
             copied_bytes: copied.min(total_bytes),
             verified_bytes: verified.min(total_bytes),
             active: files,
```

Change `crates/secopy-core/src/job/runner.rs`:

```diff
diff --git a/crates/secopy-core/src/job/runner.rs b/crates/secopy-core/src/job/runner.rs
index 341c36a..e290377 100644
--- a/crates/secopy-core/src/job/runner.rs
+++ b/crates/secopy-core/src/job/runner.rs
@@ -1,17 +1,18 @@
 //! Copy and verify lanes (RFD §7.2, §7.3).
 
+use std::collections::HashSet;
 use std::fs;
-use std::path::Path;
+use std::path::{Path, PathBuf};
 use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering::Relaxed};
 use std::sync::{Arc, Mutex, mpsc};
 use std::time::Instant;
 
 use super::progress::{Phase, Slot};
-use super::{Event, FileOutcome, FileStatus, JobControl, JobOptions};
+use super::{Event, FileOutcome, FileStatus, JobControl, JobOptions, SkipReason};
 use crate::copy::{self, Commit, CopyConfig, PartialCopy};
 use crate::error::FileError;
 use crate::hash;
-use crate::scan::{ScanEntry, Selection};
+use crate::plan::{Action, Plan, PlannedFile};
 use crate::verify::{self, CacheBypass};
 
 /// Verify tasks that may wait per verify lane before copy lanes block (RFD §7.3).
@@ -53,57 +54,72 @@ pub(super) struct VerifyTask {
 }
 
 pub(super) struct Runner<'a> {
-    pub(super) sel: &'a Selection,
-    pub(super) dest: &'a Path,
+    pub(super) plan: &'a Plan,
     pub(super) opts: &'a JobOptions,
     pub(super) copy_cfg: CopyConfig,
     pub(super) control: &'a JobControl,
     pub(super) on_event: &'a (dyn Fn(Event) + Sync),
-    /// Indexed like `Selection::files`; true = skip with `FileError::NameClash`.
-    pub(super) clashes: Vec<bool>,
+    pub(super) bytes_to_write: u64,
     pub(super) copied_done: AtomicU64,
     pub(super) verified_done: AtomicU64,
     pub(super) files_done: AtomicU64,
+    pub(super) files_skipped: AtomicU64,
     pub(super) active: Mutex<Vec<Arc<Slot>>>,
     pub(super) outcomes: Mutex<Vec<FileOutcome>>,
     pub(super) fatal: Mutex<Option<FileError>>,
     pub(super) bypass_unavailable: AtomicBool,
     /// Partial files left by interrupted jobs that were removed (FR-18).
     pub(super) removed_partials: AtomicU64,
+    /// Folders already created, so each one costs one system call (FR-10).
+    created_dirs: Mutex<HashSet<PathBuf>>,
 }
 
 impl<'a> Runner<'a> {
     pub(super) fn new(
-        sel: &'a Selection,
-        dest: &'a Path,
+        plan: &'a Plan,
         opts: &'a JobOptions,
         control: &'a JobControl,
         on_event: &'a (dyn Fn(Event) + Sync),
-        clashes: Vec<bool>,
     ) -> Self {
         let copy_cfg = CopyConfig {
             uncached_write: opts.verify,
             ..opts.copy.clone()
         };
         Self {
-            sel,
-            dest,
+            plan,
             opts,
             copy_cfg,
             control,
             on_event,
-            clashes,
+            bytes_to_write: plan.bytes_to_write(),
             copied_done: AtomicU64::new(0),
             verified_done: AtomicU64::new(0),
             files_done: AtomicU64::new(0),
+            files_skipped: AtomicU64::new(0),
             active: Mutex::new(Vec::new()),
             outcomes: Mutex::new(Vec::new()),
             fatal: Mutex::new(None),
             bypass_unavailable: AtomicBool::new(false),
             removed_partials: AtomicU64::new(0),
+            created_dirs: Mutex::new(HashSet::new()),
         }
     }
 
+    /// Records files the plan doesn't write: skipped ones and pre-flight failures.
+    pub(super) fn finish_unwritten(&self, idx: usize) {
+        let file = &self.plan.files[idx];
+        let status = match &file.action {
+            Action::SkipIdentical => FileStatus::Skipped(SkipReason::Identical),
+            Action::SkipDiffers => FileStatus::Skipped(SkipReason::Differs),
+            Action::Fail(e) => FileStatus::Failed(e.clone()),
+            _ => unreachable!("only unwritten files"),
+        };
+        if matches!(status, FileStatus::Skipped(_)) {
+            self.files_skipped.fetch_add(1, Relaxed);
+        }
+        self.finish(None, self.outcome(idx, None, status, None, Instant::now()));
+    }
+
     pub(super) fn copy_lane(&self, queue: &Queue, verify_tx: &mpsc::SyncSender<VerifyTask>) {
         // Blocks while paused, so no new file starts during a pause (FR-22).
         while self.control.checkpoint().is_ok() {
@@ -113,23 +129,21 @@ impl<'a> Runner<'a> {
     }
 
     fn copy_one(&self, idx: usize, verify_tx: &mpsc::SyncSender<VerifyTask>) {
-        let entry = &self.sel.files[idx];
-        let final_path = self.dest.join(&entry.rel);
+        let file = &self.plan.files[idx];
+        let final_path = self.plan.dest.join(file.final_rel());
         let started = Instant::now();
-        let slot = self.begin(idx, entry);
-        let blocked = if self.clashes[idx] {
-            Some(FileError::NameClash)
-        } else if fs::symlink_metadata(&final_path).is_ok() {
-            Some(FileError::AlreadyExists)
-        } else {
-            None
-        };
-        if let Some(e) = blocked {
-            return self.finish(&slot, entry, None, FileStatus::Failed(e), started);
+        let slot = self.begin(idx, file);
+        // A file that appeared since pre-flight is never replaced.
+        if file.action == Action::Copy && fs::symlink_metadata(&final_path).is_ok() {
+            let status = FileStatus::Failed(FileError::AlreadyExists);
+            return self.finish(Some(&slot), self.outcome(idx, None, status, None, started));
         }
-        let partial = match self.copy_attempt(entry, &final_path, &slot, 0) {
+        let partial = match self.copy_attempt(file, &final_path, &slot, 0) {
             Ok(partial) => partial,
-            Err(e) => return self.finish(&slot, entry, None, FileStatus::Failed(e), started),
+            Err(e) => {
+                let status = FileStatus::Failed(e);
+                return self.finish(Some(&slot), self.outcome(idx, None, status, None, started));
+            }
         };
         self.count_copied(&slot);
         if self.opts.verify {
@@ -146,22 +160,24 @@ impl<'a> Runner<'a> {
             return;
         }
         let hash = partial.hash;
-        match partial.commit(&final_path, Commit::NoReplace) {
-            Ok(_) => self.finish(&slot, entry, Some(hash), FileStatus::Copied, started),
-            Err(e) => self.finish(&slot, entry, Some(hash), FileStatus::Failed(e), started),
-        }
+        let outcome = match self.commit(file, partial) {
+            Ok(landed) => self.outcome(idx, Some(hash), FileStatus::Copied, Some(landed), started),
+            Err(e) => self.outcome(idx, Some(hash), FileStatus::Failed(e), None, started),
+        };
+        self.finish(Some(&slot), outcome);
     }
 
     fn copy_attempt(
         &self,
-        entry: &ScanEntry,
+        file: &PlannedFile,
         final_path: &Path,
         slot: &Slot,
         attempt: u32,
     ) -> Result<PartialCopy, FileError> {
         slot.set_phase(Phase::Copying);
+        self.create_parent(final_path)?;
         let partial = copy::copy_to_partial(
-            &entry.source,
+            &file.entry.source,
             final_path,
             &self.copy_cfg,
             &|b| slot.bytes.store(b, Relaxed),
@@ -176,6 +192,48 @@ impl<'a> Runner<'a> {
         Ok(partial)
     }
 
+    /// Folders are created when their first file starts, so a cancelled or failed job
+    /// leaves no empty folders behind (FR-10).
+    fn create_parent(&self, final_path: &Path) -> Result<(), FileError> {
+        let Some(parent) = final_path.parent() else {
+            return Ok(());
+        };
+        if self
+            .created_dirs
+            .lock()
+            .expect("dirs lock poisoned")
+            .contains(parent)
+        {
+            return Ok(());
+        }
+        fs::create_dir_all(parent).map_err(FileError::write_dest)?;
+        self.created_dirs
+            .lock()
+            .expect("dirs lock poisoned")
+            .insert(parent.to_path_buf());
+        Ok(())
+    }
+
+    /// Gives the copy its final name as the plan says; returns where it landed,
+    /// relative to the destination.
+    fn commit(&self, file: &PlannedFile, partial: PartialCopy) -> Result<PathBuf, FileError> {
+        let dest = &self.plan.dest;
+        let original = dest.join(&file.entry.rel);
+        let how = match &file.action {
+            Action::Overwrite => Commit::Replace,
+            Action::KeepBoth { n, .. } => Commit::KeepBoth {
+                original: &original,
+                n: *n,
+            },
+            _ => Commit::NoReplace,
+        };
+        let landed = partial.commit(&dest.join(file.final_rel()), how)?;
+        Ok(landed
+            .strip_prefix(dest)
+            .map(Path::to_path_buf)
+            .unwrap_or(landed))
+    }
+
     pub(super) fn verify_lane(&self, rx: &Mutex<mpsc::Receiver<VerifyTask>>) {
         loop {
             let task = match rx.lock().expect("verify queue lock poisoned").recv() {
@@ -194,8 +252,8 @@ impl<'a> Runner<'a> {
             mut partial,
             started,
         } = task;
-        let entry = &self.sel.files[idx];
-        let final_path = self.dest.join(&entry.rel);
+        let file = &self.plan.files[idx];
+        let final_path = self.plan.dest.join(file.final_rel());
         let mut attempt = 0;
         let result = loop {
             slot.set_phase(Phase::Verifying);
@@ -216,9 +274,7 @@ impl<'a> Runner<'a> {
             }
             let expected = partial.hash;
             if actual == expected {
-                break partial
-                    .commit(&final_path, Commit::NoReplace)
-                    .map(|_| expected);
+                break self.commit(file, partial).map(|landed| (expected, landed));
             }
             partial.discard();
             if attempt == 1 {
@@ -228,22 +284,25 @@ impl<'a> Runner<'a> {
                 });
             }
             attempt += 1;
-            partial = match self.copy_attempt(entry, &final_path, &slot, attempt) {
+            partial = match self.copy_attempt(file, &final_path, &slot, attempt) {
                 Ok(p) => p,
                 Err(e) => break Err(e),
             };
         };
-        match result {
-            Ok(hash) => self.finish(&slot, entry, Some(hash), FileStatus::Verified, started),
-            Err(e) => self.finish(&slot, entry, None, FileStatus::Failed(e), started),
-        }
+        let outcome = match result {
+            Ok((hash, landed)) => {
+                self.outcome(idx, Some(hash), FileStatus::Verified, Some(landed), started)
+            }
+            Err(e) => self.outcome(idx, None, FileStatus::Failed(e), None, started),
+        };
+        self.finish(Some(&slot), outcome);
     }
 
-    fn begin(&self, idx: usize, entry: &ScanEntry) -> Arc<Slot> {
+    fn begin(&self, idx: usize, file: &PlannedFile) -> Arc<Slot> {
         let slot = Arc::new(Slot {
             id: idx,
-            rel: entry.rel.clone(),
-            size: entry.size,
+            rel: file.final_rel().to_path_buf(),
+            size: file.entry.size,
             phase: AtomicU8::new(Phase::Copying as u8),
             bytes: AtomicU64::new(0),
             copy_counted: AtomicBool::new(false),
@@ -261,25 +320,43 @@ impl<'a> Runner<'a> {
         }
     }
 
-    fn finish(
+    fn outcome(
         &self,
-        slot: &Arc<Slot>,
-        entry: &ScanEntry,
+        idx: usize,
         hash: Option<u64>,
         status: FileStatus,
+        landed: Option<PathBuf>,
         started: Instant,
-    ) {
-        self.active
-            .lock()
-            .expect("active lock poisoned")
-            .retain(|s| !Arc::ptr_eq(s, slot));
-        // A finished file counts as fully processed, so the bars reach 100 % even with failures.
-        self.count_copied(slot);
-        if self.opts.verify {
-            self.verified_done.fetch_add(slot.size, Relaxed);
+    ) -> FileOutcome {
+        let file = &self.plan.files[idx];
+        let ok = matches!(status, FileStatus::Copied | FileStatus::Verified);
+        FileOutcome {
+            id: idx,
+            rel: file.entry.rel.clone(),
+            final_rel: landed.unwrap_or_else(|| file.final_rel().to_path_buf()),
+            size: file.entry.size,
+            hash,
+            in_checksum_file: ok && hash.is_some() && file.in_checksum_file,
+            status,
+            elapsed: started.elapsed(),
+        }
+    }
+
+    fn finish(&self, slot: Option<&Arc<Slot>>, outcome: FileOutcome) {
+        if let Some(slot) = slot {
+            self.active
+                .lock()
+                .expect("active lock poisoned")
+                .retain(|s| !Arc::ptr_eq(s, slot));
+            // A finished file counts as fully processed, so the bars reach 100 % even
+            // with failures.
+            self.count_copied(slot);
+            if self.opts.verify {
+                self.verified_done.fetch_add(slot.size, Relaxed);
+            }
         }
         self.files_done.fetch_add(1, Relaxed);
-        if let FileStatus::Failed(e) = &status
+        if let FileStatus::Failed(e) = &outcome.status
             && e.is_fatal()
         {
             self.fatal
@@ -288,13 +365,6 @@ impl<'a> Runner<'a> {
                 .get_or_insert(e.clone());
             self.control.cancel();
         }
-        let outcome = FileOutcome {
-            rel: entry.rel.clone(),
-            size: entry.size,
-            hash,
-            status,
-            elapsed: started.elapsed(),
-        };
         self.outcomes
             .lock()
             .expect("outcomes lock poisoned")
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): run jobs from a resolved plan

Skipped and failed files finish without I/O, copies commit as their
action says, and the checksum file lists final names (FR-17, FR-31).
Folders are created with their first file, so a cancelled job leaves no
empty folders (FR-10). small_file_threshold defaults to the buffer size,
which bounds buffer memory (NFR-4). Both are plan 1 carry-overs.
EOF
```


---

### Task 11: Metadata (FR-19)

Before the final `fsync`, the copy gets:
- the source's modification time
- the access time
- the creation time (macOS, Windows)
- the permission bits (`mode & 0o777`, macOS/Linux)

All of it is set on the open file, so it is flushed with the data. If a file system
rejects part of it, the modification time alone is kept; the identical check depends on it.

Folder mtimes are restored at the end, deepest folder first, unless the job was cancelled.

**Files:**
- Create: `crates/secopy-core/src/metadata.rs`
- Modify: `crates/secopy-core/src/copy.rs`, `crates/secopy-core/src/job/mod.rs`, `crates/secopy-core/src/lib.rs`, `crates/secopy-core/tests/job.rs`

**Interfaces:**
- Consumes: Task 3's mtimes, Task 10's `create_empty_dirs`.
- Produces: private `metadata::{copy_to(&Metadata, &File), set_dir_mtime(&Path, SystemTime)}`.

- [ ] **Step 1: Write the failing tests**

Update the tests in `crates/secopy-core/tests/job.rs`:

```diff
diff --git a/crates/secopy-core/tests/job.rs b/crates/secopy-core/tests/job.rs
index f6f5932..9483909 100644
--- a/crates/secopy-core/tests/job.rs
+++ b/crates/secopy-core/tests/job.rs
@@ -793,3 +793,56 @@ fn small_files_never_take_the_pipelined_path_by_default() {
     let o = JobOptions::default();
     assert_eq!(o.small_file_threshold, o.copy.buffer_size as u64);
 }
+
+#[test]
+fn file_and_folder_metadata_is_kept() {
+    let f = fixture();
+    let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
+    let file = f.src.join("clips/B002.mov");
+    #[allow(unused_mut)]
+    let mut times = fs::FileTimes::new().set_modified(t).set_accessed(t);
+    #[cfg(target_os = "macos")]
+    {
+        use std::os::macos::fs::FileTimesExt;
+        times = times.set_created(t);
+    }
+    fs::File::options()
+        .write(true)
+        .open(&file)
+        .unwrap()
+        .set_times(times)
+        .unwrap();
+    #[cfg(unix)]
+    {
+        use std::os::unix::fs::PermissionsExt;
+        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
+    }
+    let dir_t = t + std::time::Duration::from_secs(3600);
+    let clips_set = fs::File::open(f.src.join("clips"))
+        .and_then(|d| d.set_modified(dir_t))
+        .is_ok();
+
+    for verify in [false, true] {
+        let dest = f.dest.join(if verify { "verify" } else { "copy" });
+        fs::create_dir_all(&dest).unwrap();
+        let (report, _) = run(&plan(&f.src, &dest), &opts(verify));
+        assert!(report.is_success(), "{report:?}");
+        let copy = fs::metadata(dest.join("CARD/clips/B002.mov")).unwrap();
+        assert_eq!(copy.modified().unwrap(), t, "verify={verify}");
+        #[cfg(target_os = "macos")]
+        assert_eq!(copy.created().unwrap(), t);
+        #[cfg(unix)]
+        {
+            use std::os::unix::fs::PermissionsExt;
+            assert_eq!(copy.permissions().mode() & 0o777, 0o640);
+        }
+        if clips_set {
+            let clips = fs::metadata(dest.join("CARD/clips")).unwrap();
+            assert_eq!(
+                clips.modified().unwrap(),
+                dir_t,
+                "folder mtime (verify={verify})"
+            );
+        }
+    }
+}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test job file_and_folder_metadata`
Expected: the test fails: the copy's mtime is the copy time.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/copy.rs`:

```diff
diff --git a/crates/secopy-core/src/copy.rs b/crates/secopy-core/src/copy.rs
index f39742c..93b1702 100644
--- a/crates/secopy-core/src/copy.rs
+++ b/crates/secopy-core/src/copy.rs
@@ -8,7 +8,7 @@ use std::sync::mpsc;
 
 use crate::control::JobControl;
 use crate::error::FileError;
-use crate::{hash, os};
+use crate::{hash, metadata, os};
 
 /// Tuning for the copy pipeline.
 #[derive(Debug, Clone, PartialEq, Eq)]
@@ -242,7 +242,8 @@ fn copy_inner(
     progress: &dyn Fn(u64),
     control: &JobControl,
 ) -> Result<(u64, u64), FileError> {
-    let len = reader.metadata().map_err(FileError::read_source)?.len();
+    let src_meta = reader.metadata().map_err(FileError::read_source)?;
+    let len = src_meta.len();
     if cfg.uncached_write {
         os::set_nocache(writer);
     }
@@ -251,6 +252,7 @@ fn copy_inner(
     } else {
         copy_pipelined(reader, writer, cfg, progress, control)?
     };
+    metadata::copy_to(&src_meta, writer).map_err(FileError::write_dest)?;
     os::sync_file(writer).map_err(FileError::write_dest)?;
     Ok(result)
 }
```

Change `crates/secopy-core/src/job/mod.rs`:

```diff
diff --git a/crates/secopy-core/src/job/mod.rs b/crates/secopy-core/src/job/mod.rs
index d6a7767..ed99306 100644
--- a/crates/secopy-core/src/job/mod.rs
+++ b/crates/secopy-core/src/job/mod.rs
@@ -15,10 +15,10 @@ use chrono::Local;
 use crate::checksum_file;
 use crate::copy::CopyConfig;
 use crate::error::FileError;
-use crate::os;
 use crate::plan::Plan;
 use crate::scan::DirEntry;
 use crate::verify::CacheBypass;
+use crate::{metadata, os};
 
 pub use crate::control::JobControl;
 pub use progress::{ActiveFile, Phase, Progress};
@@ -219,6 +219,7 @@ pub fn run_job(
     let fatal = runner.fatal.into_inner().expect("fatal lock poisoned");
     if !control.is_stopped() {
         create_empty_dirs(plan);
+        restore_dir_mtimes(plan);
     }
     let (checksum_file, checksum_error) = if opts.write_checksum_file {
         write_checksum(dest, &outcomes)
@@ -263,6 +264,17 @@ fn create_empty_dirs(plan: &Plan) {
     }
 }
 
+/// Deepest folders first: setting a folder's time doesn't change its parent's (FR-19).
+fn restore_dir_mtimes(plan: &Plan) {
+    let mut dirs: Vec<&DirEntry> = plan.dirs.iter().collect();
+    dirs.sort_by_key(|d| std::cmp::Reverse(d.rel.components().count()));
+    for dir in dirs {
+        if let Some(mtime) = dir.mtime {
+            let _ = metadata::set_dir_mtime(&plan.dest.join(&dir.rel), mtime);
+        }
+    }
+}
+
 fn write_checksum(dest: &Path, outcomes: &[FileOutcome]) -> (Option<PathBuf>, Option<String>) {
     let entries: Vec<(PathBuf, u64)> = outcomes
         .iter()
```

Change `crates/secopy-core/src/lib.rs`:

```diff
diff --git a/crates/secopy-core/src/lib.rs b/crates/secopy-core/src/lib.rs
index f74b30e..af39828 100644
--- a/crates/secopy-core/src/lib.rs
+++ b/crates/secopy-core/src/lib.rs
@@ -10,6 +10,7 @@ pub mod fsinfo;
 pub mod hash;
 pub mod hidden;
 pub mod job;
+mod metadata;
 pub mod names;
 mod os;
 pub mod plan;
```

Create `crates/secopy-core/src/metadata.rs`:

```rust
//! Keeps file and folder metadata on the copies (FR-19).

use std::fs::{File, FileTimes, Metadata};
use std::io;
use std::path::Path;
use std::time::SystemTime;

/// Gives `dst` the source's modification, access and (macOS, Windows) creation times,
/// and on macOS and Linux its permission bits. Call before the final `fsync`, so the
/// metadata is flushed with the data.
pub fn copy_to(src: &Metadata, dst: &File) -> io::Result<()> {
    let mut times = FileTimes::new();
    if let Ok(t) = src.modified() {
        times = times.set_modified(t);
    }
    let full = with_extra_times(times, src);
    // Some file systems reject part of it (e.g. creation times over SMB): then keep at
    // least the modification time, which later identical-file checks rely on (FR-17).
    if dst.set_times(full).is_err() {
        dst.set_times(times)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = src.permissions().mode() & 0o777;
        dst.set_permissions(std::fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

fn with_extra_times(times: FileTimes, src: &Metadata) -> FileTimes {
    let times = match src.accessed() {
        Ok(t) => times.set_accessed(t),
        Err(_) => times,
    };
    #[cfg(target_os = "macos")]
    use std::os::macos::fs::FileTimesExt;
    #[cfg(windows)]
    use std::os::windows::fs::FileTimesExt;
    #[cfg(any(target_os = "macos", windows))]
    if let Ok(t) = src.created() {
        return times.set_created(t);
    }
    times
}

/// Sets a folder's modification time. Called after everything inside it is written,
/// deepest folders first, since writing into a folder changes its time.
pub fn set_dir_mtime(dir: &Path, mtime: SystemTime) -> io::Result<()> {
    open_dir(dir)?.set_modified(mtime)
}

#[cfg(unix)]
fn open_dir(dir: &Path) -> io::Result<File> {
    File::open(dir)
}

#[cfg(windows)]
fn open_dir(dir: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
    std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(dir)
}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

Also lint the other platforms' `cfg` code (install the targets once with
`rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`):
`cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` and
`cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`.
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): keep file and folder metadata on copies

Modification, access and (macOS, Windows) creation times and POSIX
permission bits are set on the open copy before its fsync; folder mtimes
are restored deepest first after the job.
EOF
```


---

### Task 12: Fatal errors: disk full, destination gone, source gone (FR-21)

After a per-file I/O error the runner re-checks the root the error came from:
- a write or read-back error checks the destination
- a read error checks that file's source root

If the folder is gone, is no longer a folder, or its device id changed, the job stops with
`FatalError::DestinationGone` or `SourceGone`. A full disk stays fatal (`DiskFull`).
`JobReport::fatal` becomes `Option<FatalError>`.

A copy whose byte count differs from the scanned size now fails with `SourceChanged`.
`Hooks::before_copy` lets tests pull the source away mid-job.

**Files:**
- Modify: `crates/secopy-core/src/error.rs`, `crates/secopy-core/src/job/mod.rs`, `crates/secopy-core/src/job/runner.rs`, `crates/secopy-core/tests/job.rs`

**Interfaces:**
- Consumes: Task 4's `device_id`, Task 7's `SourceRoot`.
- Produces: `error::FatalError::{DiskFull, DestinationGone, SourceGone}`; `FileError::SourceChanged`;
  `FileError::is_disk_full()` (replaces `is_fatal()`); `JobReport::fatal: Option<FatalError>`;
  `Hooks { before_copy: Option<fn(&Path)>, after_copy }`.

- [ ] **Step 1: Write the failing tests**

Update the tests in `crates/secopy-core/tests/job.rs`:

```diff
diff --git a/crates/secopy-core/tests/job.rs b/crates/secopy-core/tests/job.rs
index 9483909..382e6dd 100644
--- a/crates/secopy-core/tests/job.rs
+++ b/crates/secopy-core/tests/job.rs
@@ -6,7 +6,7 @@ use std::sync::Mutex;
 
 use common::{pattern, read_tree, write_files};
 use secopy_core::copy::CopyConfig;
-use secopy_core::error::FileError;
+use secopy_core::error::{FatalError, FileError};
 use secopy_core::filter::ExtensionFilter;
 use secopy_core::hash::{hash_bytes, to_hex};
 use secopy_core::job::{
@@ -151,6 +151,7 @@ fn a_corrupted_copy_is_recopied_once_and_then_verifies() {
     let f = fixture();
     let mut o = opts(true);
     o.hooks = Hooks {
+        before_copy: None,
         after_copy: Some(|p, attempt| {
             if attempt == 0 && p.to_string_lossy().contains("A001") {
                 flip_first_byte(p);
@@ -167,6 +168,7 @@ fn a_copy_that_stays_corrupted_fails_and_leaves_no_file() {
     let f = fixture();
     let mut o = opts(true);
     o.hooks = Hooks {
+        before_copy: None,
         after_copy: Some(|p, _| {
             if p.to_string_lossy().contains("A001") {
                 flip_first_byte(p);
@@ -339,7 +341,7 @@ fn files_that_map_to_the_same_name_never_mix() {
 }
 
 #[test]
-fn an_unwritable_destination_fails_every_file_without_hanging() {
+fn a_destination_that_turns_into_a_file_stops_the_job() {
     let f = fixture();
     let plan = plan(&f.src, &f.dest);
     // The destination turns into a file after pre-flight.
@@ -347,11 +349,11 @@ fn an_unwritable_destination_fails_every_file_without_hanging() {
     fs::write(&f.dest, b"x").unwrap();
     let (report, _) = run(&plan, &opts(true));
 
-    assert_eq!(report.outcomes.len(), 4);
+    assert_eq!(report.fatal, Some(FatalError::DestinationGone));
+    assert!(!report.cancelled, "a fatal error is not a cancel");
     assert!(
         report
-            .outcomes
-            .iter()
+            .failed()
             .all(|o| matches!(o.status, FileStatus::Failed(FileError::WriteDest(_))))
     );
     assert_eq!(report.checksum_file, None);
@@ -725,6 +727,7 @@ fn a_copy_that_fails_verification_never_replaces_the_old_file() {
     let plan = plan_with(&source, &f.dest, DiffersPolicy::Overwrite);
     let mut o = opts(true);
     o.hooks = Hooks {
+        before_copy: None,
         after_copy: Some(|p, _| {
             if p.to_string_lossy().contains("A001") {
                 flip_first_byte(p);
@@ -768,6 +771,7 @@ fn failed_files_can_be_retried() {
     let first = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
     let mut o = opts(true);
     o.hooks = Hooks {
+        before_copy: None,
         after_copy: Some(|p, _| {
             if p.to_string_lossy().contains("A001") {
                 flip_first_byte(p);
@@ -846,3 +850,105 @@ fn file_and_folder_metadata_is_kept() {
         }
     }
 }
+
+/// Many files, one lane each, so a fault in the middle leaves files not started.
+#[cfg(unix)]
+fn long_fixture() -> Fixture {
+    let dir = tempfile::tempdir().unwrap();
+    let src = dir.path().join("src/CARD");
+    let dest = dir.path().join("dest");
+    fs::create_dir_all(&dest).unwrap();
+    for i in 0..30 {
+        write_files(&src, &[(&format!("f{i:02}.bin"), &pattern(2000))]);
+    }
+    Fixture {
+        _dir: dir,
+        src,
+        dest,
+    }
+}
+
+#[cfg(unix)]
+fn one_lane(verify: bool) -> JobOptions {
+    JobOptions {
+        small_file_lanes: 1,
+        large_file_lanes: 1,
+        verify_lanes: 1,
+        ..opts(verify)
+    }
+}
+
+/// Unplugging the destination: its folder disappears mid-job.
+#[cfg(unix)]
+#[test]
+fn a_destination_that_disappears_stops_the_job() {
+    let f = long_fixture();
+    let mut o = one_lane(false);
+    o.hooks = Hooks {
+        before_copy: None,
+        after_copy: Some(|partial, _| {
+            if partial.to_string_lossy().contains("f05") {
+                // <dest>/CARD/.f05.bin.secopy-partial → move <dest> away
+                let dest = partial.parent().unwrap().parent().unwrap();
+                fs::rename(dest, dest.with_extension("gone")).unwrap();
+            }
+        }),
+    };
+    let (report, _) = run(&plan(&f.src, &f.dest), &o);
+
+    assert_eq!(report.fatal, Some(FatalError::DestinationGone));
+    assert!(report.not_started > 0, "{report:?}");
+    assert!(!report.is_success());
+}
+
+/// Unplugging the card: the source folder disappears mid-job.
+#[cfg(unix)]
+#[test]
+fn a_source_that_disappears_stops_the_job() {
+    let f = long_fixture();
+    let mut o = one_lane(true);
+    o.hooks = Hooks {
+        before_copy: Some(|source| {
+            if source.to_string_lossy().contains("f05") {
+                let card = source.parent().unwrap();
+                fs::rename(card, card.with_extension("gone")).unwrap();
+            }
+        }),
+        after_copy: None,
+    };
+    let (report, _) = run(&plan(&f.src, &f.dest), &o);
+
+    assert_eq!(report.fatal, Some(FatalError::SourceGone));
+    assert!(report.not_started > 0, "{report:?}");
+    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
+    assert_eq!(
+        sums.lines().count(),
+        5,
+        "files before the fault are kept and listed"
+    );
+}
+
+#[test]
+fn a_source_file_that_changes_while_copied_fails_alone() {
+    let f = fixture();
+    let mut o = opts(true);
+    o.hooks = Hooks {
+        before_copy: Some(|source| {
+            if source.to_string_lossy().contains("A001") {
+                fs::write(source, b"shorter now").unwrap();
+            }
+        }),
+        after_copy: None,
+    };
+    let (report, _) = run(&plan(&f.src, &f.dest), &o);
+
+    let failed: Vec<_> = report.failed().collect();
+    assert_eq!(failed.len(), 1);
+    assert_eq!(
+        failed[0].status,
+        FileStatus::Failed(FileError::SourceChanged)
+    );
+    assert_eq!(report.fatal, None);
+    assert!(!f.dest.join("CARD/A001.mov").exists());
+    assert_eq!(report.outcomes.len(), 4);
+}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test job`
Expected: compile errors: `FatalError` doesn't exist; `Hooks` has no field `before_copy`.

- [ ] **Step 3: Implement**

Change `crates/secopy-core/src/error.rs`:

```diff
diff --git a/crates/secopy-core/src/error.rs b/crates/secopy-core/src/error.rs
index a8ecbfc..fe0ee8e 100644
--- a/crates/secopy-core/src/error.rs
+++ b/crates/secopy-core/src/error.rs
@@ -50,6 +50,8 @@ pub enum FileError {
     TooLarge { limit: u64 },
     #[error("something is in the way at {}", path.display())]
     InTheWay { path: PathBuf },
+    #[error("the source file changed while it was copied")]
+    SourceChanged,
     #[error("cancelled")]
     Cancelled,
 }
@@ -67,26 +69,36 @@ impl FileError {
         FileError::ReadBack(e.into())
     }
 
-    /// Errors that stop the whole job instead of just this file (FR-21).
-    pub fn is_fatal(&self) -> bool {
+    pub fn is_disk_full(&self) -> bool {
         matches!(self, FileError::WriteDest(f) if f.kind == io::ErrorKind::StorageFull)
     }
 }
 
+/// Errors that stop the whole job instead of just one file (FR-21).
+#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
+pub enum FatalError {
+    #[error("the destination drive is full")]
+    DiskFull,
+    #[error("the destination is no longer available; was it disconnected?")]
+    DestinationGone,
+    #[error("the source is no longer available; was it disconnected?")]
+    SourceGone,
+}
+
 #[cfg(test)]
 mod tests {
     use super::*;
 
     #[test]
-    fn disk_full_is_fatal() {
+    fn disk_full_is_recognised() {
         let e = FileError::write_dest(io::Error::from(io::ErrorKind::StorageFull));
-        assert!(e.is_fatal());
+        assert!(e.is_disk_full());
     }
 
     #[test]
-    fn other_errors_are_not_fatal() {
-        let e = FileError::read_source(io::Error::from(io::ErrorKind::PermissionDenied));
-        assert!(!e.is_fatal());
-        assert!(!FileError::AlreadyExists.is_fatal());
+    fn other_errors_are_not_disk_full() {
+        let e = FileError::read_source(io::Error::from(io::ErrorKind::StorageFull));
+        assert!(!e.is_disk_full(), "only writes can fill the destination");
+        assert!(!FileError::AlreadyExists.is_disk_full());
     }
 }
```

Change `crates/secopy-core/src/job/mod.rs`:

```diff
diff --git a/crates/secopy-core/src/job/mod.rs b/crates/secopy-core/src/job/mod.rs
index ed99306..838fb04 100644
--- a/crates/secopy-core/src/job/mod.rs
+++ b/crates/secopy-core/src/job/mod.rs
@@ -14,7 +14,7 @@ use chrono::Local;
 
 use crate::checksum_file;
 use crate::copy::CopyConfig;
-use crate::error::FileError;
+use crate::error::{FatalError, FileError};
 use crate::plan::Plan;
 use crate::scan::DirEntry;
 use crate::verify::CacheBypass;
@@ -64,6 +64,8 @@ impl Default for JobOptions {
 #[doc(hidden)]
 #[derive(Debug, Clone, Copy, Default)]
 pub struct Hooks {
+    /// Called with the source file before each copy attempt.
+    pub before_copy: Option<fn(&Path)>,
     /// Called with the partial file after each copy attempt (0 = first), before verifying.
     pub after_copy: Option<fn(&Path, u32)>,
 }
@@ -120,7 +122,7 @@ pub struct JobReport {
     pub cache_bypass: Option<CacheBypass>,
     /// Partial files left by interrupted jobs that were removed (FR-18).
     pub removed_partials: u64,
-    pub fatal: Option<FileError>,
+    pub fatal: Option<FatalError>,
     pub cancelled: bool,
     pub elapsed: Duration,
 }
```

Change `crates/secopy-core/src/job/runner.rs`:

```diff
diff --git a/crates/secopy-core/src/job/runner.rs b/crates/secopy-core/src/job/runner.rs
index e290377..375864b 100644
--- a/crates/secopy-core/src/job/runner.rs
+++ b/crates/secopy-core/src/job/runner.rs
@@ -10,7 +10,8 @@ use std::time::Instant;
 use super::progress::{Phase, Slot};
 use super::{Event, FileOutcome, FileStatus, JobControl, JobOptions, SkipReason};
 use crate::copy::{self, Commit, CopyConfig, PartialCopy};
-use crate::error::FileError;
+use crate::error::{FatalError, FileError};
+use crate::fsinfo;
 use crate::hash;
 use crate::plan::{Action, Plan, PlannedFile};
 use crate::verify::{self, CacheBypass};
@@ -66,7 +67,7 @@ pub(super) struct Runner<'a> {
     pub(super) files_skipped: AtomicU64,
     pub(super) active: Mutex<Vec<Arc<Slot>>>,
     pub(super) outcomes: Mutex<Vec<FileOutcome>>,
-    pub(super) fatal: Mutex<Option<FileError>>,
+    pub(super) fatal: Mutex<Option<FatalError>>,
     pub(super) bypass_unavailable: AtomicBool,
     /// Partial files left by interrupted jobs that were removed (FR-18).
     pub(super) removed_partials: AtomicU64,
@@ -176,6 +177,9 @@ impl<'a> Runner<'a> {
     ) -> Result<PartialCopy, FileError> {
         slot.set_phase(Phase::Copying);
         self.create_parent(final_path)?;
+        if let Some(hook) = self.opts.hooks.before_copy {
+            hook(&file.entry.source);
+        }
         let partial = copy::copy_to_partial(
             &file.entry.source,
             final_path,
@@ -186,6 +190,11 @@ impl<'a> Runner<'a> {
         if partial.removed_stale {
             self.removed_partials.fetch_add(1, Relaxed);
         }
+        // Growing or shrinking while it was read: the copy matches no version of the file.
+        if partial.bytes != file.entry.size {
+            partial.discard();
+            return Err(FileError::SourceChanged);
+        }
         if let Some(hook) = self.opts.hooks.after_copy {
             hook(&partial.partial, attempt);
         }
@@ -342,6 +351,30 @@ impl<'a> Runner<'a> {
         }
     }
 
+    /// After an I/O error, checks whether the whole source or destination went away
+    /// (FR-21): the folder is gone, or its volume changed (unplugged or remounted).
+    fn fatal_cause(&self, idx: usize, e: &FileError) -> Option<FatalError> {
+        let plan = self.plan;
+        if e.is_disk_full() {
+            return Some(FatalError::DiskFull);
+        }
+        match e {
+            FileError::WriteDest(_) | FileError::ReadBack(_)
+                if !root_is_there(&plan.dest, plan.fs.device) =>
+            {
+                Some(FatalError::DestinationGone)
+            }
+            FileError::ReadSource(_) => {
+                let source = &plan.files[idx].entry.source;
+                plan.source_roots
+                    .iter()
+                    .any(|r| source.starts_with(&r.path) && !root_is_there(&r.path, r.device))
+                    .then_some(FatalError::SourceGone)
+            }
+            _ => None,
+        }
+    }
+
     fn finish(&self, slot: Option<&Arc<Slot>>, outcome: FileOutcome) {
         if let Some(slot) = slot {
             self.active
@@ -357,12 +390,12 @@ impl<'a> Runner<'a> {
         }
         self.files_done.fetch_add(1, Relaxed);
         if let FileStatus::Failed(e) = &outcome.status
-            && e.is_fatal()
+            && let Some(fatal) = self.fatal_cause(outcome.id, e)
         {
             self.fatal
                 .lock()
                 .expect("fatal lock poisoned")
-                .get_or_insert(e.clone());
+                .get_or_insert(fatal);
             self.control.cancel();
         }
         self.outcomes
@@ -372,3 +405,9 @@ impl<'a> Runner<'a> {
         (self.on_event)(Event::FileFinished(outcome));
     }
 }
+
+/// The folder still exists, as a folder, on the same volume.
+fn root_is_there(path: &Path, device: u64) -> bool {
+    fs::metadata(path).is_ok_and(|m| m.is_dir())
+        && fsinfo::device_id(path).is_ok_and(|d| d == device)
+}
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): stop the job when the source or destination goes away

After an I/O error the source or destination root is checked again; if
it is gone or its volume changed the job stops (FR-21). A file whose
size changed while it was read fails with SourceChanged.
EOF
```


---

### Task 13: Keep the system awake

`JobOptions::keep_awake` (default on) blocks idle sleep for the whole job, pauses included;
the display may sleep. The `keepawake` crate pulls `zbus` into every Linux build, so the
engine does it directly:
- macOS: `caffeinate -i -w <pid>`
- Linux: `systemd-inhibit --what=idle … tail --pid=<pid> -f /dev/null`
- Windows: `SetThreadExecutionState` on the job's thread (`KeepAwake` is not `Send`, so it
  ends on that thread)

On macOS and Linux the helper watches the app's pid, so it ends if the app crashes. Without
the helper the job still runs.

**Files:**
- Create: `crates/secopy-core/src/awake.rs`
- Modify: `Cargo.toml`, `crates/secopy-core/src/job/mod.rs`, `crates/secopy-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `awake::KeepAwake { new(), is_active() }` (releases on drop); `JobOptions::keep_awake: bool`.

- [ ] **Step 1: Write the failing tests**

Create `crates/secopy-core/src/awake.rs` with its unit tests; the implementation goes above them in the implementation step:

```rust
#[cfg(all(test, any(target_os = "macos", windows)))]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_accepted() {
        assert!(KeepAwake::new().is_active());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn caffeinate_ends_when_released() {
        let awake = KeepAwake::new();
        let pid = awake.child.as_ref().unwrap().id().to_string();
        drop(awake);
        let alive = Command::new("kill")
            .args(["-0", &pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(!alive);
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --lib awake`
Expected: compile error: module `awake` doesn't exist.

- [ ] **Step 3: Implement**

Change `Cargo.toml`:

```diff
diff --git a/Cargo.toml b/Cargo.toml
index c59a180..8f798f3 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -17,5 +17,5 @@ libc = "0.2"
 tempfile = "3.27.0"
 thiserror = "2.0.21"
 walkdir = "2.5.0"
-windows-sys = { version = "0.61.2", features = ["Win32_Foundation", "Win32_Storage_FileSystem"] }
+windows-sys = { version = "0.61.2", features = ["Win32_Foundation", "Win32_Storage_FileSystem", "Win32_System_Power"] }
 xxhash-rust = { version = "0.8.18", features = ["xxh64"] }
```

Add the implementation at the top of `crates/secopy-core/src/awake.rs`, above the tests:

```rust
//! Keeps the system from sleeping while a job runs (RFD §5.3). Best effort: without the
//! OS mechanism the job still runs. Only idle sleep is blocked; the display may sleep.

use std::marker::PhantomData;
#[cfg(unix)]
use std::process::{Child, Command, Stdio};

/// Holds the "stay awake" request until dropped. Not `Send`: on Windows the request
/// belongs to the thread that made it, so it must end on that thread too.
pub struct KeepAwake {
    #[cfg(unix)]
    child: Option<Child>,
    #[cfg(windows)]
    active: bool,
    _not_send: PhantomData<*const ()>,
}

impl KeepAwake {
    /// macOS: `caffeinate -i`. Linux: a logind idle inhibitor via `systemd-inhibit`.
    /// Both helpers watch this process, so they also end if it crashes.
    #[cfg(unix)]
    pub fn new() -> Self {
        let pid = std::process::id().to_string();
        let mut cmd = if cfg!(target_os = "macos") {
            let mut cmd = Command::new("caffeinate");
            cmd.args(["-i", "-w", &pid]);
            cmd
        } else {
            let mut cmd = Command::new("systemd-inhibit");
            cmd.args([
                "--what=idle",
                "--who=Secopy",
                "--why=Copying files",
                "--mode=block",
                "tail",
                &format!("--pid={pid}"),
                "-f",
                "/dev/null",
            ]);
            cmd
        };
        let child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        Self {
            child,
            _not_send: PhantomData,
        }
    }

    #[cfg(windows)]
    pub fn new() -> Self {
        use windows_sys::Win32::System::Power::{
            ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
        };
        // SAFETY: plain flags; returns 0 on failure.
        let active = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) } != 0;
        Self {
            active,
            _not_send: PhantomData,
        }
    }

    /// Whether the request was accepted.
    pub fn is_active(&self) -> bool {
        #[cfg(unix)]
        return self.child.is_some();
        #[cfg(windows)]
        return self.active;
    }
}

impl Default for KeepAwake {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        #[cfg(windows)]
        if self.active {
            use windows_sys::Win32::System::Power::{ES_CONTINUOUS, SetThreadExecutionState};
            // SAFETY: clears this thread's request.
            unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
        }
    }
}
```

Change `crates/secopy-core/src/job/mod.rs`:

```diff
diff --git a/crates/secopy-core/src/job/mod.rs b/crates/secopy-core/src/job/mod.rs
index 838fb04..2125a97 100644
--- a/crates/secopy-core/src/job/mod.rs
+++ b/crates/secopy-core/src/job/mod.rs
@@ -12,6 +12,7 @@ use std::time::{Duration, Instant};
 
 use chrono::Local;
 
+use crate::awake::KeepAwake;
 use crate::checksum_file;
 use crate::copy::CopyConfig;
 use crate::error::{FatalError, FileError};
@@ -36,6 +37,8 @@ pub struct JobOptions {
     pub small_file_lanes: usize,
     pub large_file_lanes: usize,
     pub verify_lanes: usize,
+    /// Keep the system from sleeping while the job runs, pauses included.
+    pub keep_awake: bool,
     /// How often `Event::Progress` is emitted.
     pub progress_interval: Duration,
     #[doc(hidden)]
@@ -54,6 +57,7 @@ impl Default for JobOptions {
             small_file_lanes: 8,
             large_file_lanes: 1,
             verify_lanes: 2,
+            keep_awake: true,
             progress_interval: Duration::from_millis(50),
             hooks: Hooks::default(),
         }
@@ -157,6 +161,7 @@ pub fn run_job(
     on_event: &(dyn Fn(Event) + Sync),
 ) -> JobReport {
     let started = Instant::now();
+    let _awake = opts.keep_awake.then(KeepAwake::new);
     let dest = plan.dest.as_path();
     let runner = Runner::new(plan, opts, control, on_event);
     let (write, unwritten): (Vec<usize>, Vec<usize>) =
```

Change `crates/secopy-core/src/lib.rs`:

```diff
diff --git a/crates/secopy-core/src/lib.rs b/crates/secopy-core/src/lib.rs
index af39828..5e75809 100644
--- a/crates/secopy-core/src/lib.rs
+++ b/crates/secopy-core/src/lib.rs
@@ -1,6 +1,7 @@
 //! Secopy engine: scan, copy, verify and write checksum files (RFD 0001).
 //! UI-independent; used by the desktop app, the CLI, tests and benchmarks.
 
+pub mod awake;
 pub mod checksum_file;
 pub mod control;
 pub mod copy;
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

Also lint the other platforms' `cfg` code (install the targets once with
`rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`):
`cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` and
`cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`.
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): keep the system awake during a job

Idle sleep is blocked while a job runs, pauses included: caffeinate on
macOS, systemd-inhibit on Linux, SetThreadExecutionState on Windows.
Best effort; the keepawake crate would pull zbus into every Linux build.
EOF
```


---

### Task 14: Job report (FR-35)

`Report::new(&Plan, &JobReport, &JobMeta)` builds the report from string-based types,
because serde's `PathBuf` fails on names that aren't UTF-8. That also keeps the JSON schema
stable for tools. It contains:
- settings and times
- a one-line result
- counts
- cache bypass
- the checksum file
- partial files removed
- one row per file in plan order, not-started files included, with reasons

Output: `to_text()`, `to_json()`, and `write_next_to(checksum_file)` / `write(dir, stem)`,
which never overwrite. Adds `serde` and `serde_json`.

**Files:**
- Create: `crates/secopy-core/src/report.rs`, `crates/secopy-core/tests/report.rs`
- Modify: `Cargo.toml`, `crates/secopy-core/Cargo.toml`, `crates/secopy-core/src/lib.rs`
- Generated: `Cargo.lock` (Cargo updates it; commit it)

**Interfaces:**
- Consumes: Tasks 10–12's outcomes.
- Produces: `report::{Report { app_version, mode, source, destination, started, finished, duration_secs, result, counts, cache_bypass, checksum_file, checksum_error, removed_partials, files }, Counts, ReportFile { path, copied_to, size, status, reason, xxh64, in_checksum_file }, JobMeta { app_version, source, verify, started, finished }}`;
  `Report::{new, to_text, to_json, write_next_to}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/secopy-core/tests/report.rs`:

```rust
mod common;

use std::fs;
use std::path::Path;

use chrono::Local;
use common::{pattern, write_files};
use secopy_core::filter::ExtensionFilter;
use secopy_core::job::{Hooks, JobControl, JobOptions, run_job};
use secopy_core::plan::{DiffersPolicy, Plan};
use secopy_core::preflight::preflight;
use secopy_core::report::{JobMeta, Report};
use secopy_core::scan::{ScanOptions, scan};
use secopy_core::source::{DirMode, Source};

/// CARD/ with a file that fails verification, one already at the destination, and
/// one that gets renamed (Keep both).
fn job() -> (tempfile::TempDir, Report) {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    write_files(
        &src,
        &[
            ("bad.mov", &pattern(100)),
            ("same.wav", b"same"),
            ("new.txt", b"new"),
        ],
    );
    write_files(&dest, &[("CARD/new.txt", b"older and different")]);
    // same.wav: identical copy already there.
    fs::copy(src.join("same.wav"), dest.join("CARD/same.wav")).unwrap();
    let mtime = fs::metadata(src.join("same.wav"))
        .unwrap()
        .modified()
        .unwrap();
    fs::File::options()
        .write(true)
        .open(dest.join("CARD/same.wav"))
        .unwrap()
        .set_modified(mtime)
        .unwrap();

    let source = Source::Directory {
        path: src,
        mode: DirMode::FolderItself,
    };
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &dest).unwrap();
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    let opts = JobOptions {
        hooks: Hooks {
            before_copy: None,
            after_copy: Some(|p: &Path, _| {
                if p.to_string_lossy().contains("bad") {
                    let mut data = fs::read(p).unwrap();
                    data[0] ^= 0xFF;
                    fs::write(p, data).unwrap();
                }
            }),
        },
        ..JobOptions::default()
    };
    let started = Local::now();
    let job = run_job(&plan, &opts, &JobControl::new(), &|_| {});
    let meta = JobMeta {
        app_version: "0.2.0".into(),
        source: "/Volumes/CARD".into(),
        verify: true,
        started,
        finished: Local::now(),
    };
    let report = Report::new(&plan, &job, &meta);
    (dir, report)
}

#[test]
fn counts_and_statuses_are_complete() {
    let (_dir, r) = job();
    assert_eq!(r.result, "1 file failed");
    assert_eq!(r.mode, "copy+verify");
    assert_eq!(r.counts.files, 3);
    assert_eq!(r.counts.verified, 1);
    assert_eq!(r.counts.skipped_identical, 1);
    assert_eq!(r.counts.failed, 1);
    assert_eq!(r.counts.bytes_written, 3);
    assert_eq!(r.cache_bypass, Some(true));

    let file = |name: &str| r.files.iter().find(|f| f.path.ends_with(name)).unwrap();
    assert_eq!(file("bad.mov").status, "failed");
    assert!(
        file("bad.mov")
            .reason
            .as_deref()
            .unwrap()
            .contains("hash mismatch")
    );
    assert_eq!(file("same.wav").status, "skipped");
    assert!(!file("same.wav").in_checksum_file);
    assert_eq!(
        file("new.txt").copied_to.as_deref(),
        Some("CARD/new (1).txt")
    );
    assert!(file("new.txt").in_checksum_file);
    assert_eq!(file("new.txt").xxh64.as_ref().unwrap().len(), 16);
}

#[test]
fn json_is_valid_and_has_every_file() {
    let (_dir, r) = job();
    let v: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
    assert_eq!(v["counts"]["failed"], 1);
    assert_eq!(v["files"].as_array().unwrap().len(), 3);
    assert_eq!(v["files"][0]["path"], "CARD/bad.mov");
}

#[test]
fn text_lists_the_result_and_the_problems() {
    let (_dir, r) = job();
    let text = r.to_text();
    assert!(text.contains("Result:       1 file failed"), "{text}");
    assert!(text.contains("Mode:         Copy & Verify"), "{text}");
    assert!(
        text.contains("1 skipped, already at the destination (not checked)"),
        "{text}"
    );
    assert!(
        text.contains("PROBLEMS\n  CARD/bad.mov: hash mismatch"),
        "{text}"
    );
    assert!(text.contains("CARD/new.txt -> CARD/new (1).txt"), "{text}");
}

#[test]
fn reports_are_saved_next_to_the_checksum_file_and_never_overwritten() {
    let (_dir, r) = job();
    let checksum = Path::new(r.checksum_file.as_ref().unwrap()).to_path_buf();
    let (text, json) = r.write_next_to(&checksum).unwrap();
    let stem = checksum.file_stem().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        text.file_name().unwrap().to_string_lossy(),
        format!("{stem}_report.txt")
    );
    assert_eq!(
        json.file_name().unwrap().to_string_lossy(),
        format!("{stem}_report.json")
    );
    assert_eq!(fs::read_to_string(&text).unwrap(), r.to_text());
    assert!(r.write_next_to(&checksum).is_err());
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-core --test report`
Expected: compile error: module `report` doesn't exist.

- [ ] **Step 3: Implement**

Change `Cargo.toml`:

```diff
diff --git a/Cargo.toml b/Cargo.toml
index 8f798f3..4b8f590 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -14,6 +14,8 @@ chrono = { version = "0.4.45", default-features = false, features = ["clock"] }
 clap = { version = "4.6.7", features = ["derive"] }
 ctrlc = "3.5.2"
 libc = "0.2"
+serde = { version = "1.0.229", features = ["derive"] }
+serde_json = "1.0.151"
 tempfile = "3.27.0"
 thiserror = "2.0.21"
 walkdir = "2.5.0"
```

Change `crates/secopy-core/Cargo.toml`:

```diff
diff --git a/crates/secopy-core/Cargo.toml b/crates/secopy-core/Cargo.toml
index febbd7c..1021224 100644
--- a/crates/secopy-core/Cargo.toml
+++ b/crates/secopy-core/Cargo.toml
@@ -8,6 +8,8 @@ publish.workspace = true
 
 [dependencies]
 chrono.workspace = true
+serde.workspace = true
+serde_json.workspace = true
 thiserror.workspace = true
 walkdir.workspace = true
 xxhash-rust.workspace = true
```

Change `crates/secopy-core/src/lib.rs`:

```diff
diff --git a/crates/secopy-core/src/lib.rs b/crates/secopy-core/src/lib.rs
index 5e75809..d1073cf 100644
--- a/crates/secopy-core/src/lib.rs
+++ b/crates/secopy-core/src/lib.rs
@@ -16,6 +16,7 @@ pub mod names;
 mod os;
 pub mod plan;
 pub mod preflight;
+pub mod report;
 pub mod scan;
 pub mod source;
 pub mod verify;
```

Create `crates/secopy-core/src/report.rs`:

```rust
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
        let dir = checksum_file.parent().unwrap_or(Path::new("."));
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
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): write the job report as text and json

Plain text and JSON: settings, times, result, counts, cache bypass,
removed partial files and one row per file with the reason for every
skip and failure. It can be saved next to the checksum file.
EOF
```


---

### Task 15: Cache-bypass check (FR-26)

Plan 1 left open whether `F_NOCACHE` writes from unaligned buffers leave the file's last
page in the macOS page cache, so a verify read of that page would come from RAM. The
prototype measured it with `mincore`. After an `F_NOCACHE` copy no page is resident, a
partial last page included, for both the single-read and the pipelined path. On Linux,
`open_uncached`'s `fadvise(DONTNEED)` after the `fsync` empties the cache. No eviction code
is needed; these tests keep it that way.

**Files:**
- Modify: `crates/secopy-core/src/os.rs`

**Interfaces:**
- Consumes: `copy_to_partial`, `os::open_uncached`.
- Produces: tests only.

- [ ] **Step 1: Write the tests**

Update the tests in `crates/secopy-core/src/os.rs`:

```diff
diff --git a/crates/secopy-core/src/os.rs b/crates/secopy-core/src/os.rs
index 170e779..7cfcc1a 100644
--- a/crates/secopy-core/src/os.rs
+++ b/crates/secopy-core/src/os.rs
@@ -265,3 +265,78 @@ fn lock(file: &File, wait: bool) -> io::Result<bool> {
         Err(e)
     }
 }
+
+#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
+mod tests {
+    use super::*;
+    use std::os::fd::AsRawFd;
+
+    /// Pages of `path` that are in the OS page cache, via `mincore` on a mapping.
+    fn resident_pages(path: &Path) -> usize {
+        let file = File::open(path).unwrap();
+        let len = file.metadata().unwrap().len() as usize;
+        // SAFETY: `sysconf` has no preconditions.
+        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
+        let mut vec = vec![0u8; len.div_ceil(page)];
+        // SAFETY: a read-only shared mapping of a file we own, unmapped below; `vec` has
+        // one byte per page, as `mincore` requires.
+        unsafe {
+            let addr = libc::mmap(
+                std::ptr::null_mut(),
+                len,
+                libc::PROT_READ,
+                libc::MAP_SHARED,
+                file.as_raw_fd(),
+                0,
+            );
+            assert_ne!(addr, libc::MAP_FAILED);
+            assert_eq!(libc::mincore(addr, len, vec.as_mut_ptr().cast()), 0);
+            libc::munmap(addr, len);
+        }
+        vec.iter().filter(|&&b| b & 1 != 0).count()
+    }
+
+    /// FR-26: after an uncached copy, no page of the file (including a partial last
+    /// page) is left in RAM for the verify read to hit.
+    #[cfg(target_os = "macos")]
+    #[test]
+    fn uncached_copies_leave_nothing_in_the_page_cache() {
+        use crate::control::JobControl;
+        use crate::copy::{CopyConfig, copy_to_partial};
+        let dir = tempfile::tempdir().unwrap();
+        // One file for the single-read path, one for the pipeline; both end mid-page.
+        for (name, len, buffer_size) in [("small", 50_000, 1 << 20), ("large", 300_123, 65_536)] {
+            let src = dir.path().join(name);
+            std::fs::write(&src, vec![7u8; len]).unwrap();
+            let cfg = CopyConfig {
+                buffer_size,
+                buffers: 3,
+                uncached_write: true,
+            };
+            let dst = dir.path().join(format!("{name}.copy"));
+            let pc = copy_to_partial(&src, &dst, &cfg, &|_| {}, &JobControl::new()).unwrap();
+            assert_eq!(resident_pages(&pc.partial), 0, "{name}");
+        }
+    }
+
+    /// FR-26: `open_uncached` evicts the fsynced file's pages before the verify read.
+    #[cfg(target_os = "linux")]
+    #[test]
+    fn opening_for_verify_evicts_the_page_cache() {
+        let dir = tempfile::tempdir().unwrap();
+        if crate::fsinfo::fs_info(dir.path()).unwrap().kind
+            == crate::fsinfo::FsKind::Other("0x1021994".into())
+        {
+            return; // tmpfs: the page cache is the storage
+        }
+        let path = dir.path().join("a.bin");
+        let mut f = File::create(&path).unwrap();
+        std::io::Write::write_all(&mut f, &vec![7u8; 300_123]).unwrap();
+        f.sync_all().unwrap();
+        drop(f);
+        assert!(resident_pages(&path) > 0, "a normal write is cached");
+        let (_file, bypassed) = open_uncached(&path).unwrap();
+        assert!(bypassed);
+        assert_eq!(resident_pages(&path), 0);
+    }
+}
```

- [ ] **Step 2: Run them**

Run: `cargo test -p secopy-core --lib os::`
Expected: these pass straight away. They pin the measured behaviour, so there is no red step. If one fails on your machine, FR-26 is broken there: stop and report it.

- [ ] **Step 3: Nothing to implement**

The behaviour already exists; this task only adds the guard.

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

Also lint the other platforms' `cfg` code (install the targets once with
`rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`):
`cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` and
`cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`.
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
test(core): guard the verify cache bypass with page residency checks

Measured with mincore: after an F_NOCACHE copy no page of the file is
cached on macOS, a partial last page included, and open_uncached empties
the cache on Linux. The tests keep verify reads coming from the device
(plan 1 carry-over).
EOF
```


---

### Task 16: Fault tests on real volumes (macOS RAM disks)

`hdiutil attach -nomount ram://…` plus `diskutil erasevolume` gives real volumes without
root. The tests cover:
- a 4 MiB exFAT disk that fills up → `DiskFull`, no partial files left
- a forced detach while the job is paused → `DestinationGone`
- a FAT32 volume → pre-flight finds an invalid name and a 4 GiB sparse file
- exFAT, which has neither a no-replace rename nor hard links → the fallback commit copies
  and verifies
- case-sensitive APFS → `a.txt` and `A.TXT` both copied

Gated behind `SECOPY_DEVICE_TESTS=1`; CI's macOS job sets it. `diskutil` fails with
"Resource busy" when two erases overlap, so disk creation is serialized. FAT volume names
are 11 characters or less.

**Files:**
- Create: `crates/secopy-core/tests/devices.rs`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Tasks 7–12.
- Produces: tests only.

- [ ] **Step 1: Write the tests**

Create `crates/secopy-core/tests/devices.rs`:

```rust
//! Fault injection on real (RAM-disk) volumes: disk full, unplugging, FAT and exFAT
//! rules, case-sensitive APFS (RFD §9). macOS only, where `hdiutil` needs no root.
//! Run with `SECOPY_DEVICE_TESTS=1`; CI's macOS job sets it.
#![cfg(target_os = "macos")]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use common::{pattern, read_tree, write_files};
use secopy_core::error::FatalError;
use secopy_core::filter::ExtensionFilter;
use secopy_core::fsinfo::FsKind;
use secopy_core::job::{Event, FileStatus, JobControl, JobOptions, run_job};
use secopy_core::plan::{DiffersPolicy, Plan};
use secopy_core::preflight::{ProblemKind, preflight};
use secopy_core::scan::{ScanOptions, scan};
use secopy_core::source::{DirMode, Source};

fn enabled() -> bool {
    std::env::var("SECOPY_DEVICE_TESTS").is_ok_and(|v| v == "1")
}

/// A RAM disk formatted with `format` and mounted under /Volumes; detached on drop.
struct RamDisk {
    dev: String,
    mount: PathBuf,
}

impl RamDisk {
    /// `format` as `diskutil` names it; `mib` in MiB.
    fn new(format: &str, mib: u32) -> RamDisk {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        // `diskutil` fails with "Resource busy" when two erases overlap.
        static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
        let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let out = Command::new("hdiutil")
            .args(["attach", "-nomount", &format!("ram://{}", mib * 2048)])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let dev = String::from_utf8(out.stdout).unwrap().trim().to_string();
        // FAT volume names: at most 11 characters, upper case.
        let name = format!(
            "SCT{}{}",
            std::process::id() % 10_000,
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let status = Command::new("diskutil")
            .args(["erasevolume", format, &name, &dev])
            .output()
            .unwrap();
        let disk = RamDisk {
            dev,
            mount: PathBuf::from("/Volumes").join(&name),
        };
        assert!(status.status.success(), "{status:?}");
        disk
    }

    fn unplug(&self) {
        let out = Command::new("hdiutil")
            .args(["detach", "-force", &self.dev])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    }
}

impl Drop for RamDisk {
    fn drop(&mut self) {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force", &self.dev])
            .output();
    }
}

fn plan_for(source: &Source, dest: &Path) -> Plan {
    let sel = scan(source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(source, &sel, dest).unwrap();
    Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth)
}

fn card(root: &Path, files: &[(&str, &[u8])]) -> Source {
    let src = root.join("CARD");
    write_files(&src, files);
    Source::Directory {
        path: src,
        mode: DirMode::FolderItself,
    }
}

fn no_partials(root: &Path) -> bool {
    read_tree(root)
        .keys()
        .all(|k| !k.contains("secopy-partial"))
}

#[test]
fn a_full_disk_stops_the_job_and_leaves_no_partial_files() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("ExFAT", 4);
    let dir = tempfile::tempdir().unwrap();
    let big = pattern(1 << 20);
    let files: Vec<(String, &[u8])> = (0..8)
        .map(|i| (format!("clip{i}.mov"), big.as_slice()))
        .collect();
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    let plan = plan_for(&card(dir.path(), &files), &disk.mount);
    assert!(!plan.blockers().is_empty(), "pre-flight sees it won't fit");

    // Started anyway, as if the disk filled up after pre-flight.
    let report = run_job(&plan, &JobOptions::default(), &JobControl::new(), &|_| {});
    assert_eq!(report.fatal, Some(FatalError::DiskFull), "{report:?}");
    assert!(no_partials(&disk.mount));
}

#[test]
fn unplugging_the_destination_stops_the_job() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("ExFAT", 16);
    let dir = tempfile::tempdir().unwrap();
    let data = pattern(512 << 10);
    let files: Vec<(String, &[u8])> = (0..10)
        .map(|i| (format!("clip{i}.mov"), data.as_slice()))
        .collect();
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    let plan = plan_for(&card(dir.path(), &files), &disk.mount);
    let control = JobControl::new();
    let opts = JobOptions {
        small_file_lanes: 1,
        ..JobOptions::default()
    };
    let report = std::thread::scope(|s| {
        let job = s.spawn(|| {
            run_job(&plan, &opts, &control, &|e| {
                if let Event::FileFinished(_) = e {
                    control.pause();
                }
            })
        });
        while !control.is_paused() {
            std::thread::yield_now();
        }
        disk.unplug();
        control.resume();
        job.join().unwrap()
    });
    assert_eq!(
        report.fatal,
        Some(FatalError::DestinationGone),
        "{report:?}"
    );
    assert!(report.not_started > 0);
}

#[test]
fn fat32_limits_are_found_in_preflight() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("MS-DOS FAT32", 40);
    let dir = tempfile::tempdir().unwrap();
    let source = card(dir.path(), &[("ok.mov", b"ok"), ("a:b.mov", b"colon")]);
    // A sparse file over 4 GiB: takes no space on APFS.
    fs::File::create(dir.path().join("CARD/huge.mov"))
        .unwrap()
        .set_len(4 << 30)
        .unwrap();
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &disk.mount).unwrap();
    assert_eq!(pf.fs.kind, FsKind::Fat);
    assert!(!pf.fs.case_sensitive);
    let problem = |name: &str| {
        let id = sel
            .files
            .iter()
            .position(|f| f.rel.ends_with(name))
            .unwrap();
        pf.file_problems
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.kind.clone())
    };
    assert!(matches!(
        problem("a:b.mov"),
        Some(ProblemKind::InvalidName(_))
    ));
    assert!(matches!(
        problem("huge.mov"),
        Some(ProblemKind::TooLarge { .. })
    ));
    assert_eq!(problem("ok.mov"), None);
}

/// exFAT has no no-replace rename and no hard links: commits take the fallback path.
#[test]
fn exfat_copies_and_verifies_through_the_fallback_commit() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("ExFAT", 16);
    let dir = tempfile::tempdir().unwrap();
    let source = card(
        dir.path(),
        &[("A001.mov", &pattern(100_000)), ("clips/B002.mov", b"b")],
    );
    let plan = plan_for(&source, &disk.mount);
    assert_eq!(plan.fs.kind, FsKind::ExFat);
    let report = run_job(&plan, &JobOptions::default(), &JobControl::new(), &|_| {});
    assert!(report.is_success(), "{report:?}");
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| o.status == FileStatus::Verified)
    );
    let copied = read_tree(&disk.mount.join("CARD"));
    assert_eq!(copied.get("A001.mov"), Some(&pattern(100_000)));
    assert!(no_partials(&disk.mount));
}

#[test]
fn case_sensitive_apfs_copies_names_that_differ_only_in_case() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("Case-sensitive APFS", 40);
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &[("x/a.txt", b"lower"), ("y/A.TXT", b"upper")]);
    let source = Source::Files(vec![dir.path().join("x/a.txt"), dir.path().join("y/A.TXT")]);
    let plan = plan_for(&source, &disk.mount);
    assert!(plan.fs.case_sensitive);
    let report = run_job(&plan, &JobOptions::default(), &JobControl::new(), &|_| {});
    assert!(report.is_success(), "{report:?}");
    assert_eq!(fs::read(disk.mount.join("a.txt")).unwrap(), b"lower");
    assert_eq!(fs::read(disk.mount.join("A.TXT")).unwrap(), b"upper");
}
```

- [ ] **Step 2: Run them**

Run: `SECOPY_DEVICE_TESTS=1 cargo test -p secopy-core --test devices`
Expected: all five pass (about 12 s). Like Task 15, they pin behaviour built in earlier tasks. Without the variable they return immediately.

- [ ] **Step 3: Nothing to implement**

Check that no `/Volumes/SCT*` volume is left mounted afterwards (`ls /Volumes`).

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit, then run them in CI**

```bash
git add -A
git commit -F - <<'EOF'
test(core): run fault tests on macos ram disks

Disk full, unplugging mid-job, FAT32 and exFAT rules and the exFAT
fallback commit, and case-sensitive APFS, on real volumes (RFD §9).
Gated behind SECOPY_DEVICE_TESTS=1.
EOF
```


Change `.github/workflows/ci.yml`:

```diff
diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml
index 1b9a0b4..d9a6159 100644
--- a/.github/workflows/ci.yml
+++ b/.github/workflows/ci.yml
@@ -39,3 +39,5 @@ jobs:
         env:
           # The xxhsum compatibility test must not skip where xxhsum is installed.
           SECOPY_REQUIRE_XXHSUM: ${{ runner.os != 'Windows' && '1' || '0' }}
+          # RAM-disk fault tests (disk full, unplugging, FAT/exFAT) run on macOS only.
+          SECOPY_DEVICE_TESTS: ${{ runner.os == 'macOS' && '1' || '0' }}
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
ci: run device fault tests on macos

The macOS runner can create RAM disks without root.
EOF
```


---

### Task 17: CLI: conflict choice, pre-flight output, report files

New flags:
- `--on-conflict keep-both|overwrite|skip` (default `keep-both`)
- `--report <DIR>`: writes the text and JSON report there, named after the checksum file

Before copying, the CLI prints files that will fail, how many identical files will be
skipped, how many different files share a name, and leftover partial files. The summary
line counts skipped files. A blocker still exits with 2.

**Files:**
- Modify: `crates/secopy-cli/Cargo.toml`, `crates/secopy-cli/src/main.rs`, `crates/secopy-cli/tests/cli.rs`, `crates/secopy-core/src/report.rs`
- Generated: `Cargo.lock` (Cargo updates it; commit it)

**Interfaces:**
- Consumes: Tasks 7, 8, 14.
- Produces: `Report::write(dir, stem)`; CLI flags above.

- [ ] **Step 1: Write the failing tests**

Update the tests in `crates/secopy-cli/tests/cli.rs`:

```diff
diff --git a/crates/secopy-cli/tests/cli.rs b/crates/secopy-cli/tests/cli.rs
index 19297ee..fc2b87e 100644
--- a/crates/secopy-cli/tests/cli.rs
+++ b/crates/secopy-cli/tests/cli.rs
@@ -32,7 +32,10 @@ fn copies_a_folder_with_verify_and_exits_zero() {
         b"movie"
     );
     let stdout = String::from_utf8_lossy(&out.stdout);
-    assert!(stdout.contains("1 files ok, 0 failed"), "{stdout}");
+    assert!(
+        stdout.contains("1 files ok, 0 skipped, 0 failed"),
+        "{stdout}"
+    );
     assert!(stdout.contains("checksum file:"), "{stdout}");
 }
 
@@ -101,3 +104,80 @@ fn a_missing_destination_is_a_usage_error() {
         .unwrap();
     assert_eq!(out.status.code(), Some(2));
 }
+
+fn copy_one_file(dir: &std::path::Path, extra: &[&str]) -> std::process::Output {
+    let dest = dir.join("dest");
+    fs::create_dir_all(&dest).unwrap();
+    cli()
+        .arg(dir.join("a.wav"))
+        .arg("--to")
+        .arg(&dest)
+        .args(extra)
+        .output()
+        .unwrap()
+}
+
+#[test]
+fn conflicts_keep_both_by_default() {
+    let dir = tempfile::tempdir().unwrap();
+    fs::write(dir.path().join("a.wav"), b"new").unwrap();
+    fs::create_dir_all(dir.path().join("dest")).unwrap();
+    fs::write(dir.path().join("dest/a.wav"), b"old one").unwrap();
+    let out = copy_one_file(dir.path(), &[]);
+    assert!(
+        out.status.success(),
+        "{}",
+        String::from_utf8_lossy(&out.stderr)
+    );
+    assert_eq!(fs::read(dir.path().join("dest/a.wav")).unwrap(), b"old one");
+    assert_eq!(fs::read(dir.path().join("dest/a (1).wav")).unwrap(), b"new");
+}
+
+#[test]
+fn on_conflict_skip_and_overwrite() {
+    let dir = tempfile::tempdir().unwrap();
+    fs::write(dir.path().join("a.wav"), b"new").unwrap();
+    fs::create_dir_all(dir.path().join("dest")).unwrap();
+    fs::write(dir.path().join("dest/a.wav"), b"old one").unwrap();
+
+    let out = copy_one_file(dir.path(), &["--on-conflict", "skip"]);
+    assert!(out.status.success());
+    assert_eq!(fs::read(dir.path().join("dest/a.wav")).unwrap(), b"old one");
+    let stdout = String::from_utf8_lossy(&out.stdout);
+    assert!(stdout.contains("0 files ok, 1 skipped"), "{stdout}");
+
+    let out = copy_one_file(dir.path(), &["--on-conflict", "overwrite", "--verify"]);
+    assert!(out.status.success());
+    assert_eq!(fs::read(dir.path().join("dest/a.wav")).unwrap(), b"new");
+}
+
+#[test]
+fn a_second_run_skips_what_the_first_copied() {
+    let dir = tempfile::tempdir().unwrap();
+    fs::write(dir.path().join("a.wav"), b"new").unwrap();
+    assert!(copy_one_file(dir.path(), &[]).status.success());
+    let out = copy_one_file(dir.path(), &[]);
+    assert!(out.status.success());
+    let stderr = String::from_utf8_lossy(&out.stderr);
+    assert!(
+        stderr.contains("1 files already at the destination will be skipped"),
+        "{stderr}"
+    );
+}
+
+#[test]
+fn report_flag_writes_text_and_json() {
+    let dir = tempfile::tempdir().unwrap();
+    fs::write(dir.path().join("a.wav"), b"new").unwrap();
+    let reports = dir.path().join("reports");
+    fs::create_dir_all(&reports).unwrap();
+    let out = copy_one_file(dir.path(), &["--report", reports.to_str().unwrap()]);
+    assert!(out.status.success());
+    let names: Vec<String> = fs::read_dir(&reports)
+        .unwrap()
+        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
+        .collect();
+    assert_eq!(names.len(), 2, "{names:?}");
+    assert!(names.iter().any(|n| n.ends_with("_report.txt")));
+    assert!(names.iter().any(|n| n.ends_with("_report.json")));
+}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p secopy-cli`
Expected: the new tests fail: unknown flags `--on-conflict` and `--report`, and the summary has no `skipped`.

- [ ] **Step 3: Implement**

Change `crates/secopy-cli/Cargo.toml`:

```diff
diff --git a/crates/secopy-cli/Cargo.toml b/crates/secopy-cli/Cargo.toml
index eedb46d..266179c 100644
--- a/crates/secopy-cli/Cargo.toml
+++ b/crates/secopy-cli/Cargo.toml
@@ -7,6 +7,7 @@ rust-version.workspace = true
 publish.workspace = true
 
 [dependencies]
+chrono.workspace = true
 clap.workspace = true
 ctrlc.workspace = true
 secopy-core.workspace = true
```

Change `crates/secopy-cli/src/main.rs`:

```diff
diff --git a/crates/secopy-cli/src/main.rs b/crates/secopy-cli/src/main.rs
index ae3030d..3daa5f8 100644
--- a/crates/secopy-cli/src/main.rs
+++ b/crates/secopy-cli/src/main.rs
@@ -5,11 +5,14 @@ use std::process::ExitCode;
 use std::sync::{Arc, Mutex};
 use std::time::{Duration, Instant};
 
-use clap::Parser;
+use chrono::Local;
+use clap::{Parser, ValueEnum};
+use secopy_core::checksum_file;
 use secopy_core::filter::ExtensionFilter;
 use secopy_core::job::{self, Event, FileStatus, JobControl, JobOptions, JobReport, Progress};
 use secopy_core::plan::{DiffersPolicy, Plan};
-use secopy_core::preflight::preflight;
+use secopy_core::preflight::{ConflictKind, Preflight, preflight};
+use secopy_core::report::{JobMeta, Report};
 use secopy_core::scan::{self, ScanOptions};
 use secopy_core::source::{DirMode, Source};
 
@@ -41,6 +44,30 @@ struct Args {
     /// Include hidden files and folders.
     #[arg(long)]
     include_hidden: bool,
+    /// What to do with files that already exist at the destination but differ.
+    /// Identical files (same size and date) are always skipped.
+    #[arg(long, value_enum, default_value_t = OnConflict::KeepBoth)]
+    on_conflict: OnConflict,
+    /// Also write the job report (text and JSON) into this folder.
+    #[arg(long, value_name = "DIR")]
+    report: Option<PathBuf>,
+}
+
+#[derive(Clone, Copy, Debug, ValueEnum)]
+enum OnConflict {
+    KeepBoth,
+    Overwrite,
+    Skip,
+}
+
+impl From<OnConflict> for DiffersPolicy {
+    fn from(c: OnConflict) -> Self {
+        match c {
+            OnConflict::KeepBoth => DiffersPolicy::KeepBoth,
+            OnConflict::Overwrite => DiffersPolicy::Overwrite,
+            OnConflict::Skip => DiffersPolicy::Skip,
+        }
+    }
 }
 
 fn main() -> ExitCode {
@@ -87,7 +114,8 @@ fn run(args: Args) -> Result<ExitCode, String> {
         scan.skipped_hidden
     );
     let pf = preflight(&source, &selection, &args.to).map_err(|e| e.to_string())?;
-    let plan = Plan::resolve(&selection, &pf, DiffersPolicy::KeepBoth);
+    let plan = Plan::resolve(&selection, &pf, args.on_conflict.into());
+    print_preflight(&pf, &plan);
     if let Some(blocker) = plan.blockers().first() {
         return Err(blocker.to_string());
     }
@@ -102,6 +130,7 @@ fn run(args: Args) -> Result<ExitCode, String> {
     ctrlc::set_handler(move || handler_control.cancel()).map_err(|e| e.to_string())?;
 
     let started = Instant::now();
+    let started_at = Local::now();
     let last_print = Mutex::new(Instant::now());
     let report = job::run_job(&plan, &opts, &control, &|event| match event {
         Event::Progress(p) => {
@@ -119,6 +148,33 @@ fn run(args: Args) -> Result<ExitCode, String> {
     });
     eprintln!();
     print_summary(&report, plan.bytes_to_write());
+    if let Some(dir) = &args.report {
+        let meta = JobMeta {
+            app_version: env!("CARGO_PKG_VERSION").to_string(),
+            source: args
+                .sources
+                .iter()
+                .map(|p| p.display().to_string())
+                .collect::<Vec<_>>()
+                .join(", "),
+            verify: args.verify,
+            started: started_at,
+            finished: Local::now(),
+        };
+        // Named like the checksum file, so the two are easy to pair up.
+        let stem = match &report.checksum_file {
+            Some(path) => path
+                .file_stem()
+                .unwrap_or_default()
+                .to_string_lossy()
+                .into_owned(),
+            None => checksum_file::file_name(started_at).replace(".xxh64", ""),
+        };
+        let (text, _) = Report::new(&plan, &report, &meta)
+            .write(dir, &stem)
+            .map_err(|e| format!("report not written: {e}"))?;
+        println!("report: {}", text.display());
+    }
     Ok(if report.is_success() {
         ExitCode::SUCCESS
     } else {
@@ -150,6 +206,36 @@ fn source_from(args: &Args) -> Result<Source, String> {
     Ok(Source::Files(args.sources.clone()))
 }
 
+/// Lists what pre-flight found, so nothing is decided silently (FR-16, FR-17).
+fn print_preflight(pf: &Preflight, plan: &Plan) {
+    for problem in &pf.file_problems {
+        let file = &plan.files[problem.id];
+        eprintln!(
+            "will fail: {}: {}",
+            file.entry.rel.display(),
+            problem.kind.to_error()
+        );
+    }
+    let identical = pf
+        .conflicts
+        .iter()
+        .filter(|c| c.kind == ConflictKind::Identical)
+        .count();
+    let differ = pf.conflicts.len() - identical;
+    if identical > 0 {
+        eprintln!("{identical} files already at the destination will be skipped (not checked)");
+    }
+    if differ > 0 {
+        eprintln!("{differ} different files have the same name at the destination");
+    }
+    if !pf.stale_partials.is_empty() {
+        eprintln!(
+            "{} partial files left by an interrupted copy will be replaced",
+            pf.stale_partials.len()
+        );
+    }
+}
+
 fn progress_line(p: &Progress, elapsed: Duration) -> String {
     let speed = p.copied_bytes as f64 / elapsed.as_secs_f64().max(0.001);
     format!(
@@ -167,9 +253,11 @@ fn progress_line(p: &Progress, elapsed: Duration) -> String {
 fn print_summary(report: &JobReport, total_bytes: u64) {
     let secs = report.elapsed.as_secs_f64().max(0.001);
     let failed = report.failed().count();
+    let skipped = report.skipped().count();
     println!(
-        "{} files ok, {} failed, {} not started",
-        report.outcomes.len() - failed,
+        "{} files ok, {} skipped, {} failed, {} not started",
+        report.outcomes.len() - failed - skipped,
+        skipped,
         failed,
         report.not_started
     );
```

Change `crates/secopy-core/src/report.rs`:

```diff
diff --git a/crates/secopy-core/src/report.rs b/crates/secopy-core/src/report.rs
index ccc541f..4d00b0a 100644
--- a/crates/secopy-core/src/report.rs
+++ b/crates/secopy-core/src/report.rs
@@ -259,7 +259,11 @@ impl Report {
             .file_stem()
             .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?
             .to_string_lossy();
-        let dir = checksum_file.parent().unwrap_or(Path::new("."));
+        self.write(checksum_file.parent().unwrap_or(Path::new(".")), &stem)
+    }
+
+    /// Saves `<dir>/<stem>_report.txt` and `.json`. Never overwrites.
+    pub fn write(&self, dir: &Path, stem: &str) -> io::Result<(PathBuf, PathBuf)> {
         let text = dir.join(format!("{stem}_report.txt"));
         let json = dir.join(format!("{stem}_report.json"));
         write_new(&text, &self.to_text())?;
```

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(cli): add conflict choice, pre-flight output and report files

--on-conflict keep-both|overwrite|skip and --report DIR; pre-flight
problems, identical and different files and leftover partial files are
printed before the copy starts.
EOF
```


---

### Task 18: Documentation and wrap-up

**Files:**
- Modify: `README.md`, `CLAUDE.md`, `docs/superpowers/plans/2026-09-26-v1-roadmap.md`

**Interfaces:**
- Consumes: everything above.
- Produces: docs only.

- [ ] **Step 1: Update the README's CLI example**

In `README.md`, replace the "Try the CLI" block with:

````markdown
Try the CLI:

```sh
cargo run --release -p secopy-cli -- /path/to/CARD --to /path/to/backup --verify
# files that already exist but differ: keep both (default), overwrite or skip
cargo run --release -p secopy-cli -- /path/to/CARD --to /path/to/backup --on-conflict skip
# also write the job report (text and JSON)
cargo run --release -p secopy-cli -- /path/to/CARD --to /path/to/backup --report /tmp
```
````

- [ ] **Step 2: Document the device tests in `CLAUDE.md`**

Under "## Development", after the `xxhsum` bullet, add:

```markdown
- Fault tests on real volumes (disk full, unplugging, FAT32/exFAT, case-sensitive APFS) use
  macOS RAM disks and run only with `SECOPY_DEVICE_TESTS=1`; CI's macOS job sets it. Run
  them locally before changing `copy`, `os`, `preflight` or the job runner.
```

- [ ] **Step 3: Mark plan 2 done in the roadmap**

In `docs/superpowers/plans/2026-09-26-v1-roadmap.md`, change the plan 2 row to
`| 2 | [Engine complete](2026-09-27-engine-complete.md) | M1 | 0.2.0 | Done |` and replace the
"### Carried over from plan 1" list with one line: `All handled in plan 2 (see its
design, "Plan 1 carry-overs"); Windows small-file speed and bench.ps1 moved to plan 4.`

- [ ] **Step 4: Run everything and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && SECOPY_DEVICE_TESTS=1 cargo test -p secopy-core --test devices`
Expected: everything passes.

```bash
git add README.md CLAUDE.md docs/superpowers/plans/2026-09-26-v1-roadmap.md
git commit -F - <<'EOF'
docs: document plan 2 cli flags and device tests
EOF
```

- [ ] **Step 5: Push and check CI (ask the user first, per CLAUDE.md)**

Run: `git push` (the current branch)
Then check GitHub → Actions:
- `ci` lint and test are green on ubuntu, macos and windows. This is the first run of the
  Windows code paths (`MoveFileExW`, share modes, `GetVolumeInformationByHandleW`,
  `SetThreadExecutionState`) and the Linux cache test. Fix what they find in follow-up
  `fix(core)` commits.
- The macOS job ran the device tests (look for `devices` in the test output).
- release-please updates its release PR. With `feat` commits since 0.1.0, it proposes 0.2.0.
  Merging it is the user's call.
