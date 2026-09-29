# Inventory: English text the Rust side sends to the UI or shows natively (#84, PR 2)

Branch `feat/84-i18n`, commit `92d7c51`. Line numbers are for that commit. Only non-test code is
listed; tests are counted at the end.

Placeholder notation:

- `{path}` is a path from `show()` / `display()`.
- `{io}` is an `std::io::Error` Display: the OS text, e.g. `Permission denied (os error 13)`.
  It is English from libc/std and can't be translated by a catalog. Map it by `ErrorKind`/errno,
  or pass it through as a raw `{os}` argument.
- `{serde}` is a serde_json error Display, e.g. `expected value at line 1 column 1`.
- `sentence(x)` (`dto.rs:344`) upper-cases the first letter of engine text.

Shared means the same `String` (or the same engine Display) also goes into the text/JSON report
(`secopy-core/src/report.rs`) or the CLI (`crates/secopy-cli`). Those must stay English there, so
the app has to map the engine *variant* to a code and must not translate the Display text.

---

## 1. Command errors (`Result<_, String>` → rejected promise → `messageOf(e)` in the UI)

Every command that takes the `blocking()` path (`commands.rs:966-973`) can also reject with
`JoinError::to_string()` (tokio: `task {id} panicked with message …` / `task {id} was cancelled`).
`pick_source` rejects with `tauri::Error` Display (`commands.rs:992`) or a JoinError (`:995`). These
are internal errors that should get one code, e.g. `error.internal`.

### 1a. Texts, by where they are written

