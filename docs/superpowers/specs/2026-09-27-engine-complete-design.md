# Engine Complete (M1) — Design

Plan 2 of the [v1 roadmap](../plans/2026-09-26-v1-roadmap.md). The requirements live in
[RFD 0001](../../rfd/0001-secopy.md); this document records how the engine meets them and
the decisions made while designing it (also logged in RFD §14).

## Goal

Finish `secopy-core` so the desktop app (plan 3) only has to call it: pre-flight, conflict
handling, metadata, pause/resume, fatal-error detection, keep-awake, the job report, the
plan 1 carry-overs, and fault-injection tests that prove the correctness promises. Release
0.2.0.

**Out of scope:** benchmarks and tuning (plan 4, RFD M3). Where a choice here affects speed,
the simplest correct option is taken and plan 4 revisits it.

## Decisions

| # | Decision |
|---|---|
| D1 | Performance work moves after the UI, to a new M3 / plan 4. Correctness work stays here. |
| D2 | **Conflicts:** identical files (same size, mtimes less than 2 s apart) are always skipped and counted. For files that differ the user chooses once: **Keep both** (default) / **Overwrite** / **Skip**. Overwrite replaces the old file only after the new copy is complete, and verified in Copy & Verify. |
| D3 | **Pre-flight:** job-level problems block Start. Per-file problems are listed; the user may start anyway and those files fail with a clear reason. Invalid names are never renamed automatically. |
| D4 | **Skipped identical files** are not read or hashed. They are not in this job's checksum file, and the summary and report say "skipped, not checked". |
| D5 | **Architecture:** a resolved plan. `scan → select → preflight → Plan::resolve → run_job`. |
| D6 | **Leftover partial files** are deleted at job start when no live writer holds their lock. |
| D7 | **Fatal errors** are detected by re-checking the source and destination roots after any per-file I/O error. |

## Pipeline

```
scan(&Source, &ScanOptions)                 -> Scan
scan.select(&ExtensionFilter)               -> Selection
preflight(&Source, &Selection, dest)        -> io::Result<Preflight>
Plan::resolve(&Selection, &Preflight, DiffersPolicy) -> Plan
plan.blockers()                             -> Vec<Blocker>   // empty = Start enabled
run_job(&Plan, &JobOptions, &JobControl, on_event) -> JobReport
Report::new(&Plan, &JobReport, JobMeta)     -> Report         // to_text(), to_json()
```

`resolve` is pure and cheap; the UI calls it again whenever the user changes the policy.
**Retry failed** is `selection.subset(&failed_ids)` followed by pre-flight and resolve.

Anything that changes on disk between pre-flight and Start is still handled safely: commits
never replace a file the plan did not mean to replace, so such a file fails with
`AlreadyExists`.

## Modules

New in `secopy-core`:

| Module | Purpose |
|---|---|
| `fsinfo` | Facts about the destination volume |
| `names` | Name validity per destination file system |
| `preflight` | Checks the selection against the destination |
| `plan` | Resolves one action per file |
| `control` | `JobControl`: pause, resume, cancel, checkpoint |
| `report` | The job report as text and JSON (FR-35) |
| `awake` | Keeps the system awake during a job |

Changed: `job.rs` becomes `job/{mod.rs, runner.rs, progress.rs}`. `scan` records directory
mtimes and resolves paths with `std::path::absolute`. Public types derive
`serde::Serialize`, for the JSON report now and Tauri events in plan 3.

### `fsinfo`

```rust
pub struct FsInfo {
    pub kind: FsKind,              // Apfs, HfsPlus, Ext4, Btrfs, Xfs, Ntfs, ReFs, ExFat, Fat32, Smb, Nfs, Other(String)
    pub case_sensitive: bool,      // probed, not guessed
    pub free_bytes: u64,
    pub max_file_size: Option<u64>,// Some(4 GiB - 1) on FAT32
    pub name_limit: NameLimit,     // Bytes(255) or Utf16Units(255)
    pub device: u64,               // st_dev / volume serial, for fatal-error detection
}
pub fn fs_info(dir: &Path) -> io::Result<FsInfo>;
```

- Type: `statfs` `f_fstypename` (macOS), `statfs` `f_type` (Linux),
  `GetVolumeInformationW` (Windows).
- Case sensitivity: create `.secopy-probe-<random>` in the destination, check whether the
  upper-case name resolves to it, delete it. This also proves the destination is writable.
- Free space: `statvfs` / `GetDiskFreeSpaceExW`.

### `names`

`check_name(name: &OsStr, fs: &FsInfo) -> Result<(), NameProblem>`, where
`NameProblem = InvalidChar(char) | Reserved | TrailingDotOrSpace | TooLong { limit }`.

- Windows rules (`< > : " / \ | ? *`, control characters, `CON`/`PRN`/`AUX`/`NUL`/`COM1-9`/`LPT1-9`
  with any extension, trailing dot or space) apply on NTFS, ReFS, exFAT and FAT32, and to
  every destination when running on Windows.
