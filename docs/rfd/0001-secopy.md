# RFD 0001 — Secopy: fast, verified file copy for the desktop

| | |
|---|---|
| **State** | Implemented (v1) |
| **Created** | 2026-09-26 |
| **Updated** | 2026-10-04 (decisions in §14) |
| **Author** | Abel Castro |
| **Stack** | Tauri 2 · Rust engine · Svelte UI |

---

## 1. Summary

Secopy is a macOS desktop app that copies files from a
source to a destination **as fast as the hardware allows**. It can optionally **verify**
every copy by comparing the XXH128 of the original with the XXH128 of the copy read
back from disk. For every job it writes an **XXH128 checksum file** into the destination
so the copy can be re-checked later with Secopy or with standard tools (`xxhsum -c`).

The UI is a single window. The user picks a source, a destination and a mode, then
presses Start. It should look good and handle edge cases without the user having to
think about them.

The main audience is people **offloading media**: camera cards, sound recorders, shuttle
drives. They need proof that every byte arrived intact. The design should still work for
anyone copying files that matter.

## 2. Motivation

Copying a directory with Finder has three problems:

1. **No proof of integrity.** A copy can be silently corrupted (bad cable, failing card
   reader, flaky USB hub, bit rot) and you only find out when it is too late.
2. **No record for later.** The destination carries no checksums, so there is nothing to
   check it against after a week, a year, or another copy.
3. **It is slow and fragile with large jobs.** One error can stop the whole copy, the
   progress bar is unreliable, and there is no summary of what actually happened.

Professional tools that fix this (media offload tools, `rsync -c`, scripts around
`cp`) are either expensive, command-line only, or aimed at specialists. Secopy aims
for the middle: reliable enough for professionals, simple enough for anyone.

## 3. Goals and non-goals

### Goals (v1)