| # | File:line | Exact text | Placeholders | Commands that return it |
|---|---|---|---|---|
| E1 | commands.rs:192, commands.rs:1739, store.rs:218 | `That preset no longer exists.` | – | select_copy_preset, export_copy_preset, edit_copy_preset (via `CopyPresets::edit`) |
| E2 | commands.rs:211 | `Couldn't save the preset: {e}` | e = `Store::save` error `{path}: {io}` (store.rs:675) | update_copy_preset, save_copy_preset_as, create_copy_preset, edit_copy_preset, delete_copy_preset |
| E3 | commands.rs:228, :244 | `Wait until the scan finishes.` | – | update_copy_preset, save_copy_preset_as |
| E4 | commands.rs:230 | `No preset is selected.` | – | update_copy_preset |
| E5 | commands.rs:247 | `A preset saves a directory as its source; pick a directory first.` | – | save_copy_preset_as |
| E6 | commands.rs:308 | `Couldn't save the settings: {e}` | e = `{path}: {io}` (store.rs:675) | set_settings |
| E7 | commands.rs:315, :438 | `The queue is running.` | – | start_job (315); every `change_queue` user (438): add_to_queue, remove_from_queue, move_in_queue, clear_queue, set_queue_on_failure, add_check_to_queue, add_mirror_to_queue |
| E8 | commands.rs:321 | `Nothing to copy, or something blocks the copy.` | – | start_job |
| E9 | jobs.rs:189 | `A copy is already running.` | – | start_job, run_mirror, start_check (via `Jobs::start` / `start_work`); also a queue reason (commands.rs:818-819) |
| E10 | commands.rs:334 | `No files failed.` | – | retry_failed |
| E11 | commands.rs:339-342 | `The source isn't there any more ({path}). Connect the card again to retry.` | path = the source directory or first file | retry_failed. Says "card", which breaks the copy rule. |
| E12 | commands.rs:444 | `That job is no longer in the queue.` | – | remove_from_queue, move_in_queue (bad index) |
| E13 | commands.rs:448, :720 | `Couldn't save the queue: {e}` | e = `{path}: {io}` | all `change_queue` commands (448). Also becomes `QueueSummaryView.saveError` (720). |
| E14 | commands.rs:457 | `Set up a copy first: a source, a destination and something to copy.` | – | add_to_queue |
| E15 | commands.rs:523, :1283, :1325 | `A copy or the queue is already running.` | – | run_queue (claim_queue_run), run_mirror, start_check |
| E16 | commands.rs:1164 | `Couldn't save the mirror: {e}` | e = `{path}: {io}` | create/edit/delete_mirror_preset |
| E17 | commands.rs:415 (used :1197, :1347) | `The mirror preset no longer exists.` (`PRESET_GONE`) | – | preview_mirror, add_mirror_to_queue; also a queue reason and `lastError` |
| E18 | session.rs:639 | `{drive} isn't connected.` | drive = first component under `/Volumes` | preview_mirror (mirrors.rs:30), check_directory and start_check's plan (commands.rs:907); also `pickProblem` and queue reasons |
| E19 | session.rs:641 | `{path} isn't there any more.` | path | same as E18 |
| E20 | commands.rs:1288 | `Preview the mirror first.` | – | run_mirror |
| E21 | commands.rs:1292 | `The mirror changed since its preview. Preview it again.` | – | run_mirror |
| E22 | commands.rs:910 | `Choose a directory.` | – | check_directory; also a queue reason |
| E23 | commands.rs:912 | `{dir}: {io}` | dir = the directory; io = `check::plan` failure (check.rs:132 `read_dir`) | check_directory; also a queue reason |
| E24 | commands.rs:1329 | `Choose the directory again.` | – | start_check |
| E25 | commands.rs:902 (used :1333) | `No checksum files here: there's nothing to verify.` (`NOTHING_TO_VERIFY`) | – | start_check; also a queue reason (:789) |
| E26 | commands.rs:1466 | `That job has no report.` | – | queue_save_report |
| E27 | jobs.rs:316 | `There is no report yet.` | – | save_report |
| E28 | jobs.rs:575 | `The copy is still running.` | – | save_report, queue_save_report |
| E29 | jobs.rs:46 (used :577) | `Secopy hit an internal error before the job ended, so there is no report.` (`NO_REPORT`) | – | save_report, queue_save_report; also `reportError` |
| E30 | jobs.rs:580 | `{io}` with no prefix | fs::write error | save_report, queue_save_report |
| E31 | store.rs:244 | `The preset needs a name.` | – | create/edit_copy_preset, save_copy_preset_as; also an Import `problem` |
| E32 | store.rs:357-359 | `The name is too long: 200 characters at most.` | MAX_NAME = 200 | copy and mirror create/edit, save_copy_preset_as; also an Import `problem` |
| E33 | store.rs:272-275 | `There is already a preset called “{name}”.` | name = the trimmed typed name | create/edit_copy_preset, save_copy_preset_as; also inside ImportDone.message |
| E34 | store.rs:598 | `The source must be a full path, like /Volumes/CARD_A/DCIM.` | – | create/edit_copy_preset; also an Import `problem` |
| E35 | store.rs:488, commands.rs:1755 | `That mirror no longer exists.` | – | edit_mirror_preset, export_mirror_preset |
| E36 | store.rs:525-528 | `There is already a mirror called “{name}”.` | name | create/edit_mirror_preset; also inside ImportDone.message |
| E37 | store.rs:538 | `The mirror needs a name.` | – | create/edit_mirror_preset; also an Import `problem` |
| E38 | store.rs:585 (called :541) | `The origin must be a full path, like /Volumes/SSD/Footage.` | built from fragments `what`="origin" and `example` | create/edit_mirror_preset; also an Import `problem` |
| E39 | store.rs:585 (called :542) | `The destination must be a full path, like /Volumes/NAS/Footage.` | `what`="destination" | same as E38 |
| E40 | store.rs:545, mirror.rs:229 | `The origin and the destination are the same directory.` | – | create/edit_mirror_preset (store); preview_mirror (engine, mirror.rs); also an Import `problem` and a queue reason |
| E41 | store.rs:548, mirror.rs:232 | `The destination can't be inside the origin.` | – | same as E40 |
| E42 | store.rs:551, mirror.rs:235 | `The origin can't be inside the destination.` | – | same as E40 |
| E43 | store.rs:554 | `Keep archived files for at least 1 day.` | – | create/edit_mirror_preset; also an Import `problem` |
| E44 | store.rs:557 | `Keep archived files for at most 36,500 days.` | 36,500 is hard-coded with an English thousands separator | same as E43 |
| E45 | commands.rs:1719 | `Choose something to export.` | – | export_all |
| E46 | transfer.rs:79 | `That isn't a file name.` | – | export_all, export_copy_preset, export_mirror_preset |
| E47 | transfer.rs:102 | `The file couldn't be saved: {io}` | – | the three export commands |
| E48 | commands.rs:1675 (used :1770, :1798) | `Import it when the current job has finished.` (`IMPORT_WAITS`) | – | open_import, apply_import |
| E49 | transfer.rs:109 | `The file can't be opened: {io}` | – | open_import |
| E50 | transfer.rs:23 (used :111, :131, :133, :138) | `This isn't a Secopy file.` (`NOT_SECOPY`) | – | open_import |
| E51 | transfer.rs:114, :121 | `This file is too big to be a Secopy file.` | – | open_import |
| E52 | transfer.rs:119 | `The file can't be read: {io}` | – | open_import |
| E53 | transfer.rs:140-142 | `This file was made by a newer Secopy (format {format}). Update Secopy to import it.` | format = the file's `secopy` number | open_import |
| E54 | transfer.rs:151-153 | `This file has too many presets. Secopy imports up to 1000 of each kind.` | MAX_PRESETS = 1000, unformatted | open_import |
| E55 | transfer.rs:24 (used :162) | `There is nothing in this file to import.` (`NOTHING`) | – | open_import |
| E56 | commands.rs:1802 | `There is no file to import.` | – | apply_import |
| E57 | commands.rs:1677 (used :1809) | `Your presets or settings changed since this file was opened. Open it again to see what it would change.` (`IMPORT_CHANGED`) | – | apply_import |
| E58 | mirror.rs:114 (core) | `The origin isn't there: {path}` | origin | preview_mirror (not reachable after mirrors.rs:27-31 checks is_dir; a race only) |
| E59 | mirror.rs:121-124 (core) | `The destination's .secopy-archive isn't a directory (it's a link or a file). Move it away, or choose to delete removed files.` | ARCHIVE_DIR | preview_mirror; also a queue reason |
| E60 | mirror.rs:130 (core) | scan error `e.to_string()`, **not** sentence'd: `{io}` or `a drive root has no directory name; copy only its contents instead` (scan.rs:284) | – | preview_mirror; also a queue reason |
| E61 | mirror.rs:136 (core) | `Blocker` Display, **lowercase, not sentence'd** (see §3, B1-B4): `the destination is not an existing directory`, `can't write to the destination: {io}`, `the destination is the source directory or inside it`, `not enough free space: …` (preflight only returns B1-B3) | – | preview_mirror; also a queue reason |
| E62 | mirror.rs:159 (core) | `Cancelled.` | – | preview_mirror, when Cancel stops the deep check. **The UI compares this text** (MirrorScreen.svelte:126). |

### 1b. Commands with no text errors of their own

These only fail with the JoinError above: scan_source, set_include_folder, clear_source,
set_filter, set_destination, set_conflicts, finished_page, job_summary, queue,
queue_finished_page, mirror_presets, mirror_preview_page, app_start, recent_destinations,
set_mode.

These return no `Result`: pause_job, resume_job, cancel_job, job_running, menubar_view,
open_main_window, quit_app, set_menu_state, hide_to_menu_bar, cancel_mirror_preview,
take_opened_file.

Their views still carry text (see §2).

### 1c. Command → possible errors (quick map)

- `select_copy_preset`: E1
- `update_copy_preset`: E3, E4, E2
- `save_copy_preset_as`: E3, E5, E31, E32, E33, E34 (the source is a picked full path, so E34 can't happen), E2
- `create_copy_preset`: E31, E32, E33, E34, E2
- `edit_copy_preset`: E31, E32, E33, E34, E1, E2
- `delete_copy_preset`: E2
- `set_settings`: E6
- `start_job`: E7, E8, E9
- `retry_failed`: E10, E11
- `add_to_queue`: E14, E7, E13
- `remove_from_queue`, `move_in_queue`: E7, E12, E13
- `clear_queue`, `set_queue_on_failure`, `add_check_to_queue`: E7, E13
- `add_mirror_to_queue`: E17, E7, E13
- `run_queue`: E15
- `save_report`: E27, E28, E29, E30
- `queue_save_report`: E26, E28, E29, E30
- `create_mirror_preset`: E37, E32, E36, E38-E44, E16
- `edit_mirror_preset`: the same plus E35
- `delete_mirror_preset`: E16
- `preview_mirror`: E17, E18/E19 (origin or destination gone), E58-E62, E40-E42 (engine copies)
- `run_mirror`: E15, E20, E21, E9
- `check_directory`: E18/E19, E22, E23
- `start_check`: E15, E24, E25, E9
- `export_all`: E45, E46, E47
- `export_copy_preset`: E1, E46, E47
- `export_mirror_preset`: E35, E46, E47
- `open_import`: E48, E49-E55
- `apply_import`: E48, E56, E57. Save failures don't reject: they go into `ImportDone.message`, §2.

---

## 2. DTO fields that carry text

### SessionView (dto.rs:15)

**`pickProblem`** (dto.rs:31). Set by `Session::no_source` (session.rs:228-229). It takes:

- `gone(path)`, session.rs:241 → E18 `{drive} isn't connected.` / E19 `{path} isn't there any more.`
- session.rs:128: `nothing was picked`. Lowercase and never sentence'd; only for an empty `paths` list.
- session.rs:138: `Pick one directory, or only files — not both.`
- Scan failure, session.rs:682: `sentence(io::Error Display)`. That is either the OS text from
  `std::path::absolute`, `fs::metadata` or `fs::canonicalize` (scan.rs:189, 190, 278), e.g.
  `Permission denied (os error 13)`, or scan.rs:284 as a sentence:
  `A drive root has no directory name; copy only its contents instead`.

The queue reuses it as a job's reason (queue.rs:116-117).

**`source.label`** (SourceView, dto.rs:40). Set at session.rs:215 by `label()` (session.rs:625-631)
and at session.rs:395. It takes:

- the directory's path, or the single file's path
- `{n} files` (session.rs:629)
- `Retry: {n} failed files` (session.rs:395; English plural even for 1)

**Shared:** this becomes `Ready.label`, the report's `source` (jobs.rs:848, `JobMeta.source`,
report.rs:195), and the menu bar panel's `from` for file sources (jobs.rs:296).

**`source.problems`** (dto.rs:62). Built at session.rs:529-534 as `{path}: {message}`, the first
20. `message` is `ScanProblem.message`:

- scan.rs:180: `not a regular file`
- scan.rs:181, :257: `{io}`
- scan.rs:231: walkdir::Error Display, `IO error for operation on {path}: {io}` or
  `File system loop found: {child} points to an ancestor {ancestor}`

**Shared:** the report's "unread" list (report.rs:219-225), jobs.rs:522 and the mirror guard
(mirror.rs:191-199).

**`source.extensions[].label`** (ExtensionView, dto.rs:74). Set at session.rs:495-497: `.{ext}`, or
`(no extension)`.

**Data, not text:** `source.folder`, `source.rootDir`, `destination.path`, `destination.copyRoot`.

### DestinationView (dto.rs:82)

**`blocker`** (dto.rs:87):

- session.rs:581: `sentence(Blocker Display)` from preflight → B1, B2, B3 (§3):
  `The destination is not an existing directory`, `Can't write to the destination: {io}`,
  `The destination is the source directory or inside it`. No full stop.
