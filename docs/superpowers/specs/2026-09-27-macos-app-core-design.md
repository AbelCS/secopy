# macOS App Core (M2, plan 3a) — Design

Plan 3a of the [v1 roadmap](../plans/2026-09-26-v1-roadmap.md). Requirements live in
[RFD 0001](../../rfd/0001-secopy.md) (UX in §5); decisions made here are also logged in RFD
§14. Issue: #15. Plan 3b (#16) adds the comfort features after 3a has been used on real
shoots.

## Goal

A working Secopy app for Apple Silicon Macs: pick or drop a source and a destination, see
what will happen, copy and verify, follow the progress, and read an honest summary. It is
built with Tauri 2 and a Svelte + TypeScript UI on top of the finished `secopy-core`
engine, and each release carries an unsigned `.dmg`. Target release: 0.2.0.

**Who it is for:** the person offloading camera cards (video and photos) after a shoot
day. The first priority is a perfect copy; the second, an interface that needs no thought
at the end of a long day.

**Out of scope (plan 3b):** source profiles, settings page, recent destinations,
notifications, Eject, keyboard shortcuts, remembered state, the full accessibility pass.
**Out of scope (later):** code signing and notarization (needs an Apple Developer ID),
performance tuning (plan 4).

## Decisions

| # | Decision |
|---|---|
| A1 | **Split plan 3:** 3a is a working app, 3b adds profiles, settings and polish once 3a has been used. |
| A2 | **Architecture:** the Rust side keeps the scan, plan and job; the UI sends small commands and receives summaries. The full file list never crosses into the web view. |
| A3 | **Progress rate: 2 updates a second** (was 10–20, RFD NFR-5 and §5.3). Bars animate for 0.5 s between updates; speeds and ETAs are averaged over the last 3 s. |
| A4 | **Destination is chosen every run.** It is never filled in automatically or restored when the app starts (RFD FR-36 changes). Within a session, "New copy" keeps it, since the next card usually goes to the same project. |
| A5 | **Warn when the copy root already exists and isn't empty** (the "Files will go to" folder, not the folder picked). A warning, not a block. |
| A6 | **Nothing is ever written to a source card.** Card identity for 3b's profiles is chosen by hand. |
| A7 | **TypeScript types are generated from the Rust types** (`tauri-specta`), so the two sides can't drift. |
| A8 | **App identifier:** `com.belisoft.secopy`. |
| A9 | **One version:** the app takes its version from the workspace `Cargo.toml`; `ui/package.json` is private and carries no version. If Tauri needs a version in `tauri.conf.json`, that file joins release-please's `extra-files`. |

## Layout

```
crates/secopy-core     engine (unchanged API)
crates/secopy-cli      developer CLI
crates/secopy-app      Tauri 2 shell: state, commands, events (Rust)
ui/                    Svelte + TypeScript front end (Vite)
```

`secopy-app` is split so the logic is testable without the Tauri runtime:

