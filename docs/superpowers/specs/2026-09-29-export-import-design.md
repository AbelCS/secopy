# Export and import — design

Issue #77. Approved in conversation on 2026-09-29.

## Goal

Move Secopy's setup between Macs and people: a backup of your own settings and presets, a new
Mac, or a team sharing the same presets. One file type holds any mix of settings, copy presets
and mirror presets. Importing never changes anything until you've seen what's in the file and
pressed Import, and never overwrites a preset without being told to.

Success: export on one Mac, import on another, and the presets and settings are the same; a
colleague's preset arrives next to yours (or replaces it, if you chose that); a damaged file
changes nothing.

## Decisions (from the conversation)

| # | Decision |
|---|---|
| D1 | For both backups/new Macs and sharing with other people. |
| D2 | One `.secopy` file (JSON inside) for everything: settings, copy presets, mirror presets, in any mix. |
| D3 | A name clash is chosen per preset: **Keep both** (default; the new one is renamed "Sony FX3 (2)") or **Replace**. |
| D4 | A path that isn't on this Mac is imported as it is, with a note. Not an error: drives come and go, and a run already checks. |
| D5 | Never exported: the queue, recent destinations, the remembered window and last preset, reports. They belong to this Mac. |

## The file

```json
{
  "secopy": 1,
  "app": "0.11.0",
  "exported": "2026-09-29T10:12:00+02:00",
  "settings": { "writeChecksumFile": true, "showSystemCount": true,
                "reportNextToChecksum": false, "notifyWhenDone": true },
  "copyPresets": [ { "name": "Sony FX3", "source": "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP",
                     "includeFolder": true, "extensions": ["mp4"] } ],
  "mirrorPresets": [ { "name": "Footage", "origin": "/Volumes/SSD/Footage",
                       "destination": "/Volumes/Media/Footage",
                       "deleted": { "mode": "archive", "days": 30 }, "deepCheck": false } ]
}
```

