# Verify an existing copy — design

Plan 8 (RFD FR-34, issue #67). Approved in conversation on 2026-09-28.

## Goal

Months after a copy, find out whether a backup is still intact. Drives can damage or lose
files silently; Verify re-reads every file a checksum file lists and says, per file, whether
it is intact, changed or missing, and which files nothing lists. It never writes to the
directory it checks and never repairs anything.

Success: a person points Verify at a directory (one copy, or a whole drive of copies) and gets
an answer they can trust: green only when every listed file was read and matched, with
nothing unreadable, and every file that couldn't be checked named.

## Decisions (from the conversation)

| # | Decision |
|---|---|
| D1 | The user picks **a directory**; Secopy finds every checksum file inside it (any depth). |
| D2 | Files in that directory that no checksum file lists are reported as **Not checked (no checksum)**: a count in the summary, the paths in the report. |
| D3 | **Mirrors keep a checksum file** too: a hidden `.secopy-checksums.xxh64` in the destination, updated on every run, so mirror backups are verifiable. |
| D4 | A separate verify engine (`secopy-core::check`); the copy engine is not changed. |
| D5 | A new tab: Copy · Mirror · **Verify** · Queue (⌘1–⌘4). Verify jobs can be queued. |

## What the user sees

- **Verify tab:** a "Directory" row with Choose… (and drag and drop). After choosing, a scan
  shows: "3 checksum files · 1,284 files listed · 212 GB · 12 files not listed", and
  problems found while reading the checksum files. Action bar: **Add to queue**, primary
  **Start verify** (off until a directory with at least one listed file is chosen).
- **Progress:** the Copying screen, titled **Verifying**, with one bar, **Checked** (bytes
  read of bytes listed), the active files, and the file list. Pause and Cancel as for a copy;
  Cancel's question has no "remove files" option (nothing was written).
- **Summary:** headline "All 1,284 files intact" (green) or, in red, the problems in this
  order: "3 files changed", "1 missing", "2 couldn't be read", "1 checksum file couldn't be
  read". Stats: files, bytes read, time, speed, "12 not checked" (a `Hint`: "Files no checksum
  file lists: nothing to compare them with."). A **Problems** section lists changed, missing
  and unreadable files with their checksum file. The file list shows ✓ Intact, ✗ Changed,
  ✗ Missing, ✗ Failed (with the reason on hover). Show in Finder, Save report…, **Done** back
  to Verify.
- **Queue:** "Verify · /Volumes/Backup/Day01"; a verify job that isn't all intact is a failed
  job (reason as the headline). Notifications as for copies.
- **CLI:** `secopy-cli --check <directory>` prints the counts and each problem; exit 0 when
  every listed file is intact and nothing is unreadable, 1 otherwise.

"Not checked" alone does not make a job fail: it is shown, not hidden (D2), but a directory
with a few extra files isn't a damaged backup.

## Engine: `secopy-core::check`

A new module, independent of the copy job runner. It reuses `verify::hash_from_device`
(reads from the device, bypassing the page cache, FR-26), `control::JobControl` (pause,
cancel) and the xxhsum line format.

1. **Find** (`find(dir) -> Found`): walk `dir` (no symlinks followed), skipping
   `.secopy-archive` and system files. Collect every `*.xxh64` file, and every other file as a
   candidate.
2. **Read** each checksum file (`parse`): GNU/xxhsum lines `<16 hex>  <path>`, with the
   coreutils escaping (a leading `\`, `\\`, `\n`, `\r`) that `checksum_file` writes. Paths are
   relative to the checksum file's directory. Unreadable files and malformed lines become
   `Problem`s (file, line number, why). A path that leaves the checked directory (`..`, an
   absolute path) is a problem, never read.
3. **Expected set:** path → (hash, checksum file). When several checksum files list a path,
   the one with the newest modification time wins (a later copy replaced the file).
4. **Not checked:** candidates that are neither listed nor checksum files, nor Secopy's own
   reports next to them (`*_report.txt`, `*_report.json`).
5. **Run** (`run(plan, opts, control, on_event) -> CheckReport`): files in lanes (the copy
   engine's lane counts for small and large files), each hashed from the device and compared.
   Outcome per file: `Intact`, `Changed { expected, actual }`, `Missing`, `Failed(reason)`,
   `Cancelled`. Progress events: bytes checked, files done, active files (the same shapes as
   `job::Progress`, so the app's progress view is reused). Keeps the system awake.
6. **Report:** `CheckReport` with the outcomes, `not_checked`, `problems`, `cancelled`,
   elapsed; `is_intact()` is true only when every expected file is `Intact` and there are no
   problems and no cancel. `report::Report` gets a check form (text and JSON) with the same
   sections: counts, problems, not checked, every file.

The checked directory is opened read-only; nothing is written under it.

## Mirrors keep a checksum file (D3)

- After a mirror run's removals (or when nothing is removed), write
  `<destination>/.secopy-checksums.xxh64`:
  - start from the previous file, if any (read with `check::parse`; unreadable lines are
    dropped);
  - set the hash of every file this run copied or updated (its verified hash);
  - drop removed files; move renamed ones to their new name;
  - with the deep check on, set the hash of every unchanged file it compared;
  - write to a temporary name, sync, then rename over the old file (never half-written).
- Written only when the copy phase ended cleanly; otherwise the previous file stays as it was
  (its entries for files this run replaced are then wrong, so a later Verify reports them
  as changed rather than intact — never a false "intact").
- The mirror's planning ignores `.secopy-checksums.xxh64` on both sides (like the archive and
  system files); the copy engine's scan treats it as a system file.
- A mirror's summary and report don't mention it; the design system gets no new component.
- Files already in the destination before this change have no hash until they change or a
  deep-check run compares them; Verify lists them as "not checked" meanwhile.

## App

- `Jobs` runs one job at a time as today; a job is a copy (today's `Ready`, including
  mirrors) or a check (a `check::Plan` and its directory). The job thread dispatches on it;
  progress, summary, report file, queue results and notifications are shared.
- DTOs: `CheckView` (the scan: checksum files, files listed, bytes, not checked count,
  problems), `SummaryView.check: Option<CheckSummaryView>` (intact, changed, missing, failed,
  not checked, problems), `RowStatus` gains `Intact`, `Changed`, `Missing`.
- Commands: `check_directory(path) -> CheckView` (the scan, stored like a preview),
  `start_check(on_progress)`, `add_check_to_queue(path)`; `QueuedJob::Check { directory }`
  in `queue.json` (older Secopy reads it as a job for a newer Secopy).
- UI: `VerifyScreen.svelte` (the tab), `App.svelte` (tab, ⌘3, screens verify /
  verify-summary), `JobProgress` (title Verifying, one bar), `Summary` (check headline,
  problems, not checked), `headline.ts`, queue rows, View menu: Copy ⌘1, Mirror ⌘2, Verify ⌘3,
  Queue ⌘4.

## Testing

Engine (`tests/check.rs`): a copy then a check is all intact; a flipped byte is `Changed`; a
deleted file is `Missing`; an unreadable file is `Failed`; extra files are not checked;
reports next to checksum files and `.secopy-archive` are ignored; two checksum files listing
one path — the newest wins; malformed lines and paths escaping the directory are problems;
escaped names (`\`, newline) round-trip with `checksum_file::write`; cancel stops within a
file and says cancelled; nothing under the directory is modified (mtimes and listing before
and after). Mirror: the checksum file after a run lists new, updated, renamed, not removed;
a failed run leaves the previous file; a later check of a mirror is all intact; the file is
never planned for removal. App: a check job's summary and queue result; a queued check that
finds a changed file fails. UI: the Verify screen (scan line, Start off without files), the
check summary headlines, queue rows. CLI: `--check` exit codes.

## Review focus

1. A check never reports "intact" for a file it didn't read in full and match, and never
   writes under the checked directory.
2. A checksum path can't reach outside the checked directory.
3. The mirror checksum file is never half-written and never claims a hash for a file this run
   didn't verify.
4. Names with Unicode forms, `\` or newlines match what `checksum_file` wrote.
5. A huge directory (100k files) stays responsive: the scan runs off the UI thread and the file
   list pages as today.

## Out of scope

Repairing files; formats other than xxHash64 in xxhsum/GNU form (MD5, SHA; ASC MHL is
v1.1); verifying against the original source instead of a checksum
file (the mirror's deep check does that); scheduling.