- session.rs:589: B1, when there is no source yet and the destination isn't a directory.
- session.rs:592: B2 `Can't write to the destination: {io}`.

The queue reuses it as a job reason (queue.rs:137-141).

**`fsKind`** (dto.rs:90). `fs_label` (session.rs:666-677): `APFS`, `Mac OS Extended`, `exFAT`,
`FAT32`, `NTFS`, `network (SMB)`, `network (NFS)`, and otherwise the Debug name: `Ext4`, `Btrfs`,
`Xfs`, `ReFs`, `Other("…")`. It is shown inside `t("copy.free", {kind})`. Only the two
`network (…)` values and the Debug fallback are really words.

**`problems[].reason`** (FileProblemView, dto.rs:109). Set at session.rs:568 as
`p.kind.to_error().to_string()`. **Not sentence'd**, so it is lowercase. It is the FileError
Display of `InvalidName` (NameProblem, N1-N5), `TooLarge` (F9), `NameClash` (F6) or `InTheWay`
(F10). **Shared:** the same FileError goes to the report per file.

### PlanView (dto.rs:124)

**`blocker`** (dto.rs:129). Set at session.rs:621:
`sentence(NotEnoughSpace)` = `Not enough free space: {needed} bytes needed, {free} bytes free`.
The byte counts are raw integers, unformatted. The queue reuses it as a job reason
(queue.rs:140).

### ProgressView (dto.rs:135)

**`fatal`** (dto.rs:164). The UI shows it as `t("progress.stopped", {why})`, i.e. `Stopped: {why}`.
It takes:

- jobs.rs:781: `sentence(FatalError Display)` → `The destination drive is full`,
  `The destination is no longer available; was it disconnected?`,
  `The source is no longer available; was it disconnected?`
- jobs.rs:640, :654: `INTERNAL_ERROR` (jobs.rs:44) = `Secopy hit an internal error`

**Data:** `active[].name` and `active[].path` are file names.

### FinishedRow (dto.rs:202): `reason` (dto.rs:214)

Used by the finished list (`finished_page`, `queue_finished_page`), `SummaryView.failures` and
`MirrorSummaryView.removalFailures`. It is shown as the tooltip (FinishedList.svelte:134) and in
Summary lists. `status` is already a code.

It takes:

- jobs.rs:977: check rows with `FileError::Changed` →
  `sentence` = `Changed since it was copied (expected {hex}, found {hex})`
- jobs.rs:982: `Already at the destination (not checked)` (SkipReason::Identical)
- jobs.rs:986: `A different file with this name was kept` (SkipReason::Differs)
- jobs.rs:988: `sentence(FileError Display)` for every other failure (F1-F17, §3)
- jobs.rs:995-998: for a check's failed file, `Listed in {checksum file}.` is appended →
  `{why} Listed in {file}.`, or just `Listed in {file}.` for `Missing`, which has no reason
- jobs.rs:522 (summary failures only): `Couldn't be read: {ScanProblem.message}`. The
  placeholder is lowercase engine text such as `not a regular file` or `{io}`.
- jobs.rs:532 (summary failures only): `Empty directory not created: {io}`
  (why = job/mod.rs:317 `e.to_string()`)