- `secopy` is the format version. Each section is optional; a file with none is refused ("There
  is nothing in this file to import.").
- Presets carry no ids: ids belong to one Mac (a queued mirror points to its preset's id).
  Each preset's fields are the ones `profiles.json` and `mirrors.json` already use, in the same
  JSON form, so a preset means the same thing in both places.
- Reading is lenient where it's safe and strict where it matters:
  - unknown fields are ignored (a later Secopy may add some);
  - a `secopy` version newer than this app knows is refused, whole: "This file was made by a
    newer Secopy (format 2). Update Secopy to import it.";
  - a file that isn't JSON, or has no `secopy` field, is refused: "This isn't a Secopy file.";
  - files over 10 MB are refused without reading them (a real one is a few KB);
  - one preset that can't be read (a missing name, a wrong field type) is listed with why and
    left out; the others can still be imported. Imported presets are normalised and checked
    exactly like presets typed by hand (the same `check` as Save).
- Written as UTF-8 JSON, indented, to a temporary name next to the target, synced, then renamed
  into place, so a failed export never leaves a half-written `.secopy` under the final name.

## Export

- **File › Export…** and an **Export…** button in Settings open a small sheet:
  "Export: ☑ Settings ☑ Copy presets (3) ☑ Mirror presets (2)", **Cancel** / **Export…**. Empty
  sections are shown unticked and disabled ("Copy presets (none)"). Export… opens the save panel
  with "Secopy settings 2026-09-29.secopy" in the user's last export directory (Documents the
  first time).
- **One preset:** an **Export…** button on the Copy presets screen (for the selected preset) and
  on the Mirror screen (for the selected mirror preset) goes straight to the save panel with
  "Sony FX3.secopy". Unsaved edits are not exported: the button asks to save or discard first,
  like switching presets does.
- After saving, a short confirmation in the screen's message area: "Exported 3 copy presets,
  2 mirror presets and the settings." A failure says why ("The file couldn't be saved: the disk
  is full.").

## Import

- **Ways in:** **File › Import…**, an **Import…** button in Settings, and opening a `.secopy` file
  from Finder (double-click, or dragging it onto the Dock icon). The app declares the `.secopy`
  document type so Finder offers it. While a job or the queue runs, importing waits: File ›
  Import… is greyed out (like Settings), and a file opened from Finder shows "Import it when the
  copy has finished." with nothing changed. Export works any time.
- **The Import screen** (a screen like Settings, with Back): the file's name, then one section
  per kind, each item with a checkbox (ticked by default):
  - **Settings:** only what would change, "Checksum file: on → off", or "Same as yours" (then
    unticked and disabled).
  - **Copy presets / Mirror presets:** the name, its path(s), and:
    - a name clash: a two-way choice, **Keep both** (default; shows the name it will get, "Sony
      FX3 (2)") / **Replace**;
    - a path that isn't there now: a muted note, "/Volumes/RAID isn't connected now." (both of a
      mirror's paths are checked);
    - a preset that can't be imported: unticked, disabled, with the reason.
  - **Import** (primary) is off while nothing is ticked; **Back** leaves without changing
    anything.
- **Importing** applies what's ticked, then goes back to where you were with one line:
  "Imported 2 copy presets and the settings." Replace keeps the existing preset's id (and so
  a queued mirror still runs it); Keep both gives the new preset a new id and the first free
  name "Name (n)". Settings are applied as a whole (the four switches), like Save in Settings.
- Each kind is saved with the existing atomic save (`Store::save`). Copy presets, mirror presets
  and settings live in three files, so an import that fails partway says exactly what was saved
  and what wasn't: "The copy presets were imported; the mirror presets couldn't be saved: …".
  Nothing is ever half-written inside a file.

## App

- `crates/secopy-app/src/transfer.rs` (new): the file format (`SecopyFile`, versioned), `export(…)
  -> String` and `read(bytes) -> Result<Contents, String>` (with per-preset problems), and
  `plan_import(contents, &CopyPresets, &MirrorPresets, &Settings, queue) -> ImportView` (clashes,
  new names, missing paths, settings changes) and `apply_import(choices) -> ImportResult`. Pure
  where it can be, so it's tested without a window.
- Commands: `export_file(path, what)`, `export_copy_preset(id, path)`,
  `export_mirror_preset(id, path)`, `open_import(path) -> ImportView`,
  `apply_import(choices) -> String` (the line to show). The save/open panels use the existing
  picker, filtered to `.secopy`.
- `lib.rs`: File menu items Export… and Import…; `RunEvent::Opened { urls }` for files opened
  from Finder, also at launch, which sends the window to the Import screen. `tauri.conf.json`:
  a `fileAssociations` entry for `.secopy` ("Secopy settings", role Editor).
- UI: `ImportScreen.svelte`, an export sheet (`ExportDialog.svelte`, using `Dialog`), Export… on
  the Copy presets and Mirror screens, Export…/Import… in Settings, `App.svelte` routing.

## Testing

- **Format:** export then read gives back the same presets and settings; unknown fields are
  ignored; a newer version, a non-Secopy file, an empty file, a 10 MB+ file are refused; one bad
  preset is listed and the rest are importable; paths with Unicode and spaces round-trip.
- **Import plan:** clashes get "Name (2)" (and "(3)" when "(2)" is taken, case-insensitively);
  Replace keeps the id; a queued mirror still runs after its preset is replaced; importing is
  refused while a job or the queue runs; missing paths get their note; settings show only changes.
- **Safety:** a failed save of one kind leaves the others as they were and the result says so; a
  failed export leaves no file under the final name; nothing changes on Back.
- **UI:** the export sheet (counts, empty kinds disabled), the Import screen (choices, notes,
  Import off with nothing ticked), the result line; axe checks.
- **Manual (checklist):** export on this Mac, import on another user account; double-click a
  `.secopy` in Finder with Secopy closed and open.

## Review focus

1. Nothing changes before Import, and Back changes nothing.
2. Replace never breaks a queued mirror, and nothing is imported while a job or the queue runs.
3. A damaged or hostile file (huge, deeply nested, wrong types) can't crash the app or change
   anything.
4. Imported presets pass the same checks as hand-made ones.
5. A partial failure is reported exactly.

## Out of scope

Encrypting or signing files; syncing presets automatically between Macs; exporting the queue;
merging a preset field by field; importing from other apps' preset formats.
