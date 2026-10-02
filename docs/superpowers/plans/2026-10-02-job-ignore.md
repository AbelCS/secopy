# Each Copy and Preset's Own Ignore List Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Copy presets, mirror presets, New copy runs and queued copy jobs get an Also ignore list, added to Settings › Always ignore when copying.

**Architecture:** The engine already takes one `Patterns`; the app combines the global list with a job's (`Patterns::with`) wherever it builds scan, mirror and ASC MHL options. Presets and queued jobs store `ignore: Vec<String>` (missing → empty, read leniently). The UI gets one `IgnoreList.svelte` editor used by Settings, New copy (collapsed row) and both preset editors.

**Tech Stack:** Rust (secopy-core, secopy-app), Svelte 5 UI.

**Spec:** `docs/superpowers/specs/2026-10-02-job-ignore-design.md` (issue #164)

## Global Constraints

- Each list holds at most 128 patterns, the global list and a job's alike; a saved list longer than that keeps its first 128; adding past it is refused ("The list can have up to 128 patterns.").
- A pattern already in the global list isn't added to a job's ("It's already in Settings › Always ignore when copying.").
- A job's list means what the global one means: not copied or mirrored; a mirror never removes, archives or compares those files in its destination; ASC MHL ignores them.
- New copy's list comes from the preset, is editable per run, scans again, and marks the preset changed.
- A queued job keeps its list; at its turn: the global list then, plus its own.
- Verify uses the global list only; CLI unchanged.
- i18n: every UI text in `ui/src/locales/en.json`; regenerate bindings with `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app`.
- Commits: Conventional Commits, author and committer `3268106+AbelCS@users.noreply.github.com` exported in the same shell; gate every commit on `bash /tmp/i18n/checks.sh` exiting 0.

## Review Focus

1. A mirror preset whose own list protects `*.LRF`, Delete mode, a backup with `.LRF` files only there: never deleted. Pinned in Task 4 (`a_mirror_presets_list_protects_its_backup`).
2. Editing a mirror preset's list after its preview: the preview doesn't run. Pinned in Task 4 (`a_preview_made_before_the_presets_list_changed_doesnt_run`).
3. A preset file from 0.21 (no `ignore` key): loads with an empty list, nothing else changes. Pinned in Task 2 (`a_preset_saved_before_has_no_list`).
4. Selecting another preset in New copy: the run's list becomes that preset's, not the old one plus. Pinned in Task 3 (`another_preset_brings_its_own_list`).
5. A queued job whose list was set in New copy: its turn uses that list even after New copy changed. Pinned in Task 4 (`a_queued_job_keeps_its_list`).

---

### Task 1: Engine — 128 and combining

**Files:** `crates/secopy-core/src/ignore.rs`, `ui/src/lib/patterns.ts`

**Interfaces:** `pub const MAX_PATTERNS: usize = 128;` `impl Patterns { pub fn with(&self, job: &Patterns) -> Patterns }` (global's patterns first, then the job's not already there in any case or Unicode form).

- [ ] Failing tests: `a_list_holds_up_to_128` (129 typed → `TooMany`; 200 read leniently → 128 kept, the first ones), `a_jobs_list_adds_to_the_global_one` (`defaults().with(["*.LRF", ".ds_store"])` → defaults + `*.LRF`, matches `a.LRF`). Update the tests that used 200/201.
- [ ] Implement; `MAX_PATTERNS` = 128 in `patterns.ts` too. Run `cargo test -p secopy-core --lib ignore::`. Commit `feat(core): ignore lists hold up to 128, and a job's adds to the global one (#164)`.

### Task 2: Saved data — presets, queued jobs, import/export

**Files:** `crates/secopy-app/src/{store,queue,transfer}.rs`

**Interfaces:** `CopyPreset::ignore`, `CopyPresetInput::ignore`, `MirrorPreset::ignore`, `MirrorPresetInput::ignore`, `CopyJob::ignore` — all `Vec<String>`, serde `"ignore"`, `#[serde(default)]`, read through `Patterns::lenient` (on-disk structs) and checked with `Patterns::new` in `CopyPresets::normalized` / `MirrorPresets::normalized` (a typed bad pattern is `errors.field.ignore` with the pattern and why).

- [ ] Failing tests: `a_preset_saved_before_has_no_list` (copy and mirror preset JSON without `ignore` → empty), `a_presets_list_is_saved_and_checked` (round trip; `a/b` refused by `normalized`), `a_queued_job_without_a_list_has_none` (queue.json without `ignore`), transfer `presets_with_lists_travel` (export/import round trip; a file without the key → empty; a preset with an unknown key is still refused — #149).
- [ ] Implement (add the field to every literal: `ignore: Vec::new()`), regenerate bindings. Run `cargo test -p secopy-app --lib`. Commit `feat(app): presets and queued jobs keep their own ignore list (#164)`.

### Task 3: New copy

**Files:** `crates/secopy-app/src/{session,commands,dto,lib}.rs`

**Interfaces:** `Session`: `global_ignore: Patterns` (renamed from `ignore`), `job_ignore: Vec<String>`; `Change::JobIgnore(Vec<String>)` (scans again, keeps file types); selecting a preset sets `job_ignore = preset.ignore`; `preset_changed` compares it; `updated_preset`, `choices` and `copy_job` carry it; `PendingScan::ignore` and `MhlInputs::ignore` = `global.with(&lenient(job))`. Command `set_job_ignore(list) -> SessionView` (checks each with `Patterns::new` and the global repeat rule, `errors.field.ignore`). `SessionView::job_ignore: Vec<String>`.

- [ ] Failing tests (session): `the_runs_list_is_ignored_too` (`Change::JobIgnore(["*.xml"])` → one file fewer, `source.ignored` 1), `a_presets_list_comes_with_it` (preset with `["*.xml"]` → applied; `view.job_ignore`), `another_preset_brings_its_own_list`, `changing_the_runs_list_marks_the_preset_changed`, `save_to_preset_keeps_the_runs_list`; commands: `set_job_ignore_refuses_a_bad_or_global_pattern`.
- [ ] Implement; regenerate bindings. Run `cargo test -p secopy-app --lib`. Commit `feat(app): New copy's own ignore list (#164)`.

### Task 4: Queue and mirrors

**Files:** `crates/secopy-app/src/{queue,mirrors,commands}.rs`

**Interfaces:** `queue::prepare(job, settings)` scans with `settings.patterns().with(&lenient(job.ignore))`; `AppState::prepare_mirror` passes `settings.patterns().with(&lenient(preset.ignore))`; `MirrorJob::ignore` records the combined list; `run_mirror`'s stale check compares the combined list now.

- [ ] Failing tests: `a_queued_job_keeps_its_list` (queue.rs), `a_mirror_presets_list_protects_its_backup` (commands: preview + run, Delete mode, `.LRF` only in the backup survives), `a_preview_made_before_the_presets_list_changed_doesnt_run`.
- [ ] Implement. Run `cargo test -p secopy-app --lib`. Commit `feat(app): queued jobs and mirrors use their own ignore list (#164)`.

### Task 5: UI

**Files:** create `ui/src/lib/ui/IgnoreList.svelte` (+ test); modify `SettingsScreen.svelte`, `Setup.svelte`, `CopyPresetEditor.svelte`, `MirrorEditor.svelte`, `lib/api.ts`, `test/fake-api.ts`, `gallery/fake.ts`, `locales/en.json`.

**Interfaces:** `IgnoreList` props: `patterns: string[]`, `onChange(list)`, `global?: string[]` (for the repeat rule), `rows?: number` (8 Settings, 4 jobs), `restore?: () => Promise<string[]>` (Settings only), `label: string`. `api.setJobIgnore(list): Promise<SessionView>`.

- [ ] Failing tests: IgnoreList (adds, removes, refuses `/`, a repeat, a global repeat, the 128th+1); Settings still restores defaults; Setup "Also ignore (2)" opens the editor and calls `setJobIgnore`; the count's hint lists both lists; CopyPresetEditor and MirrorEditor save `ignore`.
- [ ] Implement (Setup: a `<details>` row under the source; keys `copy.alsoIgnore` "Also ignore", `copy.alsoIgnoreCount` "Also ignore ({count})", `presets.alsoIgnore`, `settings.ignore.inGlobal`). Run `bash /tmp/i18n/checks.sh`. Commit `feat(ui): Also ignore in New copy and the preset editors (#164)`.

### Task 6: Docs

- RFD FR-12 (job lists) and the decision log; user guide (New copy, presets, Settings: 128). Run checks; commit `docs: each copy and preset's own ignore list (#164)`.