- jobs.rs:935 (mirror `removalFailures`): `Not removed: {e}`. `e` is `Removal.result`:
  mirror.rs:502 `It changed after the preview, so it was kept.` (capital letter mid-sentence), or
  mirror.rs:517 `{io}`.

**Shared:** the report writes its own lowercase texts for skips (report.rs:158-164) and uses the
same FileError Display for failures (report.rs:157, 247-252), `unread` messages, `dir_errors` and
`not_removed` reasons.

### SummaryView (dto.rs:237)

**`stoppedBecause`** (dto.rs:240). Set at jobs.rs:491-494: `INTERNAL_ERROR`, or
`sentence(FatalError)` (as for `fatal`). It is shown via `summary.headline.stopped`
(headline.ts:11, :45), the menu bar's `ended.text` (menubar.rs:588) and queue reasons
(commands.rs:836 `full_stop` adds a `.`).

**`durabilityError`** (dto.rs:251). Set at jobs.rs:503 from core job/mod.rs:372: raw `{io}`. The UI
wraps it in `summary.durabilityError`. **Shared:** report.rs:431 `NOT CONFIRMED SAVED TO DISK: {e}`.

**`checksumError`** (dto.rs:265). Set at jobs.rs:545. It takes:

- core job/mod.rs:344: `{io}` from `checksum_file::write`
- app jobs.rs:757: `the mirror's checksum file: {io}`, lowercase. **An app-made English text
  written into `JobReport.checksum_error`**, so it also lands in the report
  (report.rs:424 `Checksum file NOT written: the mirror's checksum file: …`).

The UI wraps it in `summary.checksumError`.

**`reportError`** (dto.rs:271). Set at jobs.rs:550-560 as a `"; "`-joined list of:

- jobs.rs:878: `{reports_dir}: {io}`
- jobs.rs:630: `NO_REPORT` (E29)
- jobs.rs:779: `next to the checksum file: {io}` (lowercase)

It is joined by concatenation. The UI wraps it in `summary.reportError`. (`Done.report_file` starts
as `Err(String::new())` at jobs.rs:682 and :763, but is always overwritten.)

**Other fields:**

- `failures`: FinishedRow, above.
- `copyRoot`, `checksumFile`, `reportFile`: paths.
- `mirror.nothingRemoved`, `mirror.removalFailures`, `check.problems`: below.

### MirrorSummaryView (dto.rs:474)

**`nothingRemoved`** (dto.rs:486). Set at jobs.rs:941 from `mirror::finish` Err (core mirror.rs):

- 463-465: `Files deleted in the origin were left in the destination: the mirror was cancelled.`
- 469-471: `Files deleted in the origin were left in the destination: the mirror stopped.`
- 474-477: `Files deleted in the origin were left in the destination: {failed} {file|files} failed.`
  (English plural choice inside the engine)
- 481-483: `Files deleted in the origin were left in the destination: the copy didn't end cleanly.`

**Shared:** the report (`MirrorPart.nothing_removed`, mirror.rs:434 → report.rs:478) and the CLI.

`removalFailures[].reason`: see FinishedRow (`Not removed: …`).

### CheckView (dto.rs:283) and CheckSummaryView (dto.rs:298): `problems`

- CheckView: commands.rs:1309-1316. CheckSummaryView: jobs.rs:456-464.
- Format: `{file}:{line}: {reason}`, or `{file}: {reason}`.
- `reason` is `check::Problem.reason`: check.rs:52, 59, 69, 157, 190, 210 (C1-C5, §3).

**Shared:** the report (report.rs:330-338, 450-451) and the CLI (main.rs:482-485).

### StartView (dto.rs:362): `warnings` (dto.rs:373)

Set at commands.rs:103-109 from `Store::load` (store.rs:635-651), via `set_aside`
(store.rs:654-668), and from `CopyPresets::repaired` notes:

- store.rs:660-663: `{name} couldn't be read ({why}). It was set aside as {aside} in {dir}, and the defaults are used.`
- store.rs:664-666: `{name} couldn't be read ({why}) or set aside ({e}). The defaults are used.`
- The placeholders:
  - name: an internal file name (`settings.json`, `profiles.json`, `state.json`, `queue.json`,
    `mirrors.json`)
  - aside: `{name}.damaged-YYYYMMDD-HHMMSS`
  - why: `{io}` (:639), or `it is from a newer Secopy, version {n}` (:643), or `{serde}` (:647)
  - e: `{io}`
- store.rs:301-303: `A copy preset in profiles.json had no name; it is now “{name}”.`
- store.rs:305-307: `Two copy presets in profiles.json were called “{wanted}”; the second is now “{name}”.`
- store.rs:310-313: `The copy preset “{name}” had a source that isn't a full path ({source}); choose its directory again.`

### QueuedJobView (dto.rs:389)

- **`kind`**: `copy`/`mirror`/`check`/`unknown` (commands.rs:363, 376, 385, 395, 404). These are
  codes already.
- **`source`**: a path, the origin, or commands.rs:367 `{n} files` (English plural, even though a
  single file shows its path). Shown inside `queue.name.job`.
- **`lastError`** (dto.rs:396): `Entry.last_error` (saved, §5), or commands.rs:389 `PRESET_GONE`
  (E17), or commands.rs:408 `Needs a newer Secopy.`.
- **`name`**: a mirror preset name (data).

### QueueResultView (dto.rs:421): `reason` (dto.rs:425)

It takes:

- commands.rs:645: `Not run: the queue stopped.`
- commands.rs:584: `QUEUE_STOPPED` (commands.rs:417) = `Secopy hit an internal error; the queue stopped.`
- commands.rs:738: `queue::prepare` Err. That is a `pickProblem` text (queue.rs:116-117); or
  `why_not` (queue.rs:136-142), i.e. `destination.blocker` / `plan.blocker` (sentence'd B1-B4), or
  queue.rs:141 `Nothing to copy.`
- commands.rs:754: `PRESET_GONE`
- commands.rs:767, :814, :828: `Cancelled.`
- commands.rs:769: the preview_mirror errors (E18/E19, E58-E62)
- commands.rs:773: the mirror guard (M1-M3, §3), and the run is not started
- commands.rs:786: `plan_check` errors (E18/E19, E22, E23)
- commands.rs:789: `NOTHING_TO_VERIFY` (E25)
- commands.rs:795: `Needs a newer Secopy.`
- commands.rs:819: `Jobs::start_work` error, E9 `A copy is already running.`
- commands.rs:830: `failure_reason()` (commands.rs:854-900). It is **built from fragments**:
  - check: `{n} changed`, `{n} missing`, `{n} couldn't be read` and `{n} checksum file problems`
    (always plural, even for 1), joined with `", "`, then `"."`
  - `{n} file failed.` / `{n} files failed.` (:872)
  - `{n} item couldn't be read.` / `{n} items couldn't be read.` (:876)
  - `{n} file couldn't be removed.` / `{n} files couldn't be removed.` (:883)
  - `{n} empty directory couldn't be created.` / `{n} empty directories couldn't be created.` (:891)
  - `full_stop("The checksum file couldn't be written: {checksumError}")` (:894)
  - `The destination couldn't confirm the files are saved: {durabilityError}` (:897, no full stop
    added, unlike its neighbour)
  - `It didn't complete.` (:898)
- commands.rs:832-837: `full_stop(stoppedBecause)`, e.g. `The destination drive is full.` /
  `Secopy hit an internal error.`, or `Stopped.`

`full_stop` (commands.rs:845-851) appends `.` unless the text ends with `.`, `!` or `?`. That is
punctuation logic on translated text.

### QueueSummaryView (dto.rs:432): `saveError` (dto.rs:439)

- commands.rs:720 (via :604, :685-687, :696-697): `Couldn't save the queue: {path}: {io}`
- commands.rs:553: `QUEUE_STOPPED`

### MirrorPreviewView (dto.rs:500)

- **`guard`** (dto.rs:518): set at commands.rs:1232 from `MirrorPlan.guard` (M1-M3, §3). The
  queue uses it as a reason (commands.rs:773).
- `name`, `origin`, `destination`: data.

### PreviewRow (dto.rs:532): `reason` (dto.rs:537)

Set at commands.rs:1253-1256 and :1269: `New in the origin`, `Changed in the origin`,
`Contents differ`, `Deleted in the origin`.

It is fully determined by `kind` plus the engine's `Change` (New / Changed / ContentsDiffer /
removed), so it can become a code with no args.

### Export messages (the UI's `said`/`info`: App.svelte:160, :267-268; MirrorScreen.svelte:40; CopyPresetsScreen.svelte:34)

- commands.rs:1729-1732 (`export_all` → `String`): `Exported {what}.`
- commands.rs:1748 (`export_copy_preset`): `Exported “{name}”.`
- commands.rs:1764 (`export_mirror_preset`): `Exported “{name}”.`

`what_line` (commands.rs:1680-1702) is **built from fragments**:

- `{n} copy preset` + `s`
- `{n} mirror preset` + `s`
- `the settings`
- `nothing` (0 parts)
- `", "` and `" and "` joiners

This covers "1 copy preset, 2 mirror presets and the settings".

### ImportDone (dto.rs:552): `message` (dto.rs:553)

Built at commands.rs:1837-1866, by concatenation. `failed` is a bool.

- Success: `Imported {what}.` (what = `what_line`)
- A save failed, nothing saved yet: `{What} couldn't be saved: {why}`
- A save failed after others saved: `Imported {what}. {What} couldn't be saved: {why}`
- Either failure form can end with ` {Rest} weren't imported.`

The fragments:

- `What`: `The copy presets` / `The mirror presets` / `The settings` (commands.rs:1819, 1826, 1830).
  **Also used for logic**: `match what { "The copy presets" => …, "The mirror presets" => … }`
  (commands.rs:1843, :1850).
- `Rest`: `the mirror presets`, `the settings`, joined with `" and "` and capitalised with
  `rest[..1].to_uppercase() + &rest[1..]` (commands.rs:1862-1864). **A byte slice: it panics if a
  translation starts with a multi-byte character.**
- `why`:
  - `Store::save` error `{path}: {io}` (commands.rs:1891, no prefix)
  - transfer.rs:404, :438 `That preset can't be imported.`
  - transfer.rs:384 `Two presets in the file would replace “{name}”; choose Keep both for one of them.`
  - `CopyPresets::add/edit` / `MirrorPresets::add/edit` errors: E1, E31-E44

### ImportView (transfer.rs:208), returned by `open_import`

**`settings.changes`** (SettingsImport, transfer.rs:220). Built by `settings_changes`
(transfer.rs:466-498) as `{label}: {on|off} → {on|off}`. The labels are:

- `Write the checksum file`
- `Show the count of skipped system files`
- `Save the report next to the checksum file`
- `Notify when a copy finishes`
- `Keep copying in the menu bar`

The `on`/`off` words come from transfer.rs:467. The labels duplicate the Settings screen's words,
which are already in the catalog, so this can become `{setting: code, from: bool, to: bool}`.

**`settings.problem`** (transfer.rs:221). Set at transfer.rs:146:
`The settings in this file can't be read.`

**`copyPresets[].problem` / `mirrorPresets[].problem`** (PresetImport, transfer.rs:237). Set at
transfer.rs:329 from `Unreadable.why`. It takes:

- transfer.rs:184: `This part of the file can't be read.`
- transfer.rs:199: `Its details can't be read ({serde}).`
- `CopyPresets::normalized` errors (transfer.rs:345-348): E31, E32, E34
- `MirrorPresets::normalized` errors (transfer.rs:369-372): E37, E32, E38, E39, E40, E41, E42,
  E43, E44

**`name` / `newName`** (transfer.rs:227, :233). Normally data, but these fallback **texts are put
in the name field**:

- transfer.rs:155: `A copy preset with no name`
- transfer.rs:159: `A mirror preset with no name`
- the section names `The copy presets` / `The mirror presets` (transfer.rs:155, :158, used at
  :183) when the list isn't an array

`newName` also comes from `free_name` → `{name} ({n})` (store.rs:370). This is a naming format
that is saved on import.

**Data:** `fileName`, `paths`, `missing`, `clash`.

### Menu bar panel: PanelView (menubar.rs:51) and Ended (menubar.rs:74)

Built by `panel()` (menubar.rs:133-185), sent by `menubar_view` and the `menubar-view` event.

**`heading`**. It takes:

