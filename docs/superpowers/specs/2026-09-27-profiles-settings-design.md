# Source Profiles and Settings (M2, plan 3b-1) — Design

Plan 3b-1 of the [v1 roadmap](../plans/2026-09-26-v1-roadmap.md), on top of the 0.2.0 app
([3a design](2026-09-27-macos-app-core-design.md)). Requirements live in
[RFD 0001](../../rfd/0001-secopy.md) (UX in §5, FR-36, FR-38); decisions made here are also
logged in RFD §14. Issue: #16. The rest of the original 3b (notifications, Eject, keyboard
shortcuts, the accessibility pass) is plan 3b-2, #20.

## Goal

Offloading a card becomes: pick the drive, pick the profile, pick the destination, Start.
Nothing to set up again per card, and nothing written to the card. The app remembers what
it should (mode, window size, last profile, recently used destinations) and never fills in
the destination by itself. Target release: 0.3.0.

**Success looks like:** the second FX3 card of the day takes three clicks and no typing;
a card that doesn't match the profile says so instead of copying the wrong thing; a
restart loses nothing the user saved.

**Out of scope (3b-2, #20):** notifications and their setting, Eject, keyboard shortcuts,
the accessibility pass, the 0.5 s wait at job end.
**Out of scope (plan 4, performance):** the Advanced settings (files in flight, buffer
size); tuning needs benchmarks first.

## Decisions

| # | Decision |
|---|---|
| B1 | **Split 3b:** 3b-1 is profiles and settings (0.3.0), 3b-2 is notifications, Eject, shortcuts and accessibility (0.4.0). Performance moves to 0.5.0, packaging to 0.6.0, betas to 0.7.x. |
| B2 | **The Rust side owns everything saved** (settings, profiles, state), in a `store` module; profiles are applied in `Session`. The UI sends commands and shows views, as in 3a. |
| B3 | **A profile's folder is relative to what was picked:** drive, drop or Choose…. Missing folder: a clear message, nothing is scanned. |
| B4 | **Profiles apply to folder sources only.** Picking files disables the profile menu for that run. |
| B5 | **No "Default mode" setting.** FR-36 remembers the last mode, which makes a default redundant (RFD §5.5 changes). |
| B6 | **The last selected profile is remembered** across launches (FR-36 changes). The destination still never is. |
| B7 | **Recent destinations are the last 5 folders a job actually started with**, offered in a menu; never filled in. |
| B8 | **Drives are listed by polling** `/Volumes` every 2 s while the setup screen shows, instead of listening to mount events. |
| B9 | **App identifier:** `com.latecommits.secopy` (was `com.belisoft.secopy`, RFD §14 changes). Reports saved by 0.2.0 move to the new data folder on first launch. |

## Main window changes (RFD §5.2)

```
FROM
  Drives   [● CARD_A  64 GB]  [○ SSD_T7  1 TB]
  Profile  [ Sony FX3 ▾ ]   (None · Sony FX3 · DJI Mini · Manage profiles…)
  ┌────────────────────────────────────────────────────────────┐
  │ /Volumes/CARD_A/PRIVATE/M4ROOT/CLIP            [ Choose… ] │
  │ 212 files · 180.4 GB                                       │
  └────────────────────────────────────────────────────────────┘
  [✓] Include the "CLIP" folder     File types [✓ .mp4] [ .xml]
  Sony FX3 · changed for this run   [Update profile] [Save as new…]

TO
  /Volumes/RAID/Day01                       [ Choose… ] [ Recent ▾ ]
```

- **Drives:** every volume under `/Volumes` except the Mac's own disk and the drive that
  holds the chosen destination, with its name and size. Clicking one picks its root as the
  source, like a drop. Updated every 2 s while the setup screen shows; a drive that goes
  away while it is the source keeps the current view (pre-flight and the job already
  handle a missing source).
- **Profile menu:** None, the profiles by name, and Manage profiles… (opens the Profiles
  screen).
  Selecting a profile rescans with its folder, include-folder choice and file types.
  Disabled when the source is a set of files (B4).
- **Picked + folder:** with a profile selected, the source is the picked path joined with
  the profile's folder; a pick that already ends with that folder (Choose… on
  `CARD_A/PRIVATE/M4ROOT/CLIP`) is used as is. If the folder doesn't exist: "CARD_A has no
  PRIVATE/M4ROOT/CLIP" in FROM, no source, Start disabled. Other pick problems (a folder and
  files together, a scan that fails) are shown the same way.