- Copy a **directory** or a **set of files** to a destination directory.
- Two modes: **Copy** and **Copy & Verify** (XXH128, source vs. copy read back from disk).
- Write an **XXH128 checksum file** to the destination for every job.
- **Filter by extension** when the source is a directory.
- Choose between copying **the directory itself** (`SOURCE/…`) or **only its contents** (`…`),
  with one checkbox: "Include the “SOURCE” directory", off by default (#152).
- Copy hidden files too (a camera can mark its own files hidden); skip only the names on the
  ignore list, by default the files computers leave on a card, such as `.DS_Store` and
  `Thumbs.db` (FR-12).
- Get close to the throughput of the slower of the two devices.
- **Queue** jobs and run them one after another, unattended (§5.7, 0.7.0).
- **Mirror** a directory one way to a backup with saved presets: new and changed files
  copied, deleted ones archived or removed (§5.8, 0.8.0).
- **Verify** an existing copy against its checksum files (FR-34), and optionally keep an
  **ASC MHL** history with each copy (FR-57).
- **Export and import** settings and presets (§6.11), and keep jobs running from the **menu
  bar** when the window is closed (§6.12).
- English and Spanish, chosen in Settings (NFR-11).
- Run natively on Apple silicon Macs. Secopy is macOS only by design, and Intel Macs are
  not supported (§14, 2026-09-27 and 2026-09-28).

### Non-goals (v1)

- Two-way sync. (One-way mirroring is a goal, §5.8.)
- Multiple destinations in one job: two copy jobs in the queue do it (§14, 2026-09-28).
- Network protocols (SFTP, S3…). Mounted network shares work as ordinary directories.
- Scheduling, watch folders, background daemons (the queue runs while the app is open).
- Resuming a job after the app is closed or crashes (planned, §11).
- Copying extended attributes, ACLs, resource forks.
- Encryption or compression.
- Telemetry of any kind. The app never talks to the network (updates excepted, see NFR-9).
- Linux and Windows. Secopy is macOS only by design (§14, 2026-09-28).

## 4. Glossary

| Term | Meaning |
|---|---|
| **Job** | One copy, mirror run or verify: what it reads (a source, an origin, or a directory to verify) and where it writes, run by hand or from the queue. |
| **Source** | What a copy reads: either one directory, or a set of individual files. |
| **Origin** | What a mirror keeps its destination identical to. |
| **Preset** | A saved setup: a copy preset (a source and its options) or a mirror preset. |
| **Ignore list** | Name patterns never copied or mirrored: Settings' list, plus a job's own Also ignore (FR-12). |
| **Destination** | One existing directory. The **copy root** is where the files actually land (see FR-4). |
| **XXH128** | 128-bit non-cryptographic hash (XXH3), seed 0, shown in canonical lowercase hex (32 chars), as `xxhsum -H2` prints. |
| **Checksum file** | Text file listing `hash  relative/path` for every file copied in the job. |
| **Verify** | Re-read the written file from the destination device and compare its hash with the source hash. |
| **Partial file** | A file that is still being written, under a temporary name. |
| **ASC MHL** | The ASC Media Hash List, a history of verified copies read by media tools (FR-57). |

## 5. User experience

### 5.1 Principles

- **One screen, one decision at a time.** Source → Destination → Mode → Start. No wizards.
  There is a Settings panel (§5.5), but every default works without opening it.
- **Show the result before it happens.** The user always sees the resulting path
  (`/Volumes/RAID/Day01/DCIM/…`), file count and total size before pressing Start.
- **Safe defaults.** Copy & Verify is the default mode. The app never overwrites without asking.
  It never leaves a half-written file that looks complete.
- **Honest progress.** Byte-accurate progress, real throughput, a stable ETA, and a clear
  phase label (Copying & verifying → Done).
- **Errors are data, not dead ends.** One bad file never aborts the job. Everything is
  listed in the final summary with a "Retry" action.
- **Dark, calm, legible.** A dark-only interface (§5.6): deep grey surfaces, light-grey text
  that stands out clearly, and one accent colour for the primary action and progress.
  Restrained motion, good typography, no clutter.

### 5.2 Main window (sketch)

```
┌────────────────────────────────────────────────────────────────────┐
│ Copy   Mirror   Verify                   [☰ Queue 3] [⚙ Settings]  │
├────────────────────────────────────────────────────────────────────┤
│ New copy                                                           │
│ FROM                                                               │
│  Source       /Volumes/CARD_A/PRIVATE/M4ROOT/CLIP      [Choose…]   │
│               212 files · 180.4 GB · 4 ignored                     │
│  Preset       [Sony FX3 ▾]  Changed for this run      [Update]     │
│  Options      [✓] Include the “CLIP” directory                     │
│  File types   [✓ .mp4 106 · 180.0 GB] [ .xml 106 · 400 KB]         │
│  Also ignore  .gitkeep                                   Edit…     │
│ TO                                                                 │
│  Destination  /Volumes/V001/Day01            [Recent…] [Choose…]   │
│               1.8 TB available · APFS                              │
│  Files go to  /Volumes/V001/Day01/CLIP                             │
│  Existing     (●) Keep both  ( ) Overwrite  ( ) Skip               │
├────────────────────────────────────────────────────────────────────┤
│ [Copy|Copy & Verify]  106 files · 180.0 GB  [Add to queue] [Start] │
└────────────────────────────────────────────────────────────────────┘
```

Drag and drop works on both FROM and TO. FROM has one Choose… that opens a panel where a
directory or files can be picked, the same things a drop accepts. When the source is a set of
files, the "Include the directory" checkbox and the file-type filter are hidden: they don't
apply. The ignored count only appears when it is turned on in Settings (on by default). The
real screens are in the [user guide](../user-guide.md).

### 5.3 Progress view

The view has three zones, from top to bottom.

1. **Job overview.** A phase label and elapsed time; the job's percent and time left once;
   then one bar per phase, **Copied** and, in Copy & Verify, **Verified**, each with bytes
   done / total. Files done / total is in the action bar.
2. **Active files.** One row per file in progress: name (full relative path on hover),
   phase (*Copying* / *Verifying*), bytes done of its size, and a slim bar. A file stays in
   the same row as it moves Copying → Verifying.
3. **Finished files.** A scrollable list of completed files: name, size, duration, average
   speed, XXH128, and status (✓ Copied / ✓ Verified / ✗ Failed + reason). It can be
   filtered to show failures only, and it stays smooth with 1M rows (virtualized).

```
 Copying & verifying                                        00:04:12 elapsed
 Copied    ██████████████████░░░░░░  148.2 / 212.4 GB  69.8%  1.21 GB/s  ETA 0:53
 Verified  █████████████████░░░░░░░  141.0 / 212.4 GB  66.4%  1.18 GB/s  ETA 1:00
           902 / 1,284 files

 ACTIVE
 A001C014.mov   Copying     8.4 GB   5.1 GB   60.7 %  1.20 GB/s  ETA 0:03  ██████░░░░
 A001C013.mov   Verifying   7.9 GB   2.2 GB   27.8 %  1.17 GB/s  ETA 0:05  ███░░░░░░░
 + 12 small files           41 MB    18 MB    43.9 %

 FINISHED                                                         [ Failed only ]
 A001C012.mov   8.1 GB   6.9 s   1.17 GB/s   06b05ab6733a618578af5f94892f3950   ✓ Verified
```

Rules that keep the view readable:

- Small files finish in milliseconds, and giving each one a row would make the list
  flicker. Files up to 8 MiB are grouped into one **Small files** row with combined numbers;
  larger files get their own row. (The engine's small-file lanes, §7.2, use another
  threshold, 4 MiB: they decide concurrency, not display.)
- Speeds and ETAs are smoothed over the last ~3 s (moving average) so they don't jump
  around. ETA shows "—" until there is enough data to estimate.
- All figures use tabular (fixed-width) digits so columns stay still while numbers change.
- The view updates twice per second, however many files are in flight. Bars animate for
  0.5 s between updates, so they still move smoothly.

Actions: **Pause / Resume**, **Cancel…** (asks for confirmation, explains what happens to
files already copied, and can remove them). The OS is kept awake while the job runs.

### 5.4 Summary view

- Big, unambiguous status: **1,284 files copied and verified** / **3 files failed**.
- Stats: file count, total size, duration, average speed. Files skipped because they were
  already at the destination are counted separately and marked as not checked (FR-17).
- Failure list with the reason for each file (permission denied, hash mismatch, disk full…).
- Actions, what you'd do next first: **Retry**, **Show in Finder**,
  **Open checksum file**, **Save report…**, and **New copy** on the right. Ejecting is left
  to the OS (Finder, the desktop, the menu bar).
- System notification when the job ends while the window is in the background (a setting,
  on by default).

### 5.5 Settings

One small settings page, with no tabs. The app works without ever opening it. Changes
apply when saved (Save); Cancel or Esc drops them.

| Setting | Default | Notes |
|---|---|---|
| **General** | | |
| Language | Automatic | #181: the Mac's language when Secopy has it, English otherwise; or a chosen one. |
| Notify when a job finishes | On | Only when the window isn't in front. |
| Keep jobs running in the menu bar when the window is closed | On | FR-56. |
| **Copies** | | |
| Write the checksum file to the destination | On | FR-29. |
| Write ASC MHL | Off | FR-57. |
| Show the count of ignored files | On | FR-13. |
| Save the report next to the checksum file | Off | FR-35. |
| **Always ignore when copying** | The files computers leave behind | FR-12: name patterns, up to 128; Restore defaults. |

Advanced settings (files in flight, buffer size, §7.2) are future work (§11).

Settings are stored per user in the OS's standard app-config location. A section below them
exports and imports settings and presets (§6.11).

### 5.6 Visual design

The app is **dark only in v1**. One theme is less work to design, test and keep
consistent. It also suits the main audience, who often work on set and in dim edit bays.
All colours are design tokens, so a light theme can be added later without touching
components.

The direction is:

- near-black neutral greys for backgrounds, a step lighter for each raised surface
- light greys for text and anything important, so it stands out clearly
- exactly one accent colour, for the primary button, progress bars and focus
- status colours only where they carry meaning

| Token | Starting value | Used for |
|---|---|---|
| `bg` | `#111214` | window background |
| `surface` | `#1a1b1e` | cards (FROM, TO, progress zones) |
| `surface-raised` | `#232428` | hover, inputs, chips |
| `border` | `#2e3035` | hairline separators |
| `text` | `#e6e7e9` | primary text, key numbers |
| `text-muted` | `#9a9da3` | labels, secondary info |
| `text-faint` | `#6b6e75` | disabled controls only |
| `accent` | `#4f8cff` | primary button, progress bars, focus ring |
| `success` | `#3ecf8e` | ✓ Verified / Copied |
| `warning` | `#f5a524` | skipped items, cache bypass unavailable |
| `danger` | `#f25f5c` | failed, hash mismatch |

Rules:

- Text meets WCAG AA contrast (4.5:1) against its surface.
- Colour never carries meaning alone. Every status also has an icon or a word.
- Typography: the system UI font, with tabular numerals for all figures. Hashes are set
  in monospace.

These were the starting values; the ones in use are the tokens in `ui/src/app.css`, described in the [design system](../design/design-system.md).

### 5.7 Sections and the job queue

Tabs at the top hold the kinds of job: **Copy** (the main window above), **Mirror** (§5.8) and
**Verify** (FR-34). On the right of the bar, apart from them, a **Queue** button with the number
of queued jobs (highlighted while the Queue is open) and Settings. The bar is hidden while jobs
run and on Settings and Copy presets.

- **Add to queue** next to Start (and on a mirror preset) saves the job as set up; the
  Queue screen lists the jobs (reorder, remove, clear), the choice for failures (continue with
  the next job, or stop the queue) and **Start**.
- A queue run uses the Copying screen with "Job n of m" in its status, then a queue summary with one row per
  job that opens the job's own summary, and one notification. Finished jobs leave the queue;
  failed and not-run ones stay with their reason.

Design: [job queue](../superpowers/specs/2026-09-28-job-queue-design.md).

### 5.8 Mirror

Mirror presets (name, origin, destination, what to do with deleted files, comparison) live in
the **Mirror** section. **Preview…** shows every change before anything is touched (new,
changed, deleted in the origin, unchanged); **Start** runs it on the Copying screen; a
preset can also be added to the queue.

Design: [mirror](../superpowers/specs/2026-09-28-mirror-design.md).

## 6. Functional requirements

Priority uses MoSCoW: **M**ust, **S**hould, **C**ould (v1). Anything else is future work.

### 6.1 Source selection

| ID | Req | Pri |
|---|---|---|
| FR-1 | The user can pick **one directory** as source via the native picker or drag and drop. | M |
| FR-2 | The user can pick **one or more files** as source via the same native picker (multi-select) or drag and drop. Files dropped from different directories are allowed. | M |
| FR-3 | After a source is picked, the app scans it in the background. It shows file count and total size, and updates them live as filters change. The UI stays responsive during the scan, and the scan can be cancelled by picking another source. | M |
| FR-4 | For a directory source, the user chooses between: **(a) Copy the directory itself.** Copy root = `DEST/<SOURCE_NAME>/`. **(b) Copy only its contents.** Copy root = `DEST/`. One checkbox, "Include the “<SOURCE_NAME>” directory": on is (a), off is (b). The resulting path is previewed. Default: (b) (#152; a preset keeps its own choice). | M |
| FR-5 | For a file-set source, the files are copied flat into `DEST/`. Directory structure is not recreated. | M |
| FR-6 | Directory sources are copied **recursively**, keeping the relative directory structure. Empty directories are recreated (with no file-type filter). The app needs at least one file to write before a copy starts. | M |

### 6.2 Extension filter

| ID | Req | Pri |
|---|---|---|
| FR-7 | For a directory source, the scan collects every extension found, with file count and total size per extension. | M |
| FR-8 | Extensions are shown as toggleable chips, sorted by total size (largest first). All are selected by default. There are **All / None** shortcuts. | M |
| FR-9 | Extension matching is case-insensitive (`.MOV` = `.mov`). Only the last extension counts (`a.tar.gz` → `.gz`). Files with no extension are grouped as "(no extension)". | M |
| FR-10 | The filter applies to the whole tree. Directories that end up with no files after filtering are **not** created. | S |
| FR-11 | The user can type extensions directly (e.g. `mov, wav`) as an alternative to the chips. | C |

### 6.3 Hidden and system files

| ID | Req | Pri |
|---|---|---|
| FR-12 | Hidden files and directories are copied like any other: a camera can mark its own files hidden (the FAT/exFAT hidden attribute, `UF_HIDDEN` on macOS). Only names on the **ignore list** are skipped, and matching directories are not traversed. Settings › **Always ignore when copying** (#158) holds name patterns (`*` and `?`, case ignored); its defaults are the names computers leave on a card, never the camera (macOS `.DS_Store`, `._*`, `.Spotlight-V100`, `.fseventsd`, `.Trashes`, `.TemporaryItems`…; Windows `System Volume Information`, `$RECYCLE.BIN`, `Thumbs.db`…), and **Restore defaults** puts them back. Copies and mirrors follow it, and a mirror never removes an ignored file from its destination; Verify doesn't count ignored files as not checked; ASC MHL histories ignore them. What the user picks directly is never checked against it. Each copy preset, mirror preset, New copy run and queued job also has its own **Also ignore** list, added to the global one for that job only (#164); each list holds up to 128 patterns. Secopy's own working files (unfinished copies, a mirror's checksum file) are always skipped, outside the list. | M |
| FR-13 | The number of ignored files is shown in the scan summary so nothing disappears silently. It can be turned off in Settings (§5.5). Default: on. | S |
| FR-14 | The ignore list is one engine option (`ScanOptions::ignore`, `MirrorOptions::ignore`, `check::plan`'s list). CLI: the defaults, `--ignore PATTERN` (repeatable) to add, `--include-system-files` to drop the defaults. | M |

### 6.4 Destination

| ID | Req | Pri |
|---|---|---|
| FR-15 | The user picks one existing destination directory via native picker or drag and drop. macOS's panel offers New Folder. | M |
| FR-16 | Pre-flight checks run before Start is enabled, and each problem shows a clear, actionable message. **Job-level problems block Start:** the destination is missing or not writable; not enough space for the files that will be written, plus a margin of max(1 %, 64 MiB), counting the purgeable space macOS frees on demand (a copy that needs it gets a warning, not a block); the destination is the source or inside it. **Per-file problems are listed, and the user may start anyway;** those files fail with the listed reason: the name is not valid on the destination file system (e.g. `: * ? " < > |`, reserved names like `CON`, a trailing dot or space on NTFS/exFAT/FAT32), the file is larger than the file system allows (FAT32 4 GiB limit), the name is too long, or a file or directory is in the way. Names are never changed automatically. **Non-empty copy root:** if the directory the files will go to already exists and isn't empty, a warning says so (with its file count) without blocking. | M |
| FR-17 | **Conflicts:** if files already exist at the target paths, pre-flight lists them in two groups. **Identical** files (same size, mtimes less than 2 s apart) are always skipped. They are not re-read, so they are not in this job's checksum file, and the summary and report count them as "already at the destination, not checked". For files that **differ**, the user chooses once: **Keep both** (default; the copy is named `name (1).ext`) / **Overwrite** / **Skip**. Overwrite replaces the old file only once the new copy is complete, and verified in Copy & Verify. | M |
| FR-17a | Files whose destination paths are equal (e.g. `x/a.txt` and `y/A.TXT` picked as loose files, on a case-insensitive destination) are never copied over each other: the first one is copied, the others fail with "another file in this copy has the same name". Names are compared ignoring case only when the destination file system is case-insensitive. | M |

### 6.5 Copy

| ID | Req | Pri |
|---|---|---|
| FR-18 | Every file is written to a temporary name in the same directory (`.<name>.secopy-partial`, or `.secopy-<hash>.partial` when that would be too long), flushed to disk (`fsync`), then renamed atomically to its final name. A file with its final name is always complete. Partial files left by an interrupted job are deleted by the next job that copies the same files, unless another running job is still writing them; that file then fails with "another copy is writing this file". | M |
| FR-19 | Modification time is preserved on files, and creation time where the destination supports it. Ordinary permission bits are kept, with owner-read always on (special bits are dropped). Directories Secopy creates get their modification times back after their contents are written. | M |
| FR-20 | When a hash is needed (checksum file on, or Copy & Verify), the source XXH128 is computed **during** the copy from the same bytes being written. The source is read only once. | M |
| FR-21 | Per-file errors (unreadable file, permission denied, name too long, the source file changed while it was copied…) are recorded and the job continues. Fatal errors stop the job with a clear message: destination disconnected, disk full, source volume gone. | M |
| FR-22 | Pause stops I/O at the next buffer boundary. Resume continues from where it stopped. | S |
| FR-23 | Cancel stops within ~1 s. The in-flight partial file is deleted. Completed files stay and are listed in the checksum file and report. | M |
| FR-24 | Symlinks are **not followed** and are **skipped**, and each one is reported. This avoids loops and surprises (Q4, §12). | M |

### 6.6 Verify

| ID | Req | Pri |
|---|---|---|
| FR-25 | In **Copy & Verify**, after a file is written and fsynced, it is re-read **from the destination device** and hashed. That hash is compared with the source hash from FR-20. Two independent hashes are compared, and the source is not re-read (rationale in §7.3). | M |
| FR-26 | The verify read must bypass or evict the OS page cache so it tests the bytes on the device, not the bytes in RAM: `F_NOCACHE`. Where bypass is impossible (some network shares), the report says so. | M |
| FR-27 | On mismatch, the bad copy is deleted and the file is re-copied **once** automatically. If it mismatches again, it is marked **FAILED (checksum mismatch)** and nothing is committed under the final name (a file already there stays as it was). | S |
| FR-28 | A file that failed verification is **not** listed in the checksum file. | M |

### 6.7 Checksum file

| ID | Req | Pri |
|---|---|---|
| FR-29 | When **Write checksum file** is on (the default; Settings §5.5), every job writes a checksum file to the **destination directory** (not the copy root), named `secopy_YYYY-MM-DD_HHMMSS.xxh128`. A new file per job, so nothing is ever overwritten. | M |
| FR-30 | Format: `xxhsum`/GNU-coreutils compatible, one line per file: `<32 lowercase hex chars><two spaces><relative path>`. Paths are relative to the destination directory and use `/` as separator. `cd DEST && xxhsum -c secopy_….xxh128` must pass. | M |
| FR-31 | The file is UTF-8 without BOM, with LF line endings. It is sorted by path for stable diffs. Paths containing `\` or newline use the coreutils escaping convention (line prefixed with `\`). Files whose names are not valid UTF-8 are copied but not listed; pre-flight warns about them and the report says why. | M |
| FR-32 | The checksum file contains only hash lines, no comments, so strict parsers accept it. Job metadata (mode, date, app version, counts, failures) lives in the report (FR-35). | M |
| FR-33 | The checksum file is written in **both** modes. In plain Copy it uses the source hashes from FR-20, so no extra read is needed. | M |
| FR-57 | **ASC MHL** (#154): with Settings › Write ASC MHL (off by default; CLI `--mhl`), a copy also writes an ASC MHL v2.0 history (`ascmhl/`, xxh128, process `transfer`) in the directory its files go to, or continues the one the source or the destination already has: each file `verified` or `failed` against its earlier hash, and a failed one makes the job not complete. Files already there that no history records are read and recorded, said before Start. Nested histories get their own generations, referenced by the top one. Start is blocked by two different histories for one directory, overwriting a recorded file, a damaged history, or a copy that leaves out files the source's history lists. A failed write leaves the history as it was; undo takes a generation back. Copies only, not mirrors; Verify doesn't read MHL. | S |
| FR-34 | **Verify existing copy:** point Secopy at a directory (a copy, or a whole drive); every `.xxh128` checksum file inside it is read, and every file they list is read again from the drive and compared: intact, changed, missing or unreadable, per file. Files no checksum file lists are reported as not checked. Nothing is written to the directory; nothing is repaired. When several checksum files list a file, the newest wins; a path that leaves the directory is a problem, never read. Queueable; `secopy-cli --check`. | S |

### 6.8 Reporting and settings

| ID | Req | Pri |
|---|---|---|
| FR-35 | Each job produces a report as plain text and as JSON: settings, start/end, counts, per-file result (including skipped files), failures with reasons, whether cache bypass was active, leftover partial files removed, and files left out of the checksum file. It is kept in the app's data directory, "Save report…" exports it, and there is an option to also write it next to the checksum file. | S |
| FR-36 | The app remembers the mode, the window size and the last selected copy preset, which is loaded again at launch when its source is there. The destination is chosen for every job and never filled in automatically; within one session, "New copy" keeps it. | S |
| FR-37 | Keyboard, in a File menu: `⌘O` source, `⌘D` destination, `⌘↩` start, `⌘.` cancel the job (asks first); items greyed out when they don't apply. `⌘1`–`⌘3` the tabs and `⌘4` the Queue (View menu); `⌘,` Settings. `Space` pauses and resumes a job; `Esc` goes back from Settings, Copy presets and Import and cancels dialogs. | S |
| FR-38 | **Copy presets**, chosen by hand: a saved copy setup for FROM. A name, the source (a full path, e.g. `/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP`), directory-itself or contents-only, an extension filter, and its own Also ignore list (FR-12). Selecting one loads its source and options (or says its drive isn't connected); changes last for one run unless saved with "Update" or "Save as…" (which asks only for a name). The destination is never part of a preset. No automatic card detection, and nothing is written to cards. | S |

### 6.9 Job queue

| ID | Req | Pri |
|---|---|---|
| FR-39 | Any job that can be run by hand (Copy, Copy & Verify, a mirror run, a verify) can be added to a queue: a copy as set up at that moment, a mirror as its preset is at its turn; Settings are read when each job starts. Queued jobs can be reordered and removed, not edited. | S |
| FR-40 | **Start** on the Queue screen runs the jobs one after another. Each job is scanned and checked when its turn comes; one that can't start fails with its reason. | S |
| FR-41 | If a job fails (can't start, or ends with failed files), the queue continues with the next job or stops, as chosen for the queue. Cancel stops the current job and the queue. | S |
| FR-42 | The queue is saved across launches. After a run, finished jobs leave it; failed and not-run jobs stay with their reason. | S |
| FR-43 | A queue summary lists every job with its result and opens its summary; one notification for the whole queue; the system stays awake for the whole run. | S |

### 6.10 Mirror

| ID | Req | Pri |
|---|---|---|
| FR-44 | **Mirror presets:** a name, an origin and a destination (full paths that don't overlap), what to do with files deleted in the origin (archive for N days, default 30, or delete), and the comparison: **Standard** (size and date) or **Paranoid** (the deep check: the XXH128 of both copies). | S |
| FR-45 | One way only: the origin is never written to. | M |
| FR-46 | A file is new when it isn't in the destination, changed when its size or modification date differs (dates within 2 s count as equal), and, with Paranoid (the deep check), when its contents differ (XXH128 of both sides). | S |
| FR-47 | **Preview** before a manual run: counts, sizes and the list of new, changed and deleted files; "Already in sync" when there's nothing to do. A run executes the previewed plan. | S |
| FR-48 | Everything Mirror writes is verified. A changed file is replaced atomically, and its old version archived (archive mode) only after the new copy is verified. | M |
| FR-49 | Files deleted in the origin are archived to `<destination>/.secopy-archive/<date time>/…` or deleted, only after every copy succeeded; a failed or cancelled run removes nothing. At the start of every run, archived files older than the preset's N days are removed, counted from when they were archived, in Delete mode too (what was archived before a switch still goes when due). Switching a mirror from Archive to Delete asks what to do with its archive: **delete it now**, **keep it N days**, or, when the destination isn't connected or a job runs, **delete it at the next run** (kept with that destination; dropped if the destination changes; done when that run is started, once, and only after the preset is saved without it). A new destination in the same save leaves the old one's archive alone. Files that can't be deleted are listed and go when due. Only a real `.secopy-archive` directory is touched, never through a link. | M |
| FR-50 | **Guard:** an empty origin, a part of either side that can't be read, or a run removing more than half of the destination's files, needs confirmation by hand and fails in the queue. A missing origin stops the run. | M |
| FR-51 | Ignored names (FR-12), Secopy's own files, symlinks and the archive are ignored on both sides; names are compared after Unicode normalization, and a case-only rename on a case-insensitive destination is an update, not a delete and a copy. | S |
| FR-52 | A mirror summary: what was copied, updated, archived or deleted, failures with reasons, and a report like a copy's. A mirror keeps `.secopy-checksums.xxh128` in its destination: every run but an undone one adds the files it verified, and a clean run also drops removed files and records renames; written whole or not at all, so its backup can be verified (FR-34). | S |

### 6.11 Export and import

| ID | Requirement | Priority |
|---|---|---|
| FR-53 | **Export** (File › Export…, Settings): a `.secopy` file (versioned JSON) with any of the settings, the copy presets and the mirror presets, as chosen; one preset from its own screen. Never the queue, recent destinations, the remembered window or reports. Written to a temporary name and renamed, never half-written. | S |
| FR-54 | **Import** (File › Import…, Settings) shows what the file holds before anything changes: settings that differ, each preset with its paths, name clashes (**Keep both**, the default, as "Name (2)", or **Replace**, which keeps the preset's place in the queue), paths not on this Mac (a note), and presets that can't be imported (with why). Imported presets get the same checks as hand-made ones. A file that isn't a Secopy file, has a newer format version, is over 10 MiB or holds more than 1,000 presets of a kind is refused with nothing changed (a newer app version alone isn't). A file from another Secopy version says which; settings it has that this Secopy doesn't know are left out, and ones it lacks take their defaults, both listed; a preset with a setting this Secopy doesn't know can't be imported. Nothing is imported while a job or the queue runs. A save that fails partway says exactly what went in. | S |
| FR-55 | Opening a `.secopy` file from Finder opens the Import screen. | C |

### 6.12 Menu bar

| ID | Requirement | Priority |
|---|---|---|
| FR-56 | Closing the window while a job or the queue runs hides it (setting on, and only once the menu bar icon exists; otherwise closing asks to cancel, as before). The icon shows the progress next to it (`42%`, `2/3 · 42%`, `Paused`, `Removing`, then ✓ only for a complete job, ✗ otherwise); clicking it opens a small panel under it: the job, from and to, a progress bar, files, speed and time left, Pause/Resume, Open Secopy, Quit Secopy…; clicking elsewhere closes it. Secopy leaves the Dock while hidden; Open Secopy or opening Secopy again shows the window; ⌘Q shows it and asks, as before. The icon exists only while hidden during a job, or until a job that ended while hidden is seen. | S |

## 7. Engine design (performance)

The engine is a UI-independent library. The app and the CLI share one flow:
`scan → select → preflight → Plan::resolve → run_job`, with progress through a callback and
`JobControl` for pause, resume and cancel. This keeps the UI thin and the engine testable
headless.

The progress events carry raw counters: per file `{file_id, phase, bytes_done, bytes_total}`
plus job totals per phase. They are merged and emitted at most 20 times per second. The UI
derives speeds, ETAs and smoothing from them (§5.3).

### 7.1 Scan

- One directory walk, not followed through symlinks. Filtering (ignored names, extensions, symlinks, special files) happens during the walk.
- Stores entries compactly so 1M files fit in well under 200 MB of RAM.
- Produces the plan: the ordered list of (source, relative path, size, mtime), totals per
  extension, skipped items.

### 7.2 Copy pipeline

- **Streaming with overlapped I/O.** Per file, a reader and a writer share a small ring of
  large buffers (default 4 × 4 MiB, tunable). Reading chunk N+1 overlaps writing chunk N.
  XXH128 (≈36–40 GB/s per core on Apple silicon) runs over each chunk as it is read, so it is never the
  bottleneck.
- **File-level concurrency, sized by file type** (lanes):
  - Small files (up to 4 MiB, one buffer): 8 in flight, to hide per-file metadata latency
    (open, create, rename, set times), which dominates copies of many small files.
  - Large files: **1** in flight. A card (or a spinning disk) reads fastest as one long
    stream; two big files read at once make it jump between them.
- **Planned, not built yet** (the performance work, §10): sequential-access hints
  (`F_RDAHEAD`), pre-allocating the destination file to its final size (`F_PREALLOCATE`:
  less fragmentation, "disk full" caught early), and lane defaults chosen per device class
  (SSD or HDD, which macOS reports), with an advanced setting to override them.
- **OS fast paths** (`clonefile`, `fcopyfile`: instant APFS clones, server-side copies on
  SMB) never hand us the bytes, so nothing can be hashed. Not built: they could serve plain
  Copy with the checksum file off (the performance work, §11). Today every job uses the
  streaming pipeline above and computes XXH128.

### 7.3 Verify pipeline

- Verification of file N runs concurrently with the copy of file N+1 (2 verify lanes). When
  source and destination are different devices, the verify read (destination) and the copy
  read (source) don't compete. §7.5 says why this is parallel, not one after the other.
- Cache bypass per FR-26 (`F_NOCACHE`). Reads go straight to the device when the buffer
  is page-aligned, so the buffer pool allocates page-aligned memory.
- **The source is read once, and the destination is read back.** The source hash comes
  from the bytes read during the copy. The destination hash comes from re-reading the
  written file from the destination device. If they match, the destination holds exactly
  the bytes that were read. Reading the source a second time would add a full pass over
  what is often the slowest device (a card reader). It would only catch one rare failure:
  a flaky reader or cable that returns different bytes on each read. A second, independent
  read of the source is listed in §11 as future work.

### 7.4 Durability

- Each file is flushed to the device (`fsync`) before its rename (FR-18). On macOS this is
  plain `fsync`, not `F_FULLFSYNC`: flushing the drive's cache per file made 20,000 small
  files take 80 s instead of 2.6 s in the M0 prototype.
- At the end of the job: one `fsync` per created directory (makes the renames durable), then
  one drive-cache flush for the whole volume (`F_FULLFSYNC` on macOS). When the summary
  shows, the destination can be ejected.

### 7.5 Why verify runs in parallel with the copy

The whole pipeline for a Copy & Verify job, for big files:

1. **One copy lane reads the source from start to end** (one long stream, the fastest way to
   read a card), computes XXH128 over each chunk as it reads it, and writes the chunk to
   the destination with the Mac's cache bypassed (so the read-back reads the drive).
2. **Two verify lanes read finished files back from the destination** (cache bypassed),
   hash them, and compare with the source hash, while the copy lane goes on with the next
   file.
3. **The source is read exactly once.** Verify never touches it: it only reads the
   destination and compares against the hash taken while reading the source (§7.3).

So "parallel or one after the other" never changes how the card is read; it only changes
what the **destination** does: write and read at the same time, or first write everything
and then read everything.

- **Destination SSD** (internal or external; the common case): the card is the bottleneck
  (about 90 MB/s for UHS-I, 250–300 MB/s for UHS-II), and an SSD writes at that pace while
  reading back at 1–7 GB/s, with no penalty for mixing the two. Parallel hides almost all of
  the verify time: the job takes about as long as the copy, plus verifying the last file.
  Example: a 128 GB UHS-II card copies in about 8 min; reading 128 GB back from a 1 GB/s SSD
  takes about 2 min. Parallel: about 8 min. One after the other: about 10 min (+25 %).
  Verifying each file right after copying it (no overlap) is also slower: the card waits
  during every verify.
- **Destination HDD** (or a RAID of HDDs): an HDD streams 150–250 MB/s, but only when it
  reads or writes in one place. A write stream and a read stream at once make the heads jump
  between two places on the disk, milliseconds each time. Either way the HDD writes the data
  once and reads it once; one after the other, both run at full speed, while mixed, the jumps
  are lost time. Example: 128 GB at 200 MB/s is about 11 min of writing plus 11 of reading:
  about 21 min one after the other, likely 25–35 min in parallel. Parallel there also slows
  the copy itself, so the card stays busy longer.

Two more effects of "copy everything, then verify":

- **The source is free sooner.** Verify doesn't need it, so the card can be ejected (and the
  next one started) when the copy phase ends. On a set with many cards, this matters more
  than raw speed.
- **A more meaningful read-back.** Bypassing the Mac's cache doesn't bypass the drive's own
  (256 MB on an HDD, gigabytes on some SSDs): read back right after writing, the end of a
  file can come from that cache instead of the disk. Flushing the drive's cache before the
  read-back checks what reached the disk.

**Decision:** parallel stays the default: on SSD destinations, by far the most common, it is
the fastest. Not built yet, and to be decided with measurements (the performance work, with
real footage: the same card to an internal SSD, an external SSD and an external HDD; total
time, and when the card is free): a "copy everything, then verify" mode for HDD and RAID
destinations (chosen automatically, since macOS reports whether a disk spins, or offered
where freeing the card early matters), and flushing each file's data from the drive's cache
before its read-back, which closes the cache gap in either mode.

## 8. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-1 | **Throughput, large files (≥ 1 GiB), Copy mode:** ≥ 95 % of `cp` / Finder on the same hardware. |
| NFR-2 | **Throughput, Copy & Verify:** total time ≤ 1.3 × Copy mode when source and destination are different devices, ≤ 2.1 × on the same device. |
| NFR-3 | **Many small files** (100k × 16 KiB): within 10 % of `rsync`, faster than Finder. |
| NFR-4 | **Memory:** bounded, independent of file size. < 250 MB at 1M files. |
| NFR-5 | **Responsiveness:** cold start < 1 s. UI never blocks, and progress updates twice per second regardless of file count. |
| NFR-6 | **Correctness over speed:** no optimization may weaken FR-18/25/26. |
| NFR-7 | **Paths:** full Unicode (names are preserved byte-for-byte as the OS reports them; no NFC/NFD rewriting). Files > 4 GiB are supported. |
| NFR-8 | **No elevated privileges** needed. No network access, no telemetry. |
| NFR-9 | **Distribution:** an Apple silicon (`arm64`) `.dmg` and a one-line install script that checks it against its SHA-256. Signing and notarization (M4) need the Apple Developer Program and are planned; an optional auto-updater, off by default, would be the only network access. |
| NFR-10 | **Accessibility:** full keyboard operation, screen-reader labels on every control, WCAG AA contrast on the dark palette, respects "reduce motion". |
| NFR-11 | **i18n:** all strings externalized; English and Spanish, Automatic (the Mac's language) or chosen in Settings (§14, 2026-10-03 and 2026-10-04). |
| NFR-12 | **Look and feel:** dark theme only in v1 (§5.6), built on design tokens so a light theme can be added later. Native file pickers, notifications and menus. |

## 9. Testing strategy

- **Engine unit tests:** filters, path mapping (directory itself vs. contents), conflict resolution,
  checksum-file formatting (checked against the real `xxhsum -c` in CI), escaping.
- **Integration tests** on generated trees: deep nesting, Unicode names, empty dirs, 0-byte
  files, > 4 GiB sparse files, 100k small files.
- **Fault injection:** source file removed mid-copy, permission denied, disk full,
  destination unplugged (simulated with a loopback/RAM disk), a corrupted write (hook
  that flips a byte) to prove verification catches it.
- **Benchmarks** against `cp` and `rsync` on reference trees: planned with M3, not built
  yet. Hash speed is checked by hand (an ignored test, §14 2026-10-03).
- **By hand** before each release, on a real Mac with a real card:
  [the checklist](../testing/macos-app-checklist.md).
- **Where tests run:** locally before every commit (fmt, clippy, tests), in one macOS
  (arm64) CI job after each merge to `main`, and again in the release workflow before it
  builds. PRs don't wait for CI, and docs-only changes skip it.

## 10. Milestones

1. **M0 — Engine spike (CLI only):** copy + hash + verify + checksum file. Done (0.1.0).
2. **M1 — Engine complete:** filters, conflicts, atomic writes, metadata, pause/cancel,
   error model, report, full test suite including fault injection. Done (0.1.0).
3. **M2 — UI:** the app, then the queue, mirror, Verify, export/import, the menu bar,
   translations, ASC MHL and ignore lists. Done (0.2.0–0.25).
4. **M3 — Performance:** benchmarks vs. native tools on real Macs (SSD→SSD, card
   reader→SSD, SSD→USB HDD), then tuning until NFR-1..NFR-4 pass. Deferred: speed is already
   good in real use.
5. **M4 — Packaging:** the CI release pipeline, the `.dmg` and the install script are done;
   signing and notarization are not (NFR-9).
6. **M5 — Release:** the 0.x series was the beta, used on real shoots; the hand test of the
   release candidate ([checklist](../testing/macos-app-checklist.md)), then **1.0.0**.

## 11. Future work

**v1.1 — media offload** (the main audience, see §14):

- ~~**ASC MHL** output~~: done (FR-57, #154).

**Later:** "copy everything, then verify" for HDD/RAID destinations, and a drive-cache flush
before each read-back (§7.5) · the performance work (M3) and advanced settings (files in
flight, buffer size, §7.2) · signing and notarization (M4) · light theme · a second,
independent read of the source · other checksum formats if asked (e.g. an MD5 checksum file
next to the XXH128 one) · resume interrupted jobs · extended attributes / Finder tags / ACLs
· mirror: scheduling (intervals, when a drive connects), detecting moved and renamed files,
several destinations, restoring from the archive inside the app · the queue: skipping one job
without stopping the queue · the user guide in Spanish.

## 12. Open questions

None open. Q1, Q3, Q5, Q7 and Q8 are resolved in §14; numbers are kept for traceability.

| # | Question | Answer |
|---|---|---|
| Q2 | Checksum file per job, or one file per destination that is updated? | Per job (FR-29): it never rewrites history. |
| Q4 | Symlinks: skip, copy the link, or copy the target? | Skip and report (FR-24). |
| Q6 | Allow mixing directories and files, or several directories, in one source? | No: one directory, or files (FR-4 stays unambiguous). |

## 13. Implementation stack

**Decided: Tauri 2 + a Rust engine + a Svelte UI (TypeScript).**

- **Engine:** a Rust library crate (`secopy-core`) with no UI dependencies. The Tauri app,
  the CLI (`secopy-cli`) and the tests use it.
- **App shell:** Tauri 2, for windowing, native dialogs, menus, the menu bar icon,
  notifications and packaging.
- **UI:** Svelte + TypeScript, built with Vite and styled with the tokens in §5.6.

The stack meets these constraints:

1. Give direct access to macOS's low-level file APIs (cache bypass, preallocation, file
   flags/attributes).
2. Run the engine off the UI thread with true parallelism and zero-copy buffers.
3. Produce a small, signed, native macOS app.
4. Allow a polished, themeable UI with native file dialogs, drag and drop and notifications.
5. Keep the engine as a separate library, usable headless (tests, the CLI).

## 14. Decision log

| Date | Decision |
|---|---|
| 2026-09-26 | Name: **Secopy** (Q7). |
| 2026-09-26 | Stack: Tauri 2 + Rust + Svelte (Q8). |
| 2026-09-26 | The checksum file is written in both modes (Q1). It is on by default and can be turned off in Settings. |
| 2026-09-26 | The main audience is media offload (Q5). Multiple destinations and ASC MHL are planned for v1.1. |
| 2026-09-26 | Dark theme only in v1. |
| 2026-09-26 | The skipped-hidden-items count can be turned off in Settings (default on). |
| 2026-09-26 | Per-file progress (size, bytes done, %, speed, ETA) for both the copy and the verify phase. |
| 2026-09-26 | Same-name files in one job (compared ignoring case) fail with a clear error instead of overwriting each other (FR-17a). |
| 2026-09-26 | Durability: plain `fsync` per file, one drive-cache flush per job (§7.4). |
| 2026-09-26 | Performance benchmarks and tuning move after the UI, to a new M3 (§10). Tuning is internal to the engine and doesn't change the API the UI uses. Correctness work (fault injection, cache-bypass checks) stays in M1. |
| 2026-09-27 | Conflicts (Q3, FR-17): identical files (same size, mtimes < 2 s apart) are always skipped. For files that differ the user chooses once: Keep both (default) / Overwrite / Skip. Overwrite only replaces after the new copy is complete and verified. |
| 2026-09-27 | Pre-flight (FR-16): job-level problems block Start; per-file problems are listed and those files fail, so the rest can still be copied. Invalid names are never renamed automatically, because sidecars and edit projects refer to clips by name. |
| 2026-09-27 | Skipped identical files are not re-read: they are not in the job's checksum file, and the summary says "not checked". "Verify existing copy" (FR-34) covers checking them later. |
| 2026-09-27 | Leftover partial files are locked while written and deleted by the next job when no writer holds the lock, the file wasn't modified in the last 2 s, and it still has that name (FR-18). On Windows the commit is a no-replace `MoveFileExW`, which also refuses another writer's open file. |
| 2026-09-27 | A file whose size changes while it is copied fails with "the source file changed while it was copied" (FR-21). |
| 2026-09-27 | Keep-awake uses the OS directly (`caffeinate`, `systemd-inhibit`, `SetThreadExecutionState`) instead of a crate that would add D-Bus to every Linux build. |
| 2026-09-27 | Fatal errors are detected by re-checking the source and destination roots (existence and device id) after any per-file I/O error (FR-21). |
| 2026-09-27 | Engine M1 design: [2026-09-27-engine-complete-design.md](../superpowers/specs/2026-09-27-engine-complete-design.md). |
| 2026-09-27 | Plans 1 and 2 (M0 and M1) ship together as the first release, 0.1.0: plan 2 was merged before plan 1's release PR. Later plans move up one minor version: desktop app 0.2.0, performance 0.3.0, packaging 0.4.0, betas 0.5.x, then 1.0.0. |
| 2026-09-27 | v1 is a macOS app. Design, testing, polish, signing and packaging target macOS only until 1.0; Linux and Windows apps come after v1. The engine stays portable and keeps building and testing on all three OSes in CI. Supporting three OSes mostly costs testing and packaging work, and the main audience (media offload) is largely on Macs. |
| 2026-09-27 | Apple Silicon only: no Intel Macs. Builds and releases are `aarch64-apple-darwin`, not universal binaries. |
| 2026-09-27 | CI runs one macOS job after merges to `main` (and on demand), not on PRs, and skips docs-only changes; the release workflow runs the full suite before building. Windows and Linux CI jobs return with those apps. Goal: a faster workflow without dropping any check. |
| 2026-09-27 | The app (M2) is split: plan 3a is a working app (0.2.0); plan 3b adds source profiles, settings and polish (0.3.0) once 3a has been used on real shoots. Design: [2026-09-27-macos-app-core-design.md](../superpowers/specs/2026-09-27-macos-app-core-design.md). |
| 2026-09-27 | Progress updates twice per second instead of 10–20 times (NFR-5, §5.3); bars animate between updates. |
| 2026-09-27 | The destination is never filled in automatically (FR-36): every project has its own folder, and a wrong automatic choice mixes shoots. |
| 2026-09-27 | Source profiles are chosen by hand, with no automatic card detection, and nothing is ever written to a card (FR-38): formatting erases such files, locked cards can't take them, and camera media should never be modified. |
| 2026-09-27 | App identifier: `com.belisoft.secopy`. |
| 2026-09-27 | Plan 3b is split: 3b-1 is source profiles and settings (0.3.0, #16), 3b-2 is notifications, Eject, keyboard shortcuts and accessibility (0.4.0, #20). Performance moves to 0.5.0. Design: [2026-09-27-profiles-settings-design.md](../superpowers/specs/2026-09-27-profiles-settings-design.md). |
| 2026-09-27 | No "Default mode" setting (§5.5): the app remembers the last mode (FR-36), which makes a default redundant. The last selected source profile is remembered too; the destination still never is. |
| 2026-09-27 | App identifier changes to `com.latecommits.secopy` (0.3.0). Reports saved by 0.2.0 move to the new data folder on first launch. |
| 2026-09-27 | Hidden files are copied; only known system files are skipped (FR-12, #25). On FAT/exFAT cards a camera can mark its own files hidden, so skipping every hidden item could leave a card copy silently incomplete. |
| 2026-09-27 | Plan 3b-2 (#20, 0.4.0): Eject is a summary button, never automatic (Retry failed may still need the card), using macOS's own eject; shortcuts live in a File menu; the notification only shows when the window isn't in front; accessibility is checked by tests (axe-core, token contrast) plus a VoiceOver walk-through. Design: [2026-09-27-notify-eject-shortcuts-design.md](../superpowers/specs/2026-09-27-notify-eject-shortcuts-design.md). |
| 2026-09-27 | FROM has one Choose… for a folder or files (one macOS panel that accepts both, like a drop), and "folder itself / only its contents" is one checkbox, "Include the “DCIM” folder", instead of two radio buttons. |
| 2026-09-28 | On macOS, keep-awake is an IOKit power assertion inside the app instead of a `caffeinate` process: macOS 27 reports an app whose helper process is running as "running in the background". |
| 2026-09-28 | Settings are saved with Save and dropped with Cancel, instead of applying at once: simpler to predict, and the same as profiles. Back sits in the action bar with every other button. |
| 2026-09-28 | No drives row in FROM (was B8, plan 3b-1): Source shows the chosen source, as Destination does in TO; a card is picked by dropping it or with Choose…. |
| 2026-09-28 | No Eject button and no "Safe to eject" line on the summary (were C1, C2 in plan 3b-2): macOS already ejects from Finder, the desktop and the menu bar, and a finished copy is already flushed. Retry still says when the card is gone. |
| 2026-09-28 | A profile saves the full source path instead of a folder relative to the card (B3 in plan 3b-1): a profile is a saved copy setup you load in one step, which is what the user expected. The Start button says "Start copy"; the mode is chosen next to it. |
| 2026-09-28 | **Job queue and mirror in the same app** (#50, #51): a sidebar with Copy · Mirror · Queue. One engine and one look; the queue holds every kind of job. A separate mirror app was rejected (a duplicated engine, no shared queue). Queue first (0.7.0), mirror next (0.8.0); performance and packaging move after them. |
| 2026-09-28 | **Mirror safety:** one way only; a manual run previews first; deletions happen last and only after every copy succeeded; deleted files are archived (kept N days) or deleted per preset; a guard stops runs with a missing or empty origin or that would remove more than half of the destination. Changed = size or date (2 s tolerance), with an optional deep check by checksum. |
| 2026-09-28 | **macOS only by design** (#60): no Linux or Windows apps are planned. The engine's Linux and Windows code is removed, and building it for another OS is a compile error. This replaces the 2026-09-27 plan to keep the engine portable. |
| 2026-09-28 | **Sections as tabs at the top** (#64), not a sidebar: three items don't need 180 px of width, and paths and file lists do. Settings sits on the tab bar, once for the app. |
| 2026-09-28 | **Verify an existing copy** (#67): a directory, and every checksum file inside it; files nothing lists are "not checked" (shown, not a failure); mirrors keep a hidden checksum file so their backups can be verified. A separate check engine; the copy engine is unchanged. |
| 2026-09-28 | **The Queue apart from the tabs** (#67): the tabs are the kinds of job (Copy, Mirror, Verify), underlined when selected; the Queue, where any of them waits and runs, is a button with its count on the right, next to Settings. |
| 2026-09-28 | **No multiple destinations** (#69 discussion): two copy jobs in the queue copy a source to two destinations, reading it twice. A job that reads once and writes N copies was dropped: it's a new path through the copy engine for something the queue already does. This replaces the v1.1 plan (2026-09-26). |
| 2026-09-28 | **Presets** (#72): copy profiles are now copy presets, like mirror presets: one word for saved setups. The saved file keeps its name, profiles.json. |
| 2026-09-28 | **Short button labels** (#72): the fewest words that can't be read two ways. Every job starts with **Start** (copy, verify, mirror, queue: the tab says which, like ⌘↩); **Update**, **Save as…**, **New preset…**, **Clear…**, **Retry**. **Add to queue**, **Save report…** and **Show in Finder** keep their words: one word would be ambiguous. Replaces "Start copy" (#45). |
| 2026-09-29 | **Export and import** (#77): one `.secopy` file for settings and presets, for backups, new Macs and sharing; clashes Keep both by default or Replace (keeping the preset's id); paths not on this Mac are a note, not an error; nothing imported while a job runs. |
| 2026-09-29 | **Keep copying in the menu bar** (#80): only while a job runs; Secopy leaves the Dock while hidden; a setting, on by default; the window is never hidden without the icon. A native menu was tried first: its disabled lines are grey and hard to read, and replacing it closed it on every update, so the icon opens a small panel of Secopy's own (a progress bar, readable text). |
| 2026-09-29 | **Translation-ready, English only** (#84): every word the app shows is in a catalog (`ui/src/locales/en.json`); Rust sends message codes, never sentences; the Mac's language when a catalog exists for it. Reports and the CLI stay English. No second language yet. |
| 2026-09-29 | **License: GPL-3.0-or-later** (#97), copyright Abel Castro Suárez: anyone may use and change Secopy, and what they distribute built on it stays open source under the GPL. |
| 2026-09-29 | **Mirror archive days** (#101): one number, applied at every run to everything archived, counted from when it was archived; in Delete mode too. Switching to Delete asks what to do with the archive (now, keep N days, or at the next run). |
| 2026-09-29 | **Mirror comparison** (#100): **Standard** (size and date) or **Paranoid** (byte-for-byte), no "None": every mirror copy stays verified. Paranoid is marked "Very slow" in the warning style: on a 100 MB/s drive a 1 TB mirror with nothing changed takes 5 h 33 min per run, against a fraction of a second with Standard. Saved as `deepCheck`, unchanged. |
| 2026-09-29 | **Mirror archive on its screen** (#99): files, size and oldest run of `.secopy-archive`; Show in Finder; Delete archive… (asks; not while a job runs). No per-run list for now. |
| 2026-09-29 | **Free space counts purgeable space** (#108): the destination's space is macOS's figure for data stored at the user's request (`volumeAvailableCapacityForImportantUsage`: free plus purgeable, what Finder and Disk Utility show), or `statvfs` if larger: that figure is 0 outside the boot volume group (disk images, external drives), and space free now can always be written. Only `statvfs` blocked copies that fit (29 GB free, 340 GB available). Purging Time Machine local snapshots isn't reliable (Eclectic Light, macOS 26.6.2), so a copy that needs more than what is free now warns, with how to free it first; if macOS doesn't purge in time, the copy stops with the disk full like any full disk. |
| 2026-09-30 | **Overwrite safety** (#112, from the QA review): the choice for files that differ answers for the files shown, so it resets to Keep both when the source, file types or destination change; a queued copy keeps the list of files it replaces and doesn't start if it would replace any other (nobody is there to confirm, like FR-50); Start re-checks the destination and refuses if what it would do changed; a file that would land on a source file (by device and inode) is a pre-flight problem; names that differ only in Unicode form clash. |
| 2026-09-30 | **Questions before risky actions** (#113, from the QA review): asked in the window, not in a system alert, whose first (Return) button was the risky one; the safe answer is first and focused, Esc answers no and only the question. A pending "Delete it at the next run" ends when the mirror goes back to Archive; archive days are at least 1 in both modes, and 0 (older presets) never cleans; Delete archive… names the destination it showed and is refused if the mirror's changed; "Delete them now" saves the edit first. |
| 2026-09-30 | **Mirror origin links and checksums** (#114, from the QA review): what's under a link in the origin (never followed) is never removed; a destination directory that can't be read is a guard (nothing removed, asks first); a removal that can't be looked at is a failure, not "already gone"; the mirror's checksum file records what any run verified (removals and renames only after a clean run; nothing after an undo), and a previous file with bad lines is set aside; a directory spelled otherwise is renamed too. |
| 2026-09-30 | **Durability and undo** (#115, from the QA review): every folder on the way to a file is listed, so it gets its date back (FR-19) and is synced; folders already at the destination keep their own dates; a destination gone before the final flush is a durability error; picked files named like Secopy's unfinished copies are skipped as its own; undo removes a copy only while it's the same file (device and inode); a replaced file's old version that can't be put back from the archive is named in the file's failure. |
| 2026-09-30 | **Reports, CLI and queue results** (#116, from the QA review): Save report… writes both files through a temporary name and its JSON replaces only a Secopy report; a report whose name is taken gets " (2)"; settings that can't be saved aren't used; importing over the selected copy preset loads the imported one on New copy; the CLI exits 2 when nothing matches and writes --report for a mirror; a queue says every job finished only when each is complete, and one that couldn't be saved isn't a ✓; a job's notification says when its report wasn't saved (the copy itself stays Complete). |
| 2026-09-30 | **UI robustness** (#117, from the QA review): unsaved mirror edits are asked about before a tab, Settings, the View menu or a finished Preview takes the screen; a second click while a queue change or Add to queue is on its way does nothing (jobs are addressed by place); a job shows 0 % until its first figures; a summary that can't be loaded goes back to the job's section with why; a page of the finished list that failed is asked for again; the menu bar panel never shows an older view over a newer one; Tab stays inside a dialog. |
| 2026-09-30 | **Jobs and the queue, lower-risk items** (#134, QA review P2): the Mac stays awake for a whole job (removals, undo, report) and a preview's comparison; an archive is deleted outside the lock the window's commands take, marked "preparing" so nothing else starts meanwhile (a queue cancel after the deletion still stops the job; the deletion asked for stands); Start and Verify's Start check and start under the queue's lock; everyone waiting for a job waits for its end; undo says when the checksum file stays. |
| 2026-09-30 | **Engine, lower-risk items** (#135, QA review P2): the space check counts each file in whole allocation blocks (`f_frsize`: a cluster on exFAT); FIFOs, sockets and devices under a source are listed as skipped, like links (not as unread, which would stop every mirror's removals); the report says "Checksum file: none (nothing copied)". |
| 2026-09-30 | **Mirror archive, lower-risk items** (#136, QA review P2): the clean-up's errors are said (summary, queue, CLI) and the run isn't Complete; a run folder's name holds its UTC offset (`2026-09-30 14.30.00 +0200`), older names read as local time with the repeated DST hour taken as its later time (never removed early); a removed file never replaces one already archived (no-replace move; without it, the name is reserved first by creating it); the clean-up's result is in the saved report too; `Deleted::Archive` no longer repeats the days. A clock that was wrong when a run was archived still ages it by that clock. |
| 2026-09-30 | **App state and CLI, lower-risk items** (#137, QA review P2): a settings, presets or queue file from a newer Secopy is left as it is (not set aside), the defaults are used and nothing is saved over it; a queued copy with keys this version doesn't know is kept whole as a job for a newer Secopy; nothing starts or is queued while a pick is scanned; the CLI's mirror dry run counts files that will fail apart. Mirror presets stay editable while a job runs (switching to Delete then offers "at the next run"): a pending archive deletion ends when the destination changes (#101), so a queued mirror can't delete another destination's archive. |
| 2026-09-30 | **UI, lower-risk items** (#138, QA review P2): each New copy view carries a revision made under the session's lock, and the window keeps the newest (answers can arrive out of order); a job added from New copy isn't added again until New copy changes. Accepted risks from the QA review, not fixed: another app writing into the destination during a job, a wrong clock when a mirror run was archived, a case-sensitive origin with names differing only in case, a file reappearing in the origin mid-run, SMB understating large allocation units. |
| 2026-09-30 | **Import across versions** (#149): the file's `app` version is only for the words ("Made by Secopy 0.19.0; this is 0.17.6."); what imports is decided by the keys. A higher `secopy` format is refused whole, as before. Unknown settings are left out and listed; missing ones take their defaults and are listed. A preset with a key this Secopy doesn't know (nested ones too) isn't imported: without it, it would copy or mirror differently than where it was made. |
| 2026-09-30 | **Include the directory off by default** (#152): a pick copies what's inside the directory; ticking the box copies the directory itself. A preset keeps its own choice; a new preset starts off. |
| 2026-10-01 | **ASC MHL** (#154, FR-57): optional, off by default, next to the `.xxh64` file, written only (Verify doesn't read it), for copies only. An existing history is continued as the standard asks, the source's copied with the files by the normal copy and then appended to. Files already in the destination that no history records are read and recorded (guidelines §2.7). A history that changed during the job is never written over; a cancelled copy writes none. A path an earlier history recorded only as md5/sha1/c4 gets Secopy's xxh64 as `original` (not verified). Checked end to end with the ASC's reference tool: its `verify` and XSD checks pass, it appends to Secopy's history and Secopy continues its, nested histories included. |
| 2026-10-01 | **Always ignore when copying** (#158, FR-12–14): the fixed system-file list becomes a Settings list of name patterns (`*`, `?`, case ignored; no paths), defaulting to it, with Restore defaults. Copies and mirrors follow it, and a mirror never removes, archives or compares an ignored file in its destination; Verify and ASC MHL follow it too. Secopy's own working files stay skipped outside the list. A queued job uses the list at its turn. |
| 2026-10-02 | **Each copy and preset's own ignore list** (#164, FR-12): copy presets, mirror presets, New copy runs and queued jobs get Also ignore, added to Settings' list (never switching one of its patterns off). New copy's comes from the preset and marks it changed; a queued job keeps its own and takes Settings' list at its turn. Each list holds up to 128 patterns (Settings' went down from 200). An empty list isn't written, so files are as older versions wrote them; a preset with a list is refused by an older Secopy (#149). |
| 2026-10-03 | **Copy and verify in parallel** (#170, §7.5): verify stays parallel with the copy by default (fastest on SSD destinations; the source is read once either way). A "copy everything, then verify" mode for HDD/RAID destinations and a drive-cache flush before each read-back are recorded for the performance work, to be decided with measurements. §7.2 now says which I/O hints and per-device defaults are planned rather than built. |
| 2026-10-03 | **One word per thing** (#172): every text reviewed; "directory", never "folder"; "source", "destination" and "origin"; "cancel" a job (a job that can't go on "stopped"), "stop" a queue; "checksum", not "hash"; "same size and date", not "identical". The terms table in docs/i18n.md is the reference, and translations keep the same pairs. |
| 2026-10-03 | **Spanish** (#84 follow-up): `es.json` (Spain Spanish, "tú", the terms table in docs/i18n.md), chosen by the Mac's language like English; Rust's own words (menus, the menu bar icon) use its number format; the bundle lists `en`/`es` so macOS shows its panels and the .secopy file type in Spanish. A test keeps every catalog's keys, placeholders, plural forms and edge spaces in step with English. The user guide stays English for now. |
| 2026-10-03 | **XXH128 for every hash** (#178): XXH128 (XXH3 128-bit, seed 0, as `xxhsum -H2`) replaces xxHash64 in the copy, the read-back, Verify, the mirror's Paranoid comparison, the checksum files (`.xxh128`, 32 hex digits, `xxhsum -c` checks them), ASC MHL (`<xxh128>`) and reports. Beta: old `.xxh64` files aren't read; an old mirror checksum file is an ordinary file (archived like any file the origin lacks), and a mirror's new checksum file starts empty; an ASC MHL file recorded only as xxh64 gets its XXH128 as `original`. Library: `twox-hash` 2.1.4, measured on an M2 Max at 39.9 GB/s streaming (xxh64 was 15.6; `xxhash-rust`'s XXH128 26.8; the official C crate 17.1). Checked end to end: `xxhsum -c`, and the ASC reference tool verifies Secopy's history and Secopy continues the tool's. |
| 2026-10-04 | **Language setting** (#181): Settings › General › Language, Automatic by default (the Mac's first language with a catalog, English otherwise, read from the system's own setting so Secopy's choice never hides it) or a chosen one, whatever the Mac's is. Saving (or importing) switches Secopy's words at once (what's on screen is drawn again in place, the menus are rebuilt on the main thread with the File items' state kept); macOS's own windows follow from the next launch through the app's `AppleLanguages`, which Automatic removes. An unknown language reads as Automatic; it travels with exported settings. |
| 2026-10-04 | **Documentation review for 1.0** (#190, with Fable 5.1 and GPT-6 Astra): the sections above describe Secopy as built. Clarifications to earlier entries: the per-file speed and ETA of 2026-09-26 became bytes and a bar per active file, files up to 8 MiB grouped by size (§5.3); Paranoid compares the XXH128 of both sides, not bytes (#178); OS fast paths and benchmarks were never built (§7.2, §9); the 0.x series was the beta. |
| 2026-10-04 | **Code review for 1.0** (#192, with Fable 5.1 and GPT-6.1 Sol High): ASC MHL writes on file systems that can't sync a directory (only a device error fails it, as for checksum files); free space from 64-bit `statfs` counts (`statvfs`'s wrapped past 16 TiB); Verify reads a file once when two checksum files spell it in different Unicode forms, and leaves out a second report of the same second; text and JSON reports are saved as a pair; the JSON report has `format: 1` and a mirror's `mode` is `mirror`; a data file that can't be read just now is left as it is and not saved over (only a damaged one is set aside); the CLI's `--check` writes `--report` and refuses copy options, and `--archive-days` starts at 1. From GPT-6.1 Sol: nothing is written through a symlinked directory in the destination (a link into the source let Overwrite replace a source file); checksum files and reports are written whole under a temporary name, then named; ASC MHL reads `<previousPath>` as the standard has it, compares history names in one Unicode form, and an undo removes a history brought from the source; a mirror's failed rename is said and the run isn't Complete; reports carry ASC MHL details and what Cancel's removal did; a queue change and Start don't interleave; paths keep spaces at their end; the CLI exits 1 for a differing file left as it was and refuses copy options with `--mirror`; the mirror preview lists every change. Declined: exFAT's check-then-rename fallback (no exclusive rename there; Secopy's own jobs are kept apart by the partial file's lock), a report save that fails leaving the copy Complete (decision of 2026-09-30), cancelling a superseded scan, and larger refactors. |
| 2026-10-05 | **"Not found"** (#199): a directory that isn't there is said one way everywhere, its role then its path: Source / Origin / Destination / Directory “path” not found. (es: “… no encontrado”); never "not connected" or "isn't there any more", whether its drive is unplugged or the directory deleted. Keys a saved queue may hold (`errors.source.gone`, `errors.source.notConnected`) stay in the catalogs with the new wording. A mirror's error is cleared when another mirror is selected. |