- menubar.rs:136, :236, :532: `Secopy`
- jobs.rs:287: `Verifying`
- jobs.rs:290: `Mirroring {name}`. The name comes from `strip_prefix("Mirror · ")` on
  `Ready.label` (**text used for logic**; the label is set at mirrors.rs:56).
- jobs.rs:291: `Copying & verifying`
- jobs.rs:292: `Copying`
- menubar.rs:221: `Checking job {n} of {m}`
- menubar.rs:166: `Job {n} of {m} · {heading}`. Skipped when
  `heading.starts_with("Checking")` (menubar.rs:165, **text used for logic**).

**`percent`** (menubar.rs:175): `{p}%` or `…`.

**`files`** (menubar.rs:176): `{done} of {total} files`. The UI also embeds it in
`t("menubar.paused", {files})` (Panel.svelte:44).

**`speed`** (menubar.rs:158): `{bytes}/s`. `bytes()` (menubar.rs:188-200) gives `{n} B` or
`{value:.1} {KB|MB|GB|TB|PB}`, with a decimal point that can't follow the locale.

**`left`** (menubar.rs:159): `{h:mm:ss | m:ss} left` (duration() menubar.rs:203-211).

**`from`**: a path, or `Ready.label` for file sources (`{n} files`, `Retry: {n} failed files`).
**`to`**: a path.

**`ended.text`** (menubar.rs:120-130):

- `Finished: every file done`
- `Finished with problems`
- `Cancelled`
- `Stopped: {why}` (why = `stoppedBecause`)
- `Stopped`

Where the text isn't needed: `Running.heading` and `Status::Finished.why` (menubar.rs:28, :44) are
the internal carriers of the heading and `stoppedBecause`. `Jobs::label()` (jobs.rs:271-274) makes
`Verify · {dir}` (jobs.rs:406) and has no caller outside tests. `Job::label` is used for the
report `source`, which `Report::for_check` ignores.

---

## 3. Engine (`secopy-core`) text that reaches the UI through the app

All of these are **shared with reports and/or the CLI**. Keep the Display impls English and add a
code mapping on the app side, matching on variants and fields.

### IoFailure (error.rs:9-28)

`{message}` is the io::Error Display captured at conversion (error.rs:19). Its `kind` is kept, so
it can be mapped.

### FileError (error.rs:32-69; thiserror)

The UI sees it lowercase in `FileProblemView.reason` and sentence'd in `FinishedRow.reason`. It
also goes to the report (report.rs:157, 247-252) and the CLI.

| # | Variant | Text | Fields |
|---|---|---|---|
| F1 | `ReadSource(IoFailure)` (:33-34) | `cannot read source: {0}` | io |
| F2 | `WriteDest(IoFailure)` (:35-36) | `cannot write destination: {0}` | io |
| F3 | `ReadBack(IoFailure)` (:37-38) | `cannot read back the copy: {0}` | io. check.rs:514 makes `it changed while it was read` (io::Error::other); check.rs:508 passes the OS error. |
| F4 | `HashMismatch{expected,actual}` (:39-40) | `hash mismatch (source {expected}, copy {actual})` | 16-hex strings |
| F5 | `AlreadyExists` (:41-42) | `a file with this name already exists at the destination` | – |
| F6 | `NameClash` (:43-44) | `another file in this copy has the same name (names are compared ignoring case)` | – |
| F7 | `PartialInUse` (:45-46) | `another copy is writing this file` | – |
| F8 | `InvalidName(NameProblem)` (:47-48) | `{0}` → N1-N5 | NameProblem |
| F9 | `TooLarge{limit}` (:49-50) | `the file is larger than the destination drive allows ({limit} bytes)` | u64 raw bytes |
| F10 | `InTheWay{path}` (:51-52) | `something is in the way at {path}` | PathBuf |
| F11 | `SourceChanged` (:53-54) | `the source file changed while it was copied` | – |
| F12 | `Changed{expected,actual}` (:55-56) | `changed since it was copied (expected {expected}, found {actual})` | hex |
| F13 | `Missing` (:57-58) | `missing` | – (the UI shows status `missing`, no reason) |
| F14 | `IsLink` (:60-61) | `is a link, not checked (links aren't followed)` | – |
| F15 | `IsDirectory` (:62-63) | `is a directory, not a file` | – |
| F16 | `NotAFile` (:65-66) | `isn't a regular file, not checked` | – |
| F17 | `Cancelled` (:67-68) | `cancelled` | – (becomes `FileStatus::Cancelled`, no reason) |

### NameProblem (names.rs:11-51)

- N1 `InvalidChar(c)` where c is a control character (:24-26): `the name contains a control character`
- N2 `InvalidChar(c)` (:27-32): `the name contains "{c}", which this drive doesn't allow`
- N3 `Reserved` (:33): `the name is reserved on Windows`
- N4 `TrailingDotOrSpace` (:34-39): `the name ends with a dot or a space, which this drive doesn't allow`
- N5 `TooLong{limit}` (:40-48): `the name is longer than this drive allows ({n} characters)`, where
  n comes from `NameLimit::Bytes|Utf16Units`

### FatalError (error.rs:90-98)

The UI sees it in `ProgressView.fatal`, `SummaryView.stoppedBecause`, the menu bar's `ended.text`
and the queue reasons. **Shared:** report `result` = `stopped: {fatal}` (report.rs:559) and the
CLI (main.rs:421).

- X1 `DiskFull` (:92-93): `the destination drive is full`
- X2 `DestinationGone` (:94-95): `the destination is no longer available; was it disconnected?`
- X3 `SourceGone` (:96-97): `the source is no longer available; was it disconnected?`

### Blocker (preflight.rs:18-40)

The UI sees it in `DestinationView.blocker`, `PlanView.blocker`, preview_mirror errors (raw,
lowercase) and the queue reasons. **Shared:** the CLI (main.rs:150-154).

- B1 `DestMissing` (:29): `the destination is not an existing directory`
- B2 `DestNotWritable(IoFailure)` (:30): `can't write to the destination: {e}`
- B3 `DestInsideSource` (:31-33): `the destination is the source directory or inside it`
- B4 `NotEnoughSpace{needed, free}` (:34-37): `not enough free space: {needed} bytes needed, {free} bytes free`

### ProblemKind (preflight.rs:51-75)

No text of its own. `to_error()` (:67-74) turns it into F8/F9/F6/F10.