- **Changed for this run** shows when the include-folder choice, or the selection among the
  file types present on this card, differs from the profile. It offers:
  - **Update profile:** saves the current choices into the profile. File types the profile
    lists that aren't on this card are kept.
  - **Save as new…:** asks for a name (and shows the folder, prefilled with the picked
    path relative to the drive root) and selects the new profile.
  Without either, the profile file is untouched.
- **Recent ▾:** the last 5 destinations used, most recent first, those that no longer
  exist left out. Choosing one sets the destination as Choose… would. Empty list: the
  button is hidden.

## Settings screen (RFD §5.5)

A **Settings** button (gear icon) in the header and **Secopy → Settings… (⌘,)** show it in
the main window; **‹ Back** returns to where the user was. As in macOS settings, a change
applies and is saved at once, with a short "Saved" note; there is no Apply or Cancel. Each
option has a one-line explanation. Not available during a job: the button is hidden and
the menu item does nothing.

| Setting | Default | Effect |
|---|---|---|
| Write the checksum file to the destination | On | Off: the job writes none; the summary says "No checksum file (off in Settings)". |
| Show the count of skipped system files | On | Off: the count isn't shown; system files are still skipped (hidden files are copied, #25). |
| Also save the job report next to the checksum file | Off | On: the report is also written next to the checksum file (`Report::write_next_to`). Disabled while the checksum file is off. |

## Profiles screen

Opened by **Manage profiles…** in the Profile menu (Settings has only the three settings, so
each entry point leads to one place). The profile list on the left, the selected profile's
editor on the right: name; folder on the card, typed or filled by **Choose…** (a folder on
a card, given relative to the card); "Include the “CLIP” folder"; file types as **All
types** or **Only these** with removable chips and "+ add type". **Save** is active only
when something changed; problems show next to their field; **Delete…** asks first;
**+ New profile** starts an empty one. **‹ Back** returns to the main window; leaving a
profile with unsaved changes (Back, or another profile) asks "Discard changes?". With no profiles, a short explanation of what a
profile is. Not available during a job.

Settings apply to the next job; changing one never affects a running job.

## Profile rules

- **Folder:** relative (`PRIVATE/M4ROOT/CLIP`); no leading `/`, no `..`, `/` as
  separator; empty = the picked folder itself (a profile that only filters file types).
