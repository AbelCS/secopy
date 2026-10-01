# Always ignore when copying — design

Issue #158. Approved in conversation on 2026-10-01.

## Goal

Let the user keep files they never want out of every copy and mirror (`*.LRF` proxies,
`.gitkeep`), the way Secopy already keeps out `.DS_Store` and `Thumbs.db`: one list in
Settings, which starts as those system files and can be put back to them.

Success: a pattern added in Settings keeps matching files and directories out of the next copy
and mirror; a mirror never removes an ignored file from its backup; Restore defaults gives
today's behaviour back exactly.

## Decisions (from the conversation)

| # | Decision |
|---|---|
| D1 | Settings › **Always ignore when copying**: a list of patterns; its defaults are today's system files (`.DS_Store`, `Thumbs.db`, `._*`…); **Restore defaults** puts them back. |
| D2 | Copies **and mirrors**. A mirror never deletes or archives an ignored file in its destination. |
| D3 | A pattern matches a **name** (file or directory) anywhere: `*` and `?`, letter case ignored. No paths: a pattern with `/` is refused. |
| D4 | Secopy's own working files (unfinished copies, a mirror's checksum file and its set-aside copies) stay ignored, outside the list: they can't be removed by accident. |

## Patterns

- One name per pattern; `*` is any run of characters, `?` one character. Nothing else is
  special (`[` is a plain character), so names like `$RECYCLE.BIN` or `Icon\r` need no escaping.
- Matched against each file's and directory's name, without regard to letter case (as APFS
  and HFS+ compare names). A matching directory is skipped with everything in it.
- Settings trims each pattern; an empty one or a repeat (in any case) is dropped; one with
  `/` is refused ("A pattern is a name: it can't contain /."). At most 200 patterns of up to 255
  characters each.
- **Defaults**, in this order: `.DS_Store`, `._*`, `.Spotlight-V100`, `.fseventsd`, `.Trashes`,
  `.Trash`, `.TemporaryItems`, `.DocumentRevisions-V100`, `.VolumeIcon.icns`, `.apdisk`,
  `.localized`, `Icon\r` (shown as "Icon␍"), `System Volume Information`, `$RECYCLE.BIN`,
  `Thumbs.db`, `desktop.ini`. That is `system.rs`'s list today without
  `.secopy-checksums.xxh64`, which becomes one of Secopy's own (D4).

## What it does

- **Copy:** a file or directory whose name matches isn't copied. What the user picks directly
  (the directory or files chosen as the source) is never checked against the list.
- **Mirror:** the origin is scanned the same way, so ignored files aren't mirrored. The
  destination's listing leaves them out too, so they're never removed, archived or compared.
- **Verify:** ignored files aren't counted as "not checked" (as system files today). Files a
  checksum file lists are always checked, whatever the list says.
- **ASC MHL:** the patterns join the generation's ignore list, written as gitignore name
  patterns (a `[` escaped). A source history that lists a file the list ignores blocks Start
  ("leaves out files the source's ASC MHL history lists"), as for a file-type filter.
- **New copy:** "12 ignored" where "12 system files skipped" shows today, with the Hint listing
  the patterns. The setting "Show the count of skipped system files" becomes "Show the count of
  ignored files".
- **Queue:** a queued job uses the list as it is at its turn, like the other settings.
- **Secopy's own files** (D4): `.name.secopy-partial`, `.secopy-<hash>.partial`,
  `.secopy-checksums.xxh64` and its `.damaged-*` copies, the mirror archive folder, NAS
  bookkeeping (`@eaDir`, `#recycle`…): checked as today, before and apart from the list.

## Settings screen

Under Every copy, after the checksum file options:

- **Always ignore when copying**, with a Hint: "Files and directories with these names are
  never copied or mirrored, and a mirror never removes them from its backup. * stands for any
  characters, ? for one."
- The patterns as a list, each with a remove button (its label: "Remove *.LRF").
- A text field and **Add** (Return adds too); a refused pattern says why under the field.
- **Restore defaults**: puts the default list back (Save applies it, Cancel drops it, as every
  other change on the screen).

## Settings file, export and import

- `Settings::ignore: Vec<String>`, saved as `"ignore"`. A settings file without it (older
  Secopy) reads the defaults.
- Exported with the settings. The Import screen names a change as "Always ignore when copying:
  changed" (not each pattern); a file from an older Secopy without the key lists it as set to
  its default (#149).
- Patterns read from a file (settings or import) go through the same checks as typed ones: a
  bad one is dropped, never an error that loses the other settings.

## Command-line tool

- The defaults apply, as the fixed list does today.
- `--ignore PATTERN`, repeatable, adds patterns (checked as in Settings; a bad one is an error).
- `--include-system-files` drops the defaults (Secopy's own files stay ignored), as today.

## How it's built

- **secopy-core `ignore` module** (from `system.rs`): `Patterns` (the checked list, with
  `defaults()` and `matches(name) -> bool`) and `is_own_file(name)` for D4.
- `ScanOptions` gets `ignore: Patterns` in place of `include_system_files`; `MirrorOptions` and
  `check::plan` take it too. `skipped_system` becomes `ignored`.
- `mhl::ignore::secopy_patterns` takes the list (instead of `system::NAMES`).
- **App:** `Settings::ignore`, the Settings screen editor, the count's label, the scans of New
  copy, mirrors, queued jobs and Verify using the saved list.
- **CLI:** `--ignore`.

## Tests

- The matcher: case, `*` and `?`, plain `[`, a directory skipped with its contents, a picked
  source never ignored.
- Copy: an ignored file and directory aren't copied; Restore defaults copies what today's build
  copies.
- Mirror: an ignored file in the origin isn't mirrored; one in the destination survives a run
  in Archive and in Delete mode.
- Verify: an ignored file isn't "not checked"; a listed one is checked anyway.
- ASC MHL: the patterns are in the manifest's ignore list; a source history listing an ignored
  file blocks.
- Settings: an older file reads the defaults; empty, repeated and `/` patterns; Restore
  defaults; the screen adds and removes.
- Import and export of the list (#149's key handling included).
- CLI: `--ignore` and `--include-system-files`.

## Not now

Path patterns; per-preset or per-run lists; showing which files were ignored (only how many).