### ScanProblem.message (scan.rs:50-53; set at :161-166)

- S1 (:180): `not a regular file`
- S2 (:181, :257): `{io}`
- S3 (:231): walkdir::Error Display (`IO error for operation on {path}: {io}`,
  `File system loop found: …`)

It reaches `SourceView.problems`, `FinishedRow.reason` (`Couldn't be read: …`) and the mirror
guard M1. **Shared:** report `unread` (report.rs:224, 462-466).

### Scan failure (io::Error, `scan()` scan.rs:105)

- `std::path::absolute` (:189), `fs::metadata` (:190), `fs::canonicalize` (:278): `{io}`
- S4 (:282-285): `a drive root has no directory name; copy only its contents instead`

The app sentence's it into `pickProblem` (session.rs:682). mirror.rs:130 passes it raw (E60).

### check::Problem.reason (check.rs:111-117)

- C1 (:52): `not a "<checksum>  <path>" line`
- C2 (:59): `"{hex}" isn't an xxHash64 checksum`
- C3 (:69): `the path can't be read`
- C4 (:157, :190): `couldn't be read: {e}` (walkdir error or `{io}`)
- C5 (:210): `{path} points outside the checked directory`

`check::plan` itself fails with an io::Error from `read_dir` (check.rs:132), which becomes E23.

### MirrorPlan.guard (mirror.rs:92)

It reaches `MirrorPreviewView.guard` and the queue reason. **Shared:** the CLI (main.rs:254-261).

- M1 (mirror.rs:191-199): `{1 item|{n} items} in the origin couldn't be read ({path}: {message}). Nothing is removed from the destination this run.`
  (English plural choice; the message is S1-S3)
- M2 (mirror.rs:384-386): `The origin has no files: every file in the destination would be removed.`
- M3 (mirror.rs:389): `{removals} of the destination's {destination_files} files would be removed.`

### `mirror::plan_watched` errors (mirror.rs:106-221 → `Result<_, String>`)

E58-E62 and E40-E42 (§1). **Shared:** the CLI (main.rs:240).

### `mirror::finish` errors and `Removal.result` (mirror.rs:456-535)

- The four `Files deleted in the origin were left in the destination: …` texts (§2,
  MirrorSummaryView)
- R1 (:502): `It changed after the preview, so it was kept.`
- R2 (:517): `{io}`

**Shared:** report `nothing_removed` / `not_removed` (mirror.rs:421-435) and the CLI (main.rs:279-302).

### JobReport (job/mod.rs:126-150), raw OS text

- `checksum_error` (:132, set :344): `{io}`
- `durability_error` (:147, set :372): `{io}`, device errors only
- `dir_errors` (:149, set :317): `(rel, {io})`

**Shared:** report.rs:203, 208, 211-216, 419-432, 469-474.

### Undone.failed (job/undo.rs:21)

- (:38): `It changed since it was copied, so it was kept.`
- (:57): `{io}`

This reaches the UI only as a count (`UndoneView.failed`). The text goes to the report via
`append_undone` (jobs.rs:955-967, app-made English appended to the report text).

### awake.rs:7

`Secopy is copying files`: the power assertion name macOS shows in `pmset -g assertions` and
Activity Monitor. Native; see §4.

### English that stays in reports only (no change needed; listed so it isn't mistaken for UI text)

- report.rs skip reasons (158-164)
- `result_line` (557-578)
- `with_mirror` (349-350)
- the text layout (395-520)
- jobs.rs:958-965 `append_undone`: `Removed after cancelling: …`, `— NOT REMOVED: {why}`

---

## 4. Native texts (Rust / config)

### App menu (lib.rs:364-464)

- lib.rs:367: `Secopy` (app submenu title; macOS shows the bundle name anyway)
- lib.rs:372: `Settings…` (⌘,)
- lib.rs:380: `Quit Secopy` (⌘Q)
- lib.rs:414: `File` (submenu)
  - :386 `Choose Source…` (⌘O)
  - :393 `Choose Destination…` (⌘D)
  - :400 `Start Copy` (⌘↩)
  - :407 `Cancel Copy` (⌘.)
  - :423 `Import…`
  - :424 `Export…`
- lib.rs:454: `View` (submenu)
  - :457 `Copy` (⌘1)
  - :458 `Mirror` (⌘2)
  - :459 `Verify` (⌘3)
  - :460 `Queue` (⌘4)
- lib.rs:429: `Edit` (submenu)
- lib.rs:443: `Window` (submenu)

The predefined items are created with `None` text, so muda's **hard-coded English** defaults apply
(muda-0.20.0 `predefined.rs:413-450`):

- `about` (:370): `About …`
- `services` (:374): `Services`
- `hide` (:376): `Hide …`
- `hide_others` (:377): `Hide Others`
- `show_all` (:378): `Show All`
- `undo`/`redo` (:432-433): `Undo`/`Redo`
- `cut`/`copy`/`paste`/`select_all` (:435-438): `Cut`/`Copy`/`Paste`/`Select All`
- `minimize` (:446): `Minimize`
- `maximize` (:447): `Zoom`
- `close_window` (:449): `Close Window`

To translate them, pass `Some(text)`. Items macOS adds itself (Edit → Start Dictation, Emoji &
Symbols; Window extras) and the About panel follow the bundle's declared localizations
(Info.plist `CFBundleLocalizations` / `*.lproj`). That also applies to NSOpenPanel's own buttons
and the Finder "Kind" string.

### Menu bar icon (tray), set natively

- menubar.rs:423 (`title(status)`, menubar.rs:99-117) and menubar.rs:552 (`tray.set_title`):
  - `✓` / `✗`
  - `Paused`
  - `Removing`
  - `{p}%` / `…`
  - `{n}/{m} · {p}`
- There is no tooltip; the tray has no menu.

### Menu bar panel window title

menubar.rs:343: `.title("Secopy")`. The window is undecorated, so the title isn't visible.

### Source picker (NSOpenPanel), picker.rs

- picker.rs:27-29: message `Choose a directory, or one or more files`
- picker.rs:30: prompt (button) `Choose`

### tauri.conf.json (crates/secopy-app/tauri.conf.json)

- window `title`: `Secopy`
- fileAssociations `name`: `Secopy settings`
- fileAssociations `description`: `Secopy settings and presets`