- Other file systems: only the length limit. SMB and NFS servers are not second-guessed; the
  OS error surfaces per file.
- Length is counted in bytes or UTF-16 units, per `FsInfo::name_limit`.

### `preflight`

```rust
pub struct Preflight {
    pub dest: PathBuf,
    pub fs: FsInfo,
    pub blockers: Vec<Blocker>,
    pub file_problems: Vec<FileProblem>,   // { id, kind }
    pub conflicts: Vec<Conflict>,          // { id, kind: Identical | Differs { size, mtime } }
    pub stale_partials: Vec<PathBuf>,
    pub checksum_omissions: Vec<usize>,    // ids whose names aren't valid UTF-8
    pub source_roots: Vec<(PathBuf, u64)>, // with device ids, for fatal-error detection
}
pub enum Blocker {
    DestMissing,
    DestNotWritable(IoFailure),
    DestInsideSource,
    NotEnoughSpace { needed: u64, free: u64 }, // added by Plan::blockers
}
pub enum ProblemKind {
    InvalidName(NameProblem),
    TooLarge { limit: u64 },
    NameClash,                              // FR-17a
    InTheWay { path: PathBuf },             // a folder where a file goes, or a file where a folder goes
}
```

Rules:

- **Identical:** same size and `|Δmtime| < 2 s`. FAT32 stores times in 2 s steps and exFAT and
  SMB round. This depends on FR-19: copies must keep their mtime.
- **Name clashes (FR-17a):** compared on raw bytes, not lossy UTF-8. Names are compared
  ignoring case only when the destination is case-insensitive; on case-sensitive
  destinations `a.txt` and `A.TXT` are both copied. Unicode normalization clashes (NFC vs
  NFD on APFS) are not predicted; the no-replace commit catches them as `AlreadyExists`.
- **Destination inside source:** absolute paths compared component by component, with
  symlinks resolved in the parent directories only.
- **Stale partials:** one extra `symlink_metadata` per planned file.

### `plan`

```rust
pub enum DiffersPolicy { KeepBoth, Overwrite, Skip }   // default KeepBoth
pub enum Action {
    Copy,
    Overwrite,
    KeepBoth { rel: PathBuf },   // "name (1).ext", unique on disk and within the plan
    SkipIdentical,
    SkipDiffers,
    Fail(FileError),             // from a file problem
}
pub struct Plan {
    pub dest: PathBuf,
    pub files: Vec<PlannedFile>,       // { entry: ScanEntry, action: Action }
    pub dirs: Vec<PlannedDir>,         // { rel, source_mtime }, parents first
    pub bytes_to_write: u64,
    pub fs: FsInfo,
    // + stale partials, checksum omissions, source roots from the pre-flight
}
```

- `Plan` owns a copy of the planned entries, so the app can keep it in state without
  lifetimes. Memory at 1M files is checked in plan 4 (NFR-4).
- `blockers()` = pre-flight blockers + free space: `bytes_to_write + max(1 %, 64 MiB)` must
  fit. Overwrite counts the full new size, because the old file stays until the commit.

### `control`

`JobControl` moves out of `job` and gains `pause()`, `resume()`, `is_paused()` and
`checkpoint() -> Result<(), FileError>`. The checkpoint blocks while paused (condition
variable) and returns `Cancelled` once stopped. Copy and verify call it at every buffer
boundary instead of reading an `AtomicBool`; lanes call it before starting a file.

## Running a job

Per file:

1. Create missing parent folders (on demand; a concurrent set remembers created folders).
2. Delete a stale partial for this file if its lock can be taken; if not, fail with
   `PartialInUse` ("another copy is writing this file").
3. Write the partial file with `create_new`, and hold a lock on it while writing: `flock` on
   macOS/Linux, a share mode without `FILE_SHARE_DELETE` on Windows.
4. Set modified, accessed and (macOS, Windows) creation time with `File::set_times` on the
   open file, then `fsync`, so the times are durable with the data (FR-19).
5. Verify, in Copy & Verify.
6. Set POSIX permission bits (macOS, Linux).
7. Commit by action:
   - `Copy`: no-replace rename (as in plan 1).
   - `Overwrite`: atomic replacing rename, only once the copy is complete (and, in Copy &
     Verify, step 5 passed). On Windows a read-only
     target has its read-only attribute cleared first.
   - `KeepBoth`: no-replace rename to the planned name; if it was taken since pre-flight,
     try `(2)`, `(3)`, ….
   - `Skip*` and `Fail`: nothing is written; the outcome is recorded straight away.

**Partial names:** `.<name>.secopy-partial` when that fits the name limit, else
`.secopy-<xxh64 of the lowercased name>.partial`. Hashing the lowercased name keeps the
rule that one name (ignoring case) maps to one partial file, so two writers can never
share one. With partial files locked by name, two Secopy jobs can't race on FAT/exFAT; only
a window of microseconds against other programs creating the same name remains, accepted.

**After the last file:**

