# Notifications, Eject, Shortcuts and Accessibility (M2, plan 3b-2) — Design

Plan 3b-2 of the [v1 roadmap](../plans/2026-09-26-v1-roadmap.md), on top of 0.3.0 (source
profiles, settings and the [design system](../../design/design-system.md)). Requirements live
in [RFD 0001](../../rfd/0001-secopy.md) (§5.4, §5.5, §7.4, FR-37, NFR-10); decisions made
here are also logged in RFD §14. Issue: #20. Target release: 0.4.0.

## Goal

Offload a card without touching the mouse, walk away, hear when it's done, and eject the
card from the summary. The app works fully from the keyboard and with VoiceOver, and the
summary appears the moment a copy ends.

**Success looks like:** ⌘O, pick the card, ⌘↩; switch to another app; a notification says
"All 212 files copied and verified"; back in Secopy, "Eject CARD_A" and pull the card.

## Decisions

| # | Decision |
|---|---|
| C1 | **Eject is a button on the summary, never automatic.** After a failed copy the card may still be needed for Retry failed. |
| C2 | **Eject uses macOS's own eject** (`diskutil eject` on the source's mount point): it unmounts every volume on the card and reports why when it can't. |
| C3 | **Shortcuts live in a File menu**, so they are visible and greyed out when they don't apply. |
| C4 | **A notification only when the window isn't in front**, with a setting (on by default). |
| C5 | **Accessibility is checked by tests** (axe-core on every screen, WCAG AA contrast for every token pair) plus a VoiceOver walk-through in the manual checklist. |

## 1. Notification when a copy ends

- When a job ends (the progress view's `done`) and the window doesn't have focus, the app
  shows a macOS notification: title = the summary's headline ("✓ All 212 files copied and
  verified", "✗ 3 files failed", "Stopped: …", "Cancelled"), body = the figures
  ("212 files · 180.4 GB written · took 4:12").
- Setting **"Notify when a copy finishes"**, on by default, on the Settings screen with a
  help line. Stored in `settings.json` as `notifyWhenDone` (missing = on).
- Uses `tauri-plugin-notification`. macOS asks for permission the first time; if it is
  denied, nothing else happens (the summary is there anyway).

## 2. Eject

- The summary's action bar shows **Eject <drive name>** when the job's source is on a
  drive macOS can eject (external or removable, per DiskArbitration: not internal). Never
  for the Mac's own disk.
- It runs `diskutil eject <mount point>` for the drive holding the source. On success the
  summary says "<drive> was ejected. You can remove it." and the button goes away. On
  failure (a program still uses the card) it shows diskutil's reason, and the card stays
  mounted.
- When the destination is on an ejectable drive, the summary also says **"Safe to eject
  <drive>: everything was written."** The job has already flushed the destination (RFD
  §7.4), so this is true when the summary shows. Not shown for a stopped job whose
  destination went away.
- `SummaryView` gains `source_drive: Option<DriveRef { name, mount_point }>` (ejectable
  source only) and `destination_drive` (ejectable destination only). A command
  `eject(mount_point)` runs the eject; it refuses anything that isn't one of those two.

## 3. Keyboard

A **File** menu between the app menu and Edit:

| Item | Shortcut | Enabled |
|---|---|---|
| Choose Source… | ⌘O | on New copy |
| Choose Destination… | ⌘D | on New copy |
| Start Copy | ⌘↩ | when Start is enabled |
| Cancel Copy | ⌘. | during a copy (still asks) |

- Menu items send events to the UI (like Settings… today); the UI tells the app which items
  apply (`set_menu_state`) whenever its screen or Start changes.
- **Space** pauses and resumes on the progress screen (not while a button or field has
  focus, where Space already means "press").
- **Esc** is Back on Settings and Profiles (Profiles still asks about unsaved changes), and
  closes Save as new…. Native dialogs already close with Esc.

## 4. Accessibility (NFR-10)

- **Automated:** every screen, rendered with fake data, passes axe-core with no violations;
  a test checks WCAG AA (4.5:1) for every text token on every surface token (`text-faint`
  is only for disabled controls and is exempt, as WCAG allows).
- **Focus:** when the screen changes, focus moves to its title (`h1`, focusable with
  `tabindex="-1"`), so VoiceOver reads where you are.
- **Announcements:** the action bar's status is a polite live region; the progress phase
  (Copying → Verifying → Done) and the end of a copy are announced there.
- **Keyboard:** everything is reachable with Tab; the file lists get `tabindex="0"` so the
  arrow keys scroll them; chips, segmented control and radios already work with the keyboard.
- **Manual:** a checklist item walks through a copy with VoiceOver.

## 5. The wait at the end of a job

The engine's progress ticker sleeps a whole interval (500 ms in the app) before it notices the
job ended. It waits on a channel instead (`recv_timeout`), which the job closes when the
workers finish, so the ticker ends at once. A test runs a tiny job with a 2 s interval and
checks that it finishes in well under a second.

## Testing

- **Rust:**
  - the ticker test;
  - `DriveRef` for an ejectable vs internal volume (the eject decision is a function of
    DiskArbitration's answers, tested with fixed answers);
  - `eject` refuses a mount point that isn't the job's source or destination drive;
  - the `notifyWhenDone` setting's default and round trip.
- **UI (Vitest, fake API):**
  - the notification is sent only when the window isn't focused and the setting is on, with
    the headline and figures;
  - the Eject button and its success and failure messages;
  - "Safe to eject";
  - menu events drive Choose / Start / Cancel;
  - `set_menu_state` follows the screen and Start;
  - Space and Esc;
  - focus moves to the title;
  - axe-core on every screen;
  - contrast of the tokens.
- **Manual checklist:**
  - the notification with the window in the background;
  - Eject with a real card, and with a file on it still open;
  - every shortcut;
  - a VoiceOver walk-through.

## Review focus

1. **Eject during or before a Retry:** after Eject, Retry failed must fail cleanly (the
   source is gone), not hang.
2. **Space while typing:** never pauses a copy when focus is in a field (e.g. the Save as
   new… name).
3. **Menu state out of date:** Start Copy must never start a copy the Start button wouldn't.
4. **Notification permission denied:** no error, no retry loop.
5. **Eject refused** (card in use): a clear message; the button stays usable.

## RFD and roadmap changes (with this spec)

- §5.4: the Eject button and "Safe to eject" replace the bare "Safe to eject" note of §7.4.
- §5.5: "Notify when a copy finishes" is a real setting (was "Plan 3b-2").
- FR-37: the File menu, ⌘. to cancel, Space to pause, Esc as Back.
- §14 decision log: C1 to C5.
- Roadmap: 3b-2 designed.