These are config, not Rust. Localizing them means InfoPlist.strings.

### Power assertion

secopy-core awake.rs:7: `Secopy is copying files`.

### Not in Rust

- **Notifications** are built in the UI: summaryText.ts `notificationFor` / `queueNotification`
  with `t("notify.*")`, sent by api.ts:186-190. Rust only registers the plugin (lib.rs:114). The
  body embeds `headline()`, which interpolates `stoppedBecause`.
- **Dialogs** (save/open panels for export, reports and directories) are opened by the UI through
  the dialog plugin, with UI-side titles.
- The quit confirmation is UI-side.

---

## 5. `queue.json`: saved fields holding user-facing text

**`Entry.last_error`** (queue.rs:41-45): `Option<String>`, serialized as the JSON key `lastError`.

- Custom `Serialize` (queue.rs:144-166): it always writes the key, as a string or `null`
  (queue.rs:162).
- Custom `Deserialize` (queue.rs:168-195): it reads `value.get("lastError").as_str()`. Anything
  that isn't a string becomes `None`, silently.
- `QueuedJob::Unknown(serde_json::Value)` keeps a newer version's job as-is, but its `lastError`
  is overwritten on save (queue.rs:161-163).

**What gets saved there:**

- `Failed`: the whole `QueueResultView.reason` (commands.rs:669-671; after a panic :588). That is
  every text in §2 QueueResultView: pickProblem, blockers, E9, E17-E19, E22-E25, E58-E62, the
  guard, `failure_reason()`, `full_stop(stoppedBecause)`, `Needs a newer Secopy.` and
  `QUEUE_STOPPED`.
- `Cancelled`: `Stopped.` (commands.rs:672, :589).
- Not run: `None`, cleared (commands.rs:649-652).

**Migration note:** existing `queue.json` files hold English sentences. The new format should
accept both a plain string (legacy, shown as is) and a `{key, args}` object. `Entry` already goes
through `serde_json::Value`, so reading both is easy. Keep `version: 1` compatibility; the store
wrapper checks `version` (store.rs:18, :641-644).

**Other saved files that hold generated words (data now, but English-made):**

- `profiles.json` preset names:
  - store.rs:295 `Unnamed preset`, given to a nameless preset on load and saved (commands.rs:88-92)
  - store.rs:370 the `{name} ({n})` suffix
- `mirrors.json` / `profiles.json`: import's Keep both names use `{name} ({n})`
  (transfer.rs:349, :373)
- `settings.json` and `state.json` hold no text (state.json holds only paths).
- Reports (`reports/*.txt|json`) are English by design.

`QueueRun.done: Vec<(QueueResult, Option<String>)>` (commands.rs:73) holds the same reasons in
memory only.

---

## 6. UI code that compares or parses Rust's English text

| File:line | Code | Matches Rust text | Risk |
|---|---|---|---|
| ui/src/components/CopyPresetEditor.svelte:115 | `message.startsWith("The source")` → `sourceProblem` | E34 (store.rs:598) | Needs a code, e.g. `field: "source"`. |
| ui/src/components/CopyPresetEditor.svelte:116 | `/name\|preset called/.test(message)` → `nameProblem` | E31, E32, E33 | Also matches any `{io}` containing "name" (e.g. `File name too long (os error 63)` inside E2), which then shows as a name problem. |
| ui/src/components/MirrorEditor.svelte:104 | `message.startsWith("The origin")` → `originProblem` | E38, E40, E42 | The field is chosen by English wording. |
| ui/src/components/MirrorEditor.svelte:105 | `message.startsWith("The destination")` → `destinationProblem` | E39, E41 | |
| ui/src/components/MirrorEditor.svelte:106 | `/name\|mirror called/.test(message)` → `nameProblem` | E37, E32, E36 | Same false-positive risk as :116. |
| ui/src/components/MirrorScreen.svelte:126 | `message === "Cancelled."` → no error shown | E62 (mirror.rs:159) | This is the only exact-equality check. |

Doc comments only, no logic:

- ui/src/lib/api.ts:116 (`fails with "Cancelled."`)
- App.svelte:160
- MirrorScreen.svelte:40
- CopyPresetsScreen.svelte:34
- QueueSummary.svelte:35

No regexes or comparisons on `lastError`, `reason`, `blocker`, `guard`, `pickProblem` or
`fatal`: the UI only displays those, sometimes inside a catalog wrapper.

The catalog wrappers around Rust text (ui/src/locales/en.json) are:

- `progress.stopped`: `Stopped: {why}` (en.json:28, :220)
- `summary.checksumError` (:126), `summary.durabilityError` (:127), `summary.reportError` (:128)
- `summary.headline.stopped`

Once Rust sends codes, `{why}` becomes a translated nested message plus a raw OS text.

The same logic exists on the Rust side, over its own English:

- jobs.rs:289: `ready.label.strip_prefix("Mirror · ")` (the label is set at mirrors.rs:56)
- menubar.rs:165: `r.heading.starts_with("Checking")` (set at menubar.rs:221)
- commands.rs:1843, :1850: `match what { "The copy presets" | "The mirror presets" }`
- commands.rs:845-851: `full_stop` punctuation check
- commands.rs:1863: byte-slice capitalisation
- dto.rs:344: `sentence()` capitalisation

---

## Tests that will need updating (for scale)

Rust app tests with English sentence literals:

| File | Lines |
|---|---|
| commands.rs | ~19 |
| store.rs | ~13 |
| transfer.rs | ~12 |
| jobs.rs | ~9 |
| menubar.rs | ~9 |
| session.rs | ~4 |
| queue.rs | ~1 |
| lib.rs | ~1 |

The tests that use constants (`IMPORT_WAITS`, `IMPORT_CHANGED`, `NOT_SECOPY`, `NOTHING`,
`QUEUE_STOPPED`, `INTERNAL_ERROR`) will follow the constants.

UI tests with Rust-English fixtures:

- CopyPresetsScreen.test.ts
- ImportScreen.test.ts
- JobProgress.test.ts
- MirrorScreen.test.ts
- QueueScreen.test.ts
- QueueSummary.test.ts

The gallery fixture ui/src/gallery/fake.ts also uses them (lines 88, 145-146, 191, 205, 209).
