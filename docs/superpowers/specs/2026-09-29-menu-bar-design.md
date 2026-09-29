# Keep copying in the menu bar — design

Issue #80. Approved in conversation on 2026-09-29.

## Goal

Close Secopy's window while a copy, mirror, Verify or queue runs, and the job carries on
without being in the way: a menu bar icon shows how it's going and brings the window back.
Nothing about the job changes; only what happens to the window around it.

Success: close the window at 10 % of a long copy; the Dock icon goes, a menu bar icon shows
`10%`, `11%`…; its menu says the job, files, speed and time left, and can pause it or open the
window; at the end the icon says ✓ or ✗ and the notification comes as today. Secopy never runs
with no window and no icon.

## Decisions (from the conversation)

| # | Decision |
|---|---|
| D1 | The icon exists only while something runs, or until a job that finished while hidden has been seen. Closing the window with nothing running quits, as today. |
| D2 | While the window is hidden, Secopy leaves the Dock and ⌘Tab (accessory app); it comes back when the window shows. |
| D3 | The icon opens a small panel of Secopy's own under it (a progress bar, readable text, Pause, Open Secopy, Quit). A native menu was tried first and dropped after testing: disabled lines are grey, and replacing the menu closed it on every update. The title next to the icon carries the percentage. |
| D4 | A setting, **on by default**: "Keep copying in the menu bar when the window is closed". Off: today's behaviour (closing asks "Stop copying and quit?"). It is exported and imported like the other settings. |
| D5 | ⌘Q and the menu's Quit while a job runs show the window and ask, as today. Hiding is never stopping. |

## Behaviour

### Closing the window

| Setting | Something running? | Closing the window does |
|---|---|---|
| on | yes | hides it, shows the icon, leaves the Dock (**only if the icon was created**; otherwise as "off") |
| on or off | no | quits |
| off | yes | asks "Stop … and quit?" as today |

"Something running" is `AppState::busy()` (a job, or the queue between jobs). The close request
is decided in Rust (`on_window_event` `CloseRequested`) for the hide case, before the window's
own question: a hidden window never asks.

### The icon

- A monochrome template image (macOS tints it for light and dark menu bars), with a title next
  to it:
  - running: `42%`; a queue: `2/3 · 42%`; paused: `Paused`; a check: the same percentages;
    a mirror's removals: `Removing`; before the first update: `…`;
  - finished: `✓` (complete) or `✗` (anything else).
- Clicking the icon opens the panel under it (a window of Secopy's own, 340 × 190, made hidden
  at start); clicking elsewhere closes it:
  - running: the job ("Copying & verifying", "Job 2 of 3 · …", "Checking job 2 of 3"), the
    percentage, From and To paths (cut at their start), a progress bar, "44 of 106 files ·
    850.0 MB/s · 3:12 left" (or "Paused", "Removing files…"), **Pause/Resume**, **Open Secopy**,
    and **Quit Secopy…** as a link;
  - finished: ✓ or ✗ with how it ended — "Finished: every file done", "Finished with
    problems", "Cancelled", "Stopped: <why>" — then **Open Secopy** and **Quit Secopy**.
- Updated from Rust, from the job's own progress, at most once a second, so it doesn't depend on
  the hidden window's web view.

### Getting the window back

**Open Secopy**, or opening Secopy again from Finder, Spotlight or the Dock folder
(`RunEvent::Reopen`), shows the window, focuses it and brings back the Dock icon; the icon goes.
The window shows what it would have shown: the progress screen, or the summary of the job that
finished (the UI already moves to the summary when a job ends; it keeps working while hidden).

### Quitting

⌘Q, or Quit Secopy… in the icon's menu, while something runs: show the window, then the
existing "Stop … and quit?" question. When the job has finished (the ✓/✗ state) or nothing runs:
quit at once. Logout and system shutdown quit as today (the job is stopped cleanly).

### The setting

In Settings › Every copy, after "Notify when a copy finishes": ☑ **Keep copying in the menu bar
when the window is closed** — help: "Closing the window during a job hides it; a menu bar icon
shows the progress and brings it back. When off, closing asks to stop the job." Default on.
`settings.json` gains `keepInMenuBar` (missing = on); `.secopy` files carry it (a file without it
means on); the Import screen lists it as a change like the other switches.

## App

- `crates/secopy-app/Cargo.toml`: tauri features `tray-icon` and `image-png`.
- `crates/secopy-app/src/menubar.rs` (new):
  - pure: `close_action(keep: bool, busy: bool, icon: bool) -> CloseAction { Hide, Ask, Quit }`;
    `title(&Status) -> String`; `lines(&Status) -> Vec<String>`; `Status` built from a
    `ProgressView` (+ the queue's place and the job's label) or from how a job ended;
  - `MenuBar` (state in `AppState`): creates the tray icon lazily on first hide, updates title
    and menu (throttled to 1 s, menu rebuilt only when its lines change), removes it when the
    window shows; handles its menu events (pause/resume, open, quit).
- `lib.rs`: `on_window_event` `CloseRequested` → `close_action`; hide = `prevent_close`,
  `window.hide()`, `set_activation_policy(Accessory)`, create/show the icon; `RunEvent::Reopen`
  and **Open Secopy** → show, focus, `Regular`, remove the icon; the progress sinks also feed
  the menu bar; a job's end sets the finished state if hidden.
- `store.rs`: `Settings.keep_in_menu_bar` (default true); `transfer::settings_changes` names it.
- UI: the Settings checkbox; the close question is only asked when Rust didn't hide the window
  (it already asks only when a job runs; with the setting on and the icon made, the close event
  never reaches it).
- An icon asset: `crates/secopy-app/icons/menubar.png` (+ `@2x`), a monochrome template glyph.

## Testing

- Pure: every row of the close table, including "icon couldn't be made → ask, never hide";
  titles and lines for copy, Copy & Verify, queue, paused, removing, a check, before the first
  update, each way a job ends; the throttle (no rebuild when the lines are the same).
- Settings: `keepInMenuBar` defaults to on when missing from `settings.json` and from a
  `.secopy` file; export/import carry it; the Import screen names it.
- UI: the Settings checkbox saves it.
- Manual (checklist): close during a copy → icon, no Dock icon, percentages move; Pause from
  the menu pauses; Open Secopy shows the progress; close again and let it finish → ✓, the
  notification, Open Secopy shows the summary, the icon goes; ⌘Q while hidden and running asks
  with the window shown; open Secopy from Spotlight while hidden; light and dark menu bars; the
  setting off → today's question; a queue's `2/3 · …`.

## Review focus

1. Secopy is never left running with no window and no icon (icon creation fails, a job ends
   while hidden, Reopen races with the job's end).
2. Hiding never stops, pauses or changes a job; quitting while hidden still asks.
3. The icon never claims success for a job that failed, was cancelled or stopped.
4. The menu bar's updates can't slow the copy (throttled; no work per progress event beyond a
   timestamp check).
5. Logout/shutdown while hidden still stops the job cleanly.

## Out of scope

An always-present menu bar icon;
starting jobs from the menu bar; a Dock progress badge.