| Module | Purpose |
|---|---|
| `session` | Scan, selection, pre-flight and plan for the current form; recomputes only what a change affects |
| `jobs` | Runs one job on a thread; keeps `JobControl`, the latest progress and every `FileOutcome`; builds the report |
| `dto` | The serializable types the UI sees (strings for paths, lossy for names that aren't UTF-8) |
| `commands` | Thin `#[tauri::command]` wrappers around `session` and `jobs` |
| `main.rs` / `lib.rs` | Tauri builder, plugins (dialog, opener), `tauri-specta` export |

## Commands

All return summaries; none returns the full file list.

| Command | Returns |
|---|---|
| `scan_source(paths, mode)` | Files, bytes, extension stats (sorted by bytes), hidden and symlink counts, scan problems. A newer call replaces an older one; a stale result is dropped. |
| `set_filter(filter)` | Selected files and bytes, and the copy-root path ("Files will go to"). |
| `check_destination(dest)` | Blocker (if any), free space, file-system kind, the first 100 per-file problems plus the total, identical and different counts, leftover partial files, and the non-empty warning (file count in the copy root). |
| `resolve(policy)` | Bytes to write and the free-space blocker, if any. |
| `start_job(verify, channel)` | Starts the job; progress arrives on `channel`. |
| `pause()`, `resume()`, `cancel()` | — |
| `finished_page(offset, limit, failed_only)` | Rows for the finished list. |
| `summary()` | The status line, stats, failures and the paths of the checksum file and the saved report. |
| `save_report(path)` | Writes the text report (and JSON next to it) to `path`. |
| `retry_failed()` | Prepares a new session with only the failed files (`Selection::subset`) and runs pre-flight again. |

Pickers use `tauri-plugin-dialog` from the UI; Reveal in Finder and Open checksum file use
`tauri-plugin-opener`. Finder drag and drop uses the webview's drag-drop event, whose
position tells FROM from TO.

## Progress events

The engine emits `Progress` every 500 ms (`JobOptions::progress_interval`). The app forwards
one message per tick on the job's channel:

- phase, elapsed, paused
- total files and bytes to write; copied and verified bytes
- files done, skipped, failed
- active files: files of 8 MiB or more get a row (name, path, phase, size, bytes done);
  smaller ones are summed into one "+ N small files" row

`FileFinished` events are not forwarded one by one: `jobs` stores the outcomes, and the UI
fetches pages as the finished list scrolls. Speeds and ETAs are computed in the UI from
successive messages (3 s window); "—" until two messages exist.

When the job ends, `jobs` saves the report (text and JSON, `Report`) to
`~/Library/Application Support/com.belisoft.secopy/reports/` and the UI switches to the
summary.

## Main window (RFD §5.2)

- **FROM:** Choose folder…, Choose files…, or drop from Finder. Shows the path, file count,
  size and skipped hidden items. For a folder: Copy the folder itself / Only its contents,
  and the extension chips (largest first, All / None).
- **TO:** Choose… or drop. Shows the path and free space, then "Files will go to: …". If
  that folder exists and isn't empty, a yellow warning with its file count (A5).
- **Pre-flight**, recomputed on every change:
  - blockers: a red message, Start disabled
  - files that will fail: "3 files will fail", expandable with names and reasons
  - identical files: "284 identical files will be skipped (not checked)"
  - different files: "12 files differ from what's there" with Keep both (default) /
    Overwrite / Skip
  - leftover partial files: "2 unfinished files from an interrupted copy will be replaced"
- **MODE:** Copy / Copy & Verify, Copy & Verify by default.
- **Start:** "Copy & verify 1,000 files · 208 GB"; disabled while scanning or checking, or
  when a blocker applies.

The checksum file is always written in 3a (a setting in 3b).

## Progress view (RFD §5.3)

- Phase and elapsed time; Copied bar, and Verified bar in Copy & Verify, each with bytes,
  percent, current and average speed, ETA. Files done, skipped, failed.
- Active files, as described above.
- Finished files: a virtualized list (only visible rows exist) with name, size, time,
  speed, xxHash64 and status (✓ Verified / ✓ Copied / Skipped / ✗ Failed + reason); a
  Failed only switch.
- Pause / Resume. Cancel asks first: "Files already copied stay and are listed in the
  checksum file; the file in progress is removed." Closing the window during a job asks
  the same.
- A fatal error shows a red banner ("Stopped: the source is no longer available") and
  moves to the summary.

## Summary (RFD §5.4)

- Status line: "All 1,000 files copied and verified" / "3 files failed" / "Cancelled" /
  "Stopped: …".
- Stats: files, size, time, average speed, skipped ("284 already at the destination, not
  checked").
- Failures with their reasons.
- Actions: Reveal in Finder, Open checksum file, Save report…, Retry failed, New copy
  (clears the source, keeps the destination for this session).

## Look

The dark design tokens of RFD §5.6 as CSS custom properties; system font with tabular
numerals for every figure; hashes in monospace. Controls use native HTML elements with
labels from the start; the full accessibility pass (NFR-10) is 3b.

## Errors

- Engine errors reach the UI as their `Display` text (they are written for people).
- A failed command shows an inline message where it happened (e.g. under TO for a
  destination that can't be read); nothing fails silently.
- If the app is quit during a job, the job is cancelled first, so no partial files are left
  behind.

## Build and release

- Tauri CLI and Vite through `ui/package.json` (npm). `cargo tauri` isn't needed.
- `ci.yml` (macOS, after merges) adds `npm ci`, the UI tests and `svelte-check`.
- `release-build.yml` also runs `tauri build --target aarch64-apple-darwin --bundles dmg` and
  attaches `Secopy_<version>_aarch64.dmg` (unsigned) next to the CLI archive.
- The README explains the first launch of an unsigned app: right-click → Open.

## Testing

- **Rust (`secopy-app`):** `session` recompute rules, `jobs` (start, pause, resume, cancel,
  paging, failed-only, report saved, retry failed) and `dto` conversion, on real temporary
  folders with `cargo test`. No Tauri runtime needed.
- **UI:** Vitest + Testing Library with the generated command bindings replaced by fakes:
  formatting of sizes, speeds and times; speed/ETA smoothing; the Start enable rules;
  pre-flight messages; the conflict choice; the Cancel confirmation. `svelte-check` for
  types.
- **End to end:** Tauri's WebDriver doesn't support macOS, so the plan ends with a manual
  checklist on a real SD card: a normal offload, a non-empty destination, pulling the card
  mid-copy, Retry failed, Cancel, and opening the checksum file with `xxhsum -c`.