- **Name:** not empty after trimming; unique, case-insensitive.
- **File types:** a list of lowercase extensions without the dot (the scan's keys; "no
  extension" is its own entry), or *all*. *All* also includes types it has never seen.
- **Include folder:** the same meaning as the checkbox (FR-4); the folder included is the
  source folder, i.e. the last part of picked + profile folder (`CLIP`).
- **Id:** a random id, so renaming a profile keeps it selected.

## Remembered state (FR-36)

- **Mode** (Copy / Copy & Verify): saved when changed, restored at launch. Default Copy &
  Verify.
- **Window size:** kept in memory as the window is resized and written when the window
  closes or the app quits; restored at launch, clamped to the minimum size and the screen.
- **Last profile:** saved when selected; restored at launch if it still exists.
- **Recent destinations:** a destination is added when a job starts with it; last 5,
  most recent first, no duplicates.

## Architecture

`secopy-app` gains two modules; the rest grows in place.

| Unit | Purpose | Depends on |
|---|---|---|
| `store` | Loads and saves `settings.json`, `profiles.json`, `state.json` in the app's data folder. Writes to a temp file and renames it. Validates profiles. | serde_json |
| `volumes` | Lists the drives under a given volumes folder (`/Volumes` in the app, a temp folder in tests): name, path, total and free bytes; leaves out the boot volume. | statfs |
| `session` | + the selected profile and the picked path, profile application, "changed for this run", the missing-folder message. | `store` types |
| `jobs` | + the settings a job uses (checksum file, report next to it). | `store` types |
| `commands` | + `list_drives`, `select_profile`, `update_profile`, `save_profile_as`, `profiles`, `edit_profile`, `delete_profile`, `settings`, `set_settings`, `recent_destinations`, `app_state`, `set_mode`. | — |
| UI | Drives row, profile menu and actions, Recent ▾, the Settings screen. | generated bindings |

The window size is tracked from Rust (`WindowEvent::Resized`), written at quit, and applied
before the window shows (the window starts hidden), so it doesn't jump.

### Files

In `~/Library/Application Support/com.latecommits.secopy/`, each with `"version": 1`:

```json
// settings.json
{ "version": 1, "writeChecksumFile": true, "showSystemCount": true, "reportNextToChecksum": false }
// profiles.json
{ "version": 1, "profiles": [
  { "id": "k3f9…", "name": "Sony FX3", "folder": "PRIVATE/M4ROOT/CLIP",
    "includeFolder": true, "extensions": ["mp4"] } ] }
// state.json
{ "version": 1, "verify": true, "window": { "width": 900, "height": 780 },
  "lastProfile": "k3f9…", "recentDestinations": ["/Volumes/RAID/Day01"] }
```

A missing file means defaults. Unknown fields are ignored; a newer `version` is treated as
damaged (below) rather than half-read.

## Errors

- **Saving fails** (disk full, permissions): the message appears where the user acted
  ("Couldn't save the profile: …"); the change still applies for this run.
- **A damaged or unreadable file:** renamed to `<name>.damaged-<YYYYMMDD-HHMMSS>`, defaults
  used, and one message on the setup screen says which file and where it went. The app
  always starts.
- **Moving the 0.2.0 reports** (from `com.belisoft.secopy/reports`, only when the new
  folder has none): on failure they stay where they are and the error is logged; startup
  continues.
- **A profile's folder missing on the picked drive:** the FROM message above; not an
  error dialog.
- **Invalid profile input** (empty or duplicate name, bad folder): shown next to the field;
  nothing saved.

## Testing

- **Rust (`store`):** round trip of each file; missing file → defaults; damaged file →
  renamed aside and defaults; newer version → treated as damaged; save is atomic (no temp
  file left, old content kept when the rename fails); profile validation.
- **Rust (`volumes`):** a temp folder standing in for `/Volumes` with folders and a
  symlink to `/`; the symlink is left out; sizes come from the file system.
- **Rust (`session`):** a profile's folder is joined to the picked path; missing folder;
  profile file types become the filter; "changed for this run"; Update keeps file types not
  on this card; files source ignores the profile.
- **Rust (`jobs`):** checksum file off → none written and the summary says so; report next
  to the checksum file.
- **Rust (migration):** 0.2.0 reports move once; an existing new folder is left alone.
- **UI (Vitest, fake API):** drives row picks a drive; the profile menu applies and
  disables for files; changed-for-this-run and both actions; Save as new validation; Recent
  ▾; Settings toggles and the disabled report option; profile edit and delete; the damaged
  file message.
- **Manual checklist:** real drives appearing and going away; a profile on a real card;
  relaunch remembers mode, window size and profile, but not the destination; the
  identifier move keeps the old reports.

## Review focus

1. **A profile applied to the wrong card.** The picked path + folder must exist or nothing
   is scanned; the message names both.
2. **"Changed for this run" and Update profile** on a card that lacks some of the profile's
   file types: Update must not drop them.
3. **A damaged settings file** must never stop the app from starting or silently lose the
   other files.
4. **Settings during a job:** a running job keeps the settings it started with.
5. **The destination is never restored or filled in**, including from Recent ▾ at launch.

## RFD and roadmap changes (with this spec)

- FR-36: the last selected profile is also remembered (B6).
- §5.5: no "Default mode" row (B5); "Notify when a job finishes" moves to 3b-2; Advanced
  moves to plan 4.
- §14 decision log: B1, B5, B6, B9.
- Roadmap: 3b-1 (0.3.0, #16), 3b-2 (0.4.0, #20), performance 0.5.0, packaging 0.6.0, betas
  0.7.x.
