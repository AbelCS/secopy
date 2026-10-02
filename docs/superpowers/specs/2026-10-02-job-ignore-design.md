# Each copy and preset's own ignore list — design

Issue #164. Approved in conversation on 2026-10-02. Builds on #158 (Settings › Always ignore
when copying) and #161.

## Goal

Ignore some names in some jobs only. Example: `.gitkeep` is noise in most backups, but a
Template directory's `.gitkeep` files are its content. Today the only place to say "ignore" is
the global list, so the Template backup loses them.

Success: a preset or a New copy run ignores its own patterns on top of the global list, and
only there; nothing about other jobs changes; a mirror never removes a file its own list
ignores.

## Decisions (from the conversation)

| # | Decision |
|---|---|
| D1 | Every copy preset, mirror preset, New copy run and queued copy job has its own list, **Also ignore**, added to the global list. It can't switch a global pattern off. |
| D2 | A job's list means what the global list means: never copied or mirrored; a mirror never removes, archives or compares those files in its destination; ASC MHL histories ignore them. |
| D3 | New copy's list comes from the selected preset (like its file types), is editable for the run, and marks the preset as changed. |
| D4 | A queued job keeps the list it was queued with; at its turn it uses the global list as it is then, plus its own. |

## Patterns

The same patterns as the global list (#158): one name, `*` and `?`, case and Unicode form
ignored, no `/`, no control characters, up to 255 characters. A job's list and the global list
together hold at most 200 patterns: adding one past that is refused ("The global list and this
one can have up to 200 patterns."). A pattern already in the global list isn't added
("It's already in Settings › Always ignore when copying.").

## What the user sees

- **Shared editor** (`IgnoreList.svelte`, from #161's Settings list): the field and Add, one
  pattern per row, each with its remove button, the count. Settings uses it with Restore
  defaults; jobs use it without, shorter (about 4 rows before it scrolls).
- **New copy:** under the source, a collapsed row **Also ignore** with how many ("Also ignore
  (2)", or "Also ignore" when empty) that opens the editor. Changing the list scans the source
  again. The "12 ignored" count includes both lists; its Hint lists the global patterns and
  this copy's.
- **Copy preset editor and mirror preset editor:** an Also ignore field with the editor.
- **Preset changed:** New copy's "changed" mark (and Save to preset) covers the list.

## What it does

- **Copy:** the scan uses the global list plus the run's (or the queued job's).
- **Mirror:** the plan uses the global list plus the preset's: on the origin (not mirrored)
  and on the destination (never removed, archived or compared). A preview made before either
  list changed doesn't run (as #158).
- **ASC MHL:** the combined list goes into the history's ignore list.
- **Verify:** the global list only (Verify has no presets).
- **CLI:** unchanged (`--ignore` is per run already).

## Saved data, export and import

- `CopyPresetInput`, `CopyPreset`, `MirrorPresetInput`, `MirrorPreset`: `ignore: Vec<String>`,
  saved as `"ignore"`; missing (saved before) → empty. Read through the lenient check: a bad
  pattern is dropped.
- `CopyJob` (queue.json): `ignore`, missing → empty.
- Export writes each preset's list. #149's key check makes an older Secopy refuse such a preset
  ("Needs a newer Secopy"), so it's never imported without its list. Importing into this
  Secopy: a preset file without the key → empty list.
- Settings › Export/Import unchanged otherwise.

## How it's built

- **Engine:** `Patterns::with(&self, extra: &[String]) -> Result<Patterns, PatternError>` (the
  global plus a job's, checked, deduplicated, limit enforced); `Patterns::with_lenient` for
  saved lists (drops what doesn't fit).
- **App:** the fields above; `Session` keeps the run's list (`Change::JobIgnore(Vec<String>)`
  scans again), loads it from a preset, compares it for "changed", passes it when saving a
  preset and when queueing; `queue::prepare` and the mirror preview combine it with the
  settings' list; `MirrorJob` records the combined list for the stale-preview check.
- **UI:** `IgnoreList.svelte` extracted from `SettingsScreen.svelte`; New copy's collapsed row;
  the two preset editors; `ui/src/lib/patterns.ts` checks the combined limit and global repeats.

## Tests

- `Patterns::with`: combining, repeats with the global list, the 200 limit.
- Copy preset: its list saved, loaded into New copy, changed mark, Save to preset.
- New copy: changing the list scans again; the count includes both lists.
- Queue: a queued job keeps its list and combines it at its turn.
- Mirror preset: its list ignores files in the origin and protects them in the destination
  (Archive and Delete); a preview made before the preset's list changed doesn't run.
- ASC MHL: a job's patterns in the history's ignore list.
- Import/export: presets with lists round trip; a file without the key gives an empty list;
  an older reader refuses (unknown key).
- UI: the editor in New copy and in both preset editors; Settings still has Restore defaults.

## Not now

Switching a global pattern off for one job; a list for Verify; per-run lists for mirrors (a
mirror runs its preset).
