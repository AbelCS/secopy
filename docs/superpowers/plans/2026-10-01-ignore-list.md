# Always Ignore When Copying Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Settings › Always ignore when copying: a user list of name patterns, defaulting to today's system files, that copies, mirrors, Verify, ASC MHL and the CLI all honour.

**Architecture:** A new `secopy_core::ignore` module replaces `system::is_system_file`: `Patterns` (checked list, `defaults()`, `matches(name)`) plus `is_own_file(name)` for Secopy's own files, always skipped. Every place that called `is_system_file` takes a `&Patterns` instead (scan via `ScanOptions`, mirror via `MirrorOptions`, Verify via `check::plan`, ASC MHL via `MhlInputs`). The app keeps the list in `Settings`, hands it to every scan, and edits it on the Settings screen.

**Tech Stack:** Rust (secopy-core, secopy-app, secopy-cli), Svelte 5 UI.

**Spec:** `docs/superpowers/specs/2026-10-01-ignore-list-design.md` (issue #158)

## Global Constraints

- A pattern is one name: `*` any run of characters, `?` one character, nothing else special; letter case ignored; a matching directory is skipped with everything in it.
- Refused: a pattern with `/` ("A pattern is a name: it can't contain /."), empty after trimming (dropped), a repeat in any case (dropped); at most 200 patterns of up to 255 characters.
- Defaults, in this order: `.DS_Store`, `._*`, `.Spotlight-V100`, `.fseventsd`, `.Trashes`, `.Trash`, `.TemporaryItems`, `.DocumentRevisions-V100`, `.VolumeIcon.icns`, `.apdisk`, `.localized`, `Icon\r`, `System Volume Information`, `$RECYCLE.BIN`, `Thumbs.db`, `desktop.ini`.
- Secopy's own files are always skipped, outside the list: `.name.secopy-partial`, `.secopy-<hash>.partial`, `.secopy-checksums.xxh64`, `.secopy-checksums.xxh64.damaged-*`; the mirror archive folder and NAS bookkeeping keep their own checks.
- What the user picks directly is never checked against the list.
- A mirror never deletes, archives or compares an ignored file in its destination.
- Patterns read from a file (settings, import) that fail the checks are dropped, never an error.
- i18n: every UI text in `ui/src/locales/en.json`; regenerate bindings with `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app`.
- Commits: Conventional Commits, author and committer `3268106+AbelCS@users.noreply.github.com` exported in the same shell; gate every commit on `bash /tmp/i18n/checks.sh` exiting 0.

## Review Focus

1. Removing a default (`.Spotlight-V100`) and running a mirror or Verify on a drive root: those folders are often unreadable; the job must report them as unread problems, not crash or call the copy failed silently. Pinned in Task 3 (`a_removed_default_is_mirrored_like_any_file`).
2. A pattern that matches the picked directory's own name (`DCIM` ignored, `DCIM` picked): the pick is still copied whole. Pinned in Task 2 (`what_was_picked_is_never_ignored`).
3. A settings file with a bad pattern (hand-edited, or from an import): the other settings and patterns load. Pinned in Task 6 (`a_bad_pattern_in_the_file_is_dropped`).
4. Changing the list while New copy shows a source: the shown counts and plan follow the new list. Pinned in Task 6 (`changing_the_list_scans_again`).
5. An ignored file already in a mirror's destination, in Delete mode: never deleted. Pinned in Task 3 (`an_ignored_file_in_the_destination_survives`).

---

### Task 1: The `ignore` module

**Files:**
- Create: `crates/secopy-core/src/ignore.rs`
- Modify: `crates/secopy-core/src/lib.rs`, `crates/secopy-core/src/system.rs` (keep `is_secopy_partial`; `NAMES` moves to `ignore::DEFAULTS`)

**Interfaces:**
- Produces:

```rust
pub const MAX_PATTERNS: usize = 200;
pub const MAX_LEN: usize = 255;
pub const DEFAULTS: &[&str] = &[".DS_Store", "._*", ".Spotlight-V100", ".fseventsd", ".Trashes", ".Trash",
    ".TemporaryItems", ".DocumentRevisions-V100", ".VolumeIcon.icns", ".apdisk", ".localized", "Icon\r",
    "System Volume Information", "$RECYCLE.BIN", "Thumbs.db", "desktop.ini"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternError { HasSlash, TooLong, TooMany }

/// A checked list: trimmed, no empty ones, no repeats (in any case).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patterns(Vec<String>);

impl Patterns {
    pub fn defaults() -> Patterns;
    pub fn none() -> Patterns;
    /// For what a person typed or a CLI flag: the first bad pattern is an error.
    pub fn new<I: IntoIterator<Item = String>>(list: I) -> Result<Patterns, PatternError>;
    /// For what a file says: bad patterns are dropped.
    pub fn lenient<I: IntoIterator<Item = String>>(list: I) -> Patterns;
    pub fn matches(&self, name: &std::ffi::OsStr) -> bool;
    pub fn as_slice(&self) -> &[String];
}
impl Default for Patterns { fn default() -> Self { Patterns::defaults() } }

/// Secopy's own working files: always skipped, whatever the list says.
pub fn is_own_file(name: &std::ffi::OsStr) -> bool;
```

- [ ] **Step 1: Failing tests** (`ignore.rs` test module):

```rust
#[test]
fn star_and_question_mark_and_any_case() {
    let p = Patterns::new(["*.LRF".to_string(), "C000?.XML".to_string()]).unwrap();
    for name in ["A001.LRF", "a001.lrf", "C0001.xml"] {
        assert!(p.matches(OsStr::new(name)), "{name}");
    }
    for name in ["A001.MP4", "LRF", "C00012.XML"] {
        assert!(!p.matches(OsStr::new(name)), "{name}");
    }
}

#[test]
fn only_star_and_question_mark_are_special() {
    let p = Patterns::new(["$RECYCLE.BIN".to_string(), "[x]".to_string()]).unwrap();
    assert!(p.matches(OsStr::new("$recycle.bin")));
    assert!(p.matches(OsStr::new("[x]")));
    assert!(!p.matches(OsStr::new("x")));
}

#[test]
fn the_defaults_are_todays_system_files() {
    let d = Patterns::defaults();
    for name in [".DS_Store", "._A001.MOV", ".Spotlight-V100", "Thumbs.db", "Icon\r", "DESKTOP.INI"] {
        assert!(d.matches(OsStr::new(name)), "{name:?}");
    }
    for name in ["A001.MOV", "DCIM", ".hidden_clip.mov"] {
        assert!(!d.matches(OsStr::new(name)), "{name}");
    }
}

#[test]
fn patterns_are_checked() {
    assert_eq!(Patterns::new(["a/b".to_string()]), Err(PatternError::HasSlash));
    assert_eq!(Patterns::new(["x".repeat(256)]), Err(PatternError::TooLong));
    assert_eq!(Patterns::new((0..201).map(|i| i.to_string())), Err(PatternError::TooMany));
    let p = Patterns::new([" .gitkeep ".to_string(), "".to_string(), ".GITKEEP".to_string()]).unwrap();
    assert_eq!(p.as_slice(), [".gitkeep"]);
}

#[test]
fn a_file_says_what_it_says_bad_patterns_are_dropped() {
    let p = Patterns::lenient(["a/b".to_string(), "*.LRF".to_string()]);
    assert_eq!(p.as_slice(), ["*.LRF"]);
}

#[test]
fn secopys_own_files_are_its_own() {
    for name in [".A001.MOV.secopy-partial", ".secopy-0123.partial", ".secopy-checksums.xxh64",
                 ".secopy-checksums.xxh64.damaged-2026"] {
        assert!(is_own_file(OsStr::new(name)), "{name}");
    }
    assert!(!is_own_file(OsStr::new(".DS_Store")));
}
```

- [ ] **Step 2: Run** `cargo test -p secopy-core --lib ignore` → FAIL to compile.

- [ ] **Step 3: Implement.** `matches`: compare lowercased name (`to_string_lossy().to_lowercase()`) with each lowercased pattern by a small recursive glob over `char`s (`*` any run, `?` one char). `new`: trim, drop empty, `contains('/')` → `HasSlash`, `chars().count() > MAX_LEN` → `TooLong`, dedup by lowercase, more than `MAX_PATTERNS` → `TooMany`. `lenient`: same, skipping bad ones and stopping at `MAX_PATTERNS`. `is_own_file`: `system::is_secopy_partial(name) || name == ".secopy-checksums.xxh64" || name.starts_with(".secopy-checksums.xxh64.damaged-")`.

- [ ] **Step 4: Run** → PASS (6 tests). **Step 5: Commit** `feat(core): ignore patterns (#158)`.

---

### Task 2: Scans use the list

**Files:** `crates/secopy-core/src/scan.rs`, callers (`mirror.rs` plan, `secopy-app/src/session.rs::scan_source`, `secopy-cli/src/main.rs`), `crates/secopy-core/src/system.rs` (remove `is_system_file` once no caller is left), `crates/secopy-core/tests/scan.rs`

**Interfaces:**
- `ScanOptions { pub ignore: Patterns }` (replaces `include_system_files`; `Default` = defaults).
- `Scan::skipped_system` renamed `Scan::ignored` (count of entries skipped by the list or as Secopy's own).

- [ ] **Step 1: Failing tests** in `tests/scan.rs`:
  - `an_ignored_file_and_directory_arent_scanned` — CARD/{a.MP4, a.LRF, THMBNL/t.jpg}, patterns `*.lrf`, `THMBNL` → files == [a.MP4], ignored == 2.
  - `what_was_picked_is_never_ignored` — pick `DCIM` with pattern `DCIM` → its files are scanned; files picked one by one that match are scanned.
  - `secopys_own_files_are_skipped_with_no_list` — `Patterns::none()` and a `.x.secopy-partial` file → not scanned; `.DS_Store` scanned.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** (`filter_entry`: depth 0 passes; else `is_own_file(name) || opts.ignore.matches(name)` → count, skip). Update callers: session `scan_source` keeps `ScanOptions::default()` until Task 6; CLI maps `--include-system-files` to `Patterns::none()`.
- [ ] **Step 4: Run** `cargo test --workspace` → PASS. **Step 5: Commit** `feat(core): scans skip what the ignore list names (#158)`.

---

### Task 3: Mirrors use the list

**Files:** `crates/secopy-core/src/mirror.rs`, `crates/secopy-core/tests/mirror.rs`, callers (`secopy-app/src/mirrors.rs`, CLI)

**Interfaces:** `MirrorOptions { pub deleted, pub deep_check, pub ignore: Patterns }`; `extras(destination, planned, origin_has, ignore: &Patterns)`.

- [ ] **Step 1: Failing tests** in `tests/mirror.rs`:
  - `an_ignored_file_in_the_origin_isnt_mirrored` — origin a.LRF, pattern `*.LRF` → not in the plan.
  - `an_ignored_file_in_the_destination_survives` — destination has b.LRF (not in origin), Delete mode and Archive mode → run → b.LRF still there, not archived, not listed as a removal.
  - `a_removed_default_is_mirrored_like_any_file` — `Patterns::none()`, origin has `.DS_Store` → mirrored; destination `.DS_Store` not in origin → listed for removal.
- [ ] **Step 2–4:** implement (scan with `ScanOptions { ignore: options.ignore.clone() }`; destination `filter_entry` uses `is_own_file || ignore.matches` instead of `is_system_file`); run → PASS. **Step 5: Commit** `feat(core): mirrors leave ignored files alone (#158)`.

---

### Task 4: Verify uses the list

**Files:** `crates/secopy-core/src/check.rs`, `crates/secopy-core/tests/check.rs`, callers (app `commands.rs::plan_check`, jobs tests, CLI)

**Interfaces:** `check::plan(dir: &Path, ignore: &Patterns) -> io::Result<CheckPlan>`.

- [ ] **Step 1: Failing tests:** `an_ignored_file_isnt_not_checked` (extra.LRF with `*.LRF` → not_checked empty), `a_listed_file_is_checked_whatever_the_list_says` (a.LRF listed in the checksum file and pattern `*.LRF` → in `files`).
- [ ] **Step 2–4:** replace `is_system_file` in both places with `is_own_file || ignore.matches` (directory filter and file filter); listed files come from checksum files, unaffected. **Step 5: Commit** `feat(core): Verify follows the ignore list (#158)`.

---

### Task 5: ASC MHL uses the list

**Files:** `crates/secopy-core/src/mhl/{ignore,prepare,run}.rs`, `crates/secopy-core/tests/mhl.rs`

**Interfaces:**
- `mhl::ignore::secopy_patterns(user: &Patterns) -> Vec<String>`: the standard's defaults, the user's patterns as gitignore names (`[` → `\[`), Secopy's own (`.secopy-checksums.xxh64`, `*.secopy-partial`, `.secopy-*.partial`, `._*` no longer forced), its checksum file and report (`secopy_*.xxh64`, `secopy_*.txt`, `secopy_*.json`).
- `mhl::ignore::merged(previous: &[String], user: &Patterns) -> Vec<String>`.
- `MhlInputs { copy_root, source_dir, ignore: Patterns }`; `MhlPlan { …, patterns: Vec<String> }` (Secopy's additions, used by `run`).

- [ ] **Step 1: Failing tests:** `the_ignore_list_is_in_the_manifest` (pattern `*.LRF` → manifest has `<pattern>*.LRF</pattern>`), `a_source_history_listing_an_ignored_file_blocks` (source history lists a.LRF, pattern `*.LRF` → `LeavesOut`).
- [ ] **Step 2–4:** thread the patterns through; `prepare` uses `inputs.ignore` for `leaves_out` (a recorded file the scan ignored isn't in `plan.files`, so it already blocks — confirm with the test) and `merged(previous, &inputs.ignore)` for files to read; `run` uses `plan.patterns`. **Step 5: Commit** `feat(core): ASC MHL histories ignore the list (#158)`.

---

### Task 6: The setting in the app

**Files:** `crates/secopy-app/src/{store,transfer,session,commands,mirrors,queue,jobs,dto}.rs`, `ui/src/locales/en.json`

**Interfaces:**
- `Settings::ignore: Vec<String>` (`"ignore"`; missing → `Patterns::defaults()`; read through `Patterns::lenient`).
- `AppState::set_settings` refuses a list `Patterns::new` rejects: `errors.settings.pattern` ("“{pattern}”: {why}") with `why` = `errors.pattern.slash` / `tooLong` / `tooMany`.
- `Session`: `ignore: Patterns`; `Change::Ignore(Patterns)` sets it and scans the current pick again; `PendingScan` carries the `Patterns`; `scan_source(source: &Source, ignore: &Patterns)`.
- `queue::prepare(job, settings: &Settings)` (uses `write_mhl` and `ignore`); `mirrors::prepare` takes `ignore: &Patterns`; Verify's `plan_check` uses the saved list; `MhlInputs.ignore` from the session.
- `SourceView::skipped_system` renamed `ignored`.
- `transfer`: `setting_label("ignore")` → `import.setting.ignore` ("Always ignore when copying"); a differing list adds `import.changed` ("{setting}: changed").

- [ ] **Step 1: Failing tests:**
  - store: `the_ignore_list_starts_as_the_defaults` (old file without `ignore` → defaults), `a_bad_pattern_in_the_file_is_dropped`.
  - transfer: `the_ignore_list_travels_and_a_change_is_named`, and #149's `settings_defaulted` includes `ignore` for an older file.
  - session: `changing_the_list_scans_again` (pick CARD with a.LRF; `Change::Ignore` with `*.LRF` → `selected_files` drops by one; `source.ignored` == 1).
  - commands: `a_bad_pattern_isnt_saved` (`set_settings` with `a/b` → error, saved settings unchanged); `saving_a_new_list_scans_again`.
  - queue: a queued job at its turn uses the saved list.
- [ ] **Step 2–4:** implement; on `set_settings` and on import of settings, when `ignore` changed, `state.rescan(Change::Ignore(..))`; regenerate bindings; run `cargo test -p secopy-app` → PASS. **Step 5: Commit** `feat(app): the Always ignore list, saved and used by every scan (#158)`.

---

### Task 7: Settings screen and counts in the UI

**Files:** `ui/src/components/SettingsScreen.svelte` (+ test), `ui/src/components/Setup.svelte` (+ test), `ui/src/locales/en.json`, fixtures

- [ ] **Step 1: Failing tests:**
  - SettingsScreen: "a pattern is added and removed, and Save sends the list"; "Restore defaults puts the defaults back"; "a pattern with / is refused with why"; "a repeat isn't added".
  - Setup: "ignored files are counted, with the patterns in the hint" (`ignored: 12` → "12 ignored").
- [ ] **Step 2–4:** the editor (list rows with remove buttons labelled "Remove {pattern}", a text field + Add, Return adds, Restore defaults; "Icon\r" shown as "Icon␍"); keys `settings.ignore.label` ("Always ignore when copying"), `settings.ignore.help`, `settings.ignore.add`, `settings.ignore.remove`, `settings.ignore.restore` ("Restore defaults"), `settings.ignore.slash`, `settings.ignore.tooLong`, `settings.ignore.tooMany`; `copy.ignored` one/other ("{count} ignored"), `copy.ignoredHint` ("Files and directories named in Settings › Always ignore when copying: {patterns}"); `settings.systemCount.label` → "Show the count of ignored files". The defaults list comes from the backend (`defaultIgnore()` command returning `Patterns::defaults()`), not duplicated in TypeScript. Run `bash /tmp/i18n/checks.sh` → rc 0. **Step 5: Commit** `feat(ui): Always ignore when copying in Settings (#158)`.

---

### Task 8: CLI

**Files:** `crates/secopy-cli/src/main.rs`, `crates/secopy-cli/tests/cli.rs`

- [ ] **Step 1: Failing tests:** `ignore_adds_patterns` (`--ignore '*.LRF'` → a.LRF not copied), `a_bad_pattern_is_an_error` (`--ignore a/b` → exit 2), `include_system_files_drops_the_defaults` (`.DS_Store` copied).
- [ ] **Step 2–4:** `#[arg(long, value_name = "PATTERN")] ignore: Vec<String>`; list = (`--include-system-files` ? none : defaults) + `--ignore`, via `Patterns::new`; used by the copy scan, mirror options, `--check`, ASC MHL. **Step 5: Commit** `feat(cli): --ignore (#158)`.

---

### Task 9: Docs

- RFD: FR-12/FR-14 (hidden and system files) rewritten for the list; decision log line; user guide: the setting, `--ignore`.
- Run checks, commit `docs: Always ignore when copying (#158)`.