1. Unless the job was cancelled or hit a fatal error: create the empty source folders
   (FR-6) and set folder mtimes, deepest first (FR-19).
2. Always: write the checksum file for the files that finished, without
   `checksum_omissions` (FR-23, FR-28), then fsync the folders and flush the drive cache
   (RFD §7.4, unchanged).

**Pause (FR-22):** every lane stops at its next buffer boundary. Progress events continue
with `paused: true`. Files stay open and the system stays awake.

**Fatal errors (FR-21):** after any per-file I/O error, the runner checks the relevant root
again: the destination, or the source root the file came from. If the root is gone, or its
device id changed (unmounted, or remounted elsewhere), the job stops with
`FatalError::DestinationGone` or `SourceGone`. `StorageFull` stays fatal
(`FatalError::DiskFull`). Anything else fails only that file. `FatalError` is separate from
`FileError`; `JobReport::fatal` becomes `Option<FatalError>`.

**Keep awake:** `JobOptions::keep_awake` (default on) holds a system-sleep assertion for the
whole job, including pauses; the display may sleep. Use the `keepawake` crate unless its
Linux dependencies are too heavy, in which case call the OS APIs directly
(`IOPMAssertionCreateWithName`, `SetThreadExecutionState`, logind `Inhibit`). Decided while
writing the plan.

**Memory:** `small_file_threshold` defaults to the buffer size (4 MiB), so small-file lanes
never take the pipelined path. Buffer memory is then about 8 × 4 MiB + 1 × 16 MiB.

**Events and outcomes:**

- `FileStatus` gains `Skipped(SkipReason::{Identical, Differs})`.
- `FileOutcome` gains `final_rel` (differs from `rel` for Keep both) and `in_checksum_file`.
- `Progress` gains `paused` and `files_skipped`.
- `JobReport` gains `removed_partials`.

## Report (FR-35)

`Report::new(&Plan, &JobReport, JobMeta)`; `JobMeta` carries the app version, mode, source,
destination, and start and end times.

- Contents: settings, times, counts (copied, verified, skipped identical, skipped because
  different, failed, not started), cache bypass, removed partial files, files missing from
  the checksum file with the reason, and one row per file.
- `to_text()` for people, `to_json()` (serde_json) for tools.
- `write_next_to(checksum_path)` writes `secopy_…_report.txt` and `.json` next to the
  checksum file, for the Settings option. The app's own copy in its data folder is plan 3.

## Plan 1 carry-overs

| Carry-over | Resolution |
|---|---|
| Stale partial files | Locked partials, cleanup at job start (D6) |
| FAT/exFAT commit race | Partial locks close it between Secopy jobs; the rest is accepted |
| Non-UTF-8 names | Raw-byte clash keys; copied but left out of the checksum file, with a reason |
| Scan path resolution | `std::path::absolute`, symlinks resolved in parents only |
| Memory for mid-size files | `small_file_threshold` = buffer size |
| Long names | Hashed partial-name fallback |
| Empty folders after a cancel | Folders created on demand |
| CI gaps | Clippy on all three OSes. A PAT for release-PR CI is the user's call |
| Unverified macOS cache bypass | Spike + residency test (below) |
| Windows small-file speed, `bench.ps1` | Moved to plan 4 |

## Testing

- **Unit tests:** `names` (a table per file system), identical check, clash keys, partial
  names, Keep-both numbering, `Plan::resolve` (a table of conflict, clash and problem cases),
  report text and JSON.
- **Cache bypass (FR-26), spike first:** after a copy, a test maps the file and asks
  `mincore` which pages are still resident (macOS, Linux). If the last page is cached after
  an `F_NOCACHE` write, evict it before the verify read (`mmap` + `msync(MS_INVALIDATE)` on
  macOS; `fadvise(DONTNEED)` after `fsync` on Linux). The test stays as a regression guard.
- **Fault injection, every OS:** I/O errors injected through `Hooks`; the destination
  folder renamed away mid-job → `DestinationGone`; a source file removed or truncated
  mid-copy; a flipped byte caught by verify (exists).
- **Real devices, macOS:** RAM disks from `hdiutil attach -nomount ram://…` (no root):
  disk full on a tiny volume, forced detach mid-copy, FAT32 and exFAT volumes for the 4 GiB
  limit, invalid names and case-insensitive clashes, a case-sensitive APFS volume for
  FR-17a relaxation. Gated behind `SECOPY_DEVICE_TESTS=1`; the macOS CI job sets it.
- **Job tests:** every action end to end, pause/resume (a paused job does no I/O, then
  finishes), metadata round trip (mtime, creation time, permission bits, folder mtimes),
  retry-failed via `subset`.

## CLI

- `--on-conflict keep-both|overwrite|skip` (default `keep-both`), `--report <dir>`.
- Prints pre-flight blockers, per-file problems and conflict counts before starting; exits
  with code 2 on a blocker.
- Ctrl+C cancels, as today. Pause is API-only and tested through the API.
