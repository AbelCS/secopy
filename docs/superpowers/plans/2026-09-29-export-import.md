# Export and Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export settings, copy presets and mirror presets to a `.secopy` file, and import one back with a preview, per-preset Keep both / Replace, and nothing changed until Import.

**Architecture:** A new `crates/secopy-app/src/transfer.rs` holds the file format (write, read with limits and per-preset problems) and the pure import logic (plan a view, apply choices to copies of the preset lists). `commands.rs` adds five commands that save through the existing atomic `Store::save`, one kind at a time. `lib.rs` adds File › Import…/Export… and files opened from Finder. The UI adds an export dialog, an Import screen, and Export… on the preset screens.

**Tech Stack:** Rust (serde, serde_json, chrono), Tauri 2 + tauri-specta, Svelte 5 runes, Vitest + testing-library.

**Spec:** `docs/superpowers/specs/2026-09-29-export-import-design.md`

## Global Constraints

- macOS only; no cfg code for other OSes.
- UI words: "directory" never "folder"; "source"/"destination"; saved setups are "presets" ("copy preset", "mirror preset").
- Button labels: the fewest words that can't be read two ways (design system); `help` on buttons only where the label hides the detail.
- Nothing changes before Import is pressed; Back changes nothing.
- A name clash defaults to **Keep both** (first free "Name (n)", case-insensitive); **Replace** keeps the existing preset's id.
- Never exported: the queue, recent destinations, the remembered window and last preset, reports.
- Importing waits while a job or the queue runs: "Import it when the copy has finished."
- Files: format version `1` in the `secopy` field; over 10 MB refused unread; newer format refused whole; not JSON / no `secopy` field → "This isn't a Secopy file."; nothing to import → "There is nothing in this file to import."
- Exports are written to a temporary name next to the target, synced, then renamed.
- Regenerate bindings after DTO changes: `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app --lib ui_bindings`.
- Commits: Conventional Commits, `Refs #77`, the Co-Authored-By line; author and committer `3268106+AbelCS@users.noreply.github.com` (`export GIT_AUTHOR_EMAIL=… GIT_COMMITTER_EMAIL=…` in the same shell).
- Checks before each commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -q -- -D warnings`, `cargo test --workspace`, `cd ui && npx vitest run && npm run check`.

## Review Focus

1. A hostile file (deeply nested JSON, a 50 MB file, a preset with a 1 MB name or non-string fields) is refused or listed as unreadable, never crashes and never changes anything — Task 2 tests `nested_json_is_refused_not_a_crash`, `a_huge_file_is_refused_unread`, `wrong_field_types_are_one_unreadable_preset`.
2. Two presets in one file with the same name, and a file preset whose name clashes only by letter case, each get a distinct name — Task 3 test `names_in_the_file_dont_clash_with_each_other`.
3. Replace of a mirror preset that the queue refers to keeps the queue's job pointing to it — Task 3 test `replace_keeps_the_id_a_queued_mirror_uses`.
4. A save failure partway (the mirror presets can't be written) leaves the copy presets imported, the rest untouched, and says so — Task 4 test `a_failed_save_says_what_was_imported`.
5. A file opened from Finder while a job runs changes nothing — Task 4 test `importing_waits_while_a_job_runs`.

---

## File Structure

- Create `crates/secopy-app/src/transfer.rs` — the `.secopy` format and the pure import logic.
- Modify `crates/secopy-app/src/store.rs` — shared validation (`normalized`), `free_name` for both kinds, `Serialize` on the inputs, `input()` on presets.
- Modify `crates/secopy-app/src/commands.rs` — AppState fields and the export/import commands.
- Modify `crates/secopy-app/src/lib.rs` — module, commands list, File menu items, `RunEvent::Opened`.
- Modify `crates/secopy-app/tauri.conf.json` — the `.secopy` document type.
- Create `ui/src/components/ExportDialog.svelte` (+ test), `ui/src/components/ImportScreen.svelte` (+ test).
- Modify `ui/src/lib/api.ts`, `ui/src/test/fake-api.ts`, `ui/src/App.svelte` (+ `App.test.ts`), `ui/src/components/SettingsScreen.svelte`, `CopyPresetsScreen.svelte`, `MirrorScreen.svelte` (+ tests), `ui/src/gallery/Gallery.svelte`, `ui/src/gallery/fake.ts`, `ui/src/a11y.test.ts`.
- Docs: `README.md`, `docs/rfd/0001-secopy.md`, `docs/design/design-system.md`, `docs/testing/macos-app-checklist.md`, `docs/superpowers/plans/2026-09-26-v1-roadmap.md`.

---

### Task 1: Shared preset validation and names

**Files:**
- Modify: `crates/secopy-app/src/store.rs`
- Test: `crates/secopy-app/src/store.rs` (tests module)

**Interfaces:**
- Produces:
  - `impl CopyPresets { pub fn normalized(input: CopyPresetInput) -> Result<CopyPresetInput, String>; pub fn free_name(&self, name: &str) -> String; pub fn named(&self, name: &str) -> Option<&CopyPreset> }`
  - `impl MirrorPresets { pub fn normalized(input: MirrorPresetInput) -> Result<MirrorPresetInput, String>; pub fn free_name(&self, name: &str) -> String; pub fn named(&self, name: &str) -> Option<&MirrorPreset> }`
  - `impl CopyPreset { pub fn input(&self) -> CopyPresetInput }`, `impl MirrorPreset { pub fn input(&self) -> MirrorPresetInput }`
  - `CopyPresetInput` and `MirrorPresetInput` derive `Serialize` (camelCase, same field names as today's JSON).

- [ ] **Step 1: Write the failing tests** (append to the `tests` module in `store.rs`)

```rust
    #[test]
    fn normalized_checks_a_preset_without_its_name_clashing() {
        let input = CopyPresetInput {
            name: "  Sony FX3 ".into(),
            source: "/Volumes/CARD_A/CLIP/".into(),
            include_folder: true,
            extensions: Some(vec![Some(".MP4".into())]),
        };
        let n = CopyPresets::normalized(input).unwrap();
        assert_eq!((n.name.as_str(), n.source.as_str()), ("Sony FX3", "/Volumes/CARD_A/CLIP"));
        assert_eq!(n.extensions, Some(vec![Some("mp4".into())]));
        let bad = CopyPresetInput { name: " ".into(), ..n.clone() };
        assert_eq!(CopyPresets::normalized(bad).unwrap_err(), "The preset needs a name.");
        let mirror = MirrorPresetInput {
            name: "Footage".into(),
            origin: "/Volumes/SSD/Footage".into(),
            destination: "/Volumes/SSD/Footage/Backup".into(),
            deleted: DeletedFiles { mode: DeletedMode::Archive, days: 30 },
            deep_check: false,
        };
        assert_eq!(
            MirrorPresets::normalized(mirror).unwrap_err(),
            "The destination can't be inside the origin."
        );
    }

    #[test]
    fn free_names_skip_taken_ones_in_any_case() {
        let mut presets = CopyPresets::default();
        presets.add(input("Sony FX3", "")).unwrap();
        presets.add(input("sony fx3 (2)", "")).unwrap();
        assert_eq!(presets.free_name("Sony FX3"), "Sony FX3 (3)");
        assert_eq!(presets.free_name("DJI"), "DJI");
        assert_eq!(presets.named("SONY FX3").map(|p| p.name.as_str()), Some("Sony FX3"));
    }

    #[test]
    fn a_preset_gives_back_its_input() {
        let mut presets = CopyPresets::default();
        let p = presets.add(input("Sony FX3", "/Volumes/CARD_A")).unwrap();
        let back = p.input();
        assert_eq!((back.name, back.source), ("Sony FX3".to_string(), "/Volumes/CARD_A".to_string()));
        let json = serde_json::to_value(p.input()).unwrap();
        assert!(json.get("includeFolder").is_some(), "{json}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p secopy-app --lib store::tests 2>&1 | tail -5`
Expected: compile errors: no function `normalized`, `free_name` is private or missing on `CopyPresets`, no method `named`, no method `input`.

- [ ] **Step 3: Implement**

In `store.rs`:
1. Add `Serialize` to both input derives: `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]` on `CopyPresetInput` and `MirrorPresetInput` (they already have `#[serde(rename_all = "camelCase")]`).
2. Split each `check` into a static `normalized` (everything but the name clash) and the clash test:

```rust
impl CopyPresets {
    /// `input` checked and put right like a preset typed by hand, except for its name being
    /// taken: trimmed name, full-path source, file types in lowercase without dots.
    pub fn normalized(input: CopyPresetInput) -> Result<CopyPresetInput, String> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err("The preset needs a name.".into());
        }
        Ok(CopyPresetInput {
            name,
            source: source(&input.source)?,
            include_folder: input.include_folder,
            extensions: extensions(input.extensions),
        })
    }

    /// The preset called `name`, in any letter case.
    pub fn named(&self, name: &str) -> Option<&CopyPreset> {
        let name = name.to_lowercase();
        self.presets.iter().find(|p| p.name.to_lowercase() == name)
    }

    /// `name`, or else `name (2)`, `name (3)`…: the first no preset has, in any letter case.
    pub fn free_name(&self, name: &str) -> String {
        free_name(name, |n| self.named(n).is_some())
    }

    fn check(&self, input: CopyPresetInput, editing: Option<&str>) -> Result<CopyPresetInput, String> {
        let input = Self::normalized(input)?;
        if self.named(&input.name).is_some_and(|p| Some(p.id.as_str()) != editing) {
            return Err(format!("There is already a preset called “{}”.", input.name));
        }
        Ok(input)
    }
}
```

Replace the old private `free_name` method body (used by `repaired`) with a call to the new public one, and add the shared helper:

```rust
/// `name`, or else `name (2)`, `name (3)`…: the first `taken` says no to.
fn free_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_string();
    }
    (2..)
        .map(|i| format!("{name} ({i})"))
        .find(|n| !taken(n))
        .expect("a free name")
}
```

Mirror presets, the same way (keep every existing check and message, moved into `normalized`):

```rust
impl MirrorPresets {
    /// `input` checked and put right like a mirror typed by hand, except for its name being taken.
    pub fn normalized(input: MirrorPresetInput) -> Result<MirrorPresetInput, String> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err("The mirror needs a name.".into());
        }
        let origin = full_path(&input.origin, "origin", "/Volumes/SSD/Footage")?;
        let destination = full_path(&input.destination, "destination", "/Volumes/NAS/Footage")?;
        let (o, d) = (Path::new(&origin), Path::new(&destination));
        if o == d {
            return Err("The origin and the destination are the same directory.".into());
        }
        if d.starts_with(o) {
            return Err("The destination can't be inside the origin.".into());
        }
        if o.starts_with(d) {
            return Err("The origin can't be inside the destination.".into());
        }
        if input.deleted.mode == DeletedMode::Archive && input.deleted.days == 0 {
            return Err("Keep archived files for at least 1 day.".into());
        }
        Ok(MirrorPresetInput { name, origin, destination, ..input })
    }

    pub fn named(&self, name: &str) -> Option<&MirrorPreset> {
        let name = name.to_lowercase();
        self.presets.iter().find(|p| p.name.to_lowercase() == name)
    }

    pub fn free_name(&self, name: &str) -> String {
        free_name(name, |n| self.named(n).is_some())
    }

    fn check(&self, input: MirrorPresetInput, editing: Option<&str>) -> Result<MirrorPresetInput, String> {
        let input = Self::normalized(input)?;
        if self.named(&input.name).is_some_and(|p| Some(p.id.as_str()) != editing) {
            return Err(format!("There is already a mirror called “{}”.", input.name));
        }
        Ok(input)
    }
}

impl CopyPreset {
    /// The preset as a form would hold it: everything but its id.
    pub fn input(&self) -> CopyPresetInput {
        CopyPresetInput {
            name: self.name.clone(),
            source: self.source.clone(),
            include_folder: self.include_folder,
            extensions: self.extensions.clone(),
        }
    }
}

impl MirrorPreset {
    pub fn input(&self) -> MirrorPresetInput {
        MirrorPresetInput {
            name: self.name.clone(),
            origin: self.origin.clone(),
            destination: self.destination.clone(),
            deleted: self.deleted,
            deep_check: self.deep_check,
        }
    }
}
```

- [ ] **Step 4: Run to verify they pass, and the rest still do**

Run: `cargo test -q -p secopy-app 2>&1 | grep "test result"`
Expected: all pass (the existing preset tests prove the messages didn't change).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -q -- -D warnings
git add crates/secopy-app/src/store.rs
git commit -F - <<'EOF'
refactor(app): check a preset apart from its name being taken

Refs #77

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2: The `.secopy` file: write and read

**Files:**
- Create: `crates/secopy-app/src/transfer.rs`
- Modify: `crates/secopy-app/src/lib.rs` (`mod transfer;`)
- Test: `crates/secopy-app/src/transfer.rs` (tests module)

**Interfaces:**
- Consumes: Task 1 (`CopyPreset::input`, `MirrorPreset::input`, `Serialize` inputs); `store::Settings` (Serialize + its lenient Deserialize).
- Produces:
  - `pub const FORMAT: u32 = 1;`, `pub const MAX_BYTES: u64 = 10 << 20;`
  - `pub struct Contents { pub settings: Option<Result<Settings, String>>, pub copy_presets: Vec<Result<CopyPresetInput, Unreadable>>, pub mirror_presets: Vec<Result<MirrorPresetInput, Unreadable>> }` (Debug, Clone, PartialEq)
  - `pub struct Unreadable { pub name: String, pub why: String }` (Debug, Clone, PartialEq)
  - `pub fn export_text(settings: Option<&Settings>, copy: &[CopyPreset], mirrors: &[MirrorPreset], app: &str, now: chrono::DateTime<chrono::Local>) -> String`
  - `pub fn write_file(path: &Path, text: &str) -> Result<(), String>`
  - `pub fn read(bytes: &[u8]) -> Result<Contents, String>`
  - `pub fn read_file(path: &Path) -> Result<Contents, String>`
  - `pub const NOT_SECOPY: &str = "This isn't a Secopy file.";`, `pub const NOTHING: &str = "There is nothing in this file to import.";`

- [ ] **Step 1: Write the failing tests** (new file with only the tests module and `use` lines first, or the whole file with `todo!()` bodies is NOT allowed — write the tests, then the code in Step 3)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{CopyPresets, DeletedFiles, DeletedMode, MirrorPresetInput, MirrorPresets};

    fn copy_presets() -> CopyPresets {
        let mut p = CopyPresets::default();
        p.add(CopyPresetInput {
            name: "Sony FX3".into(),
            source: "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP".into(),
            include_folder: true,
            extensions: Some(vec![Some("mp4".into()), None]),
        })
        .unwrap();
        p.add(CopyPresetInput {
            name: "Día 1 — ñandú".into(),
            source: "/Volumes/My Card/DCIM".into(),
            include_folder: false,
            extensions: None,
        })
        .unwrap();
        p
    }

    fn mirror_presets() -> MirrorPresets {
        let mut m = MirrorPresets::default();
        m.add(MirrorPresetInput {
            name: "Footage".into(),
            origin: "/Volumes/SSD/Footage".into(),
            destination: "/Volumes/Media/Footage".into(),
            deleted: DeletedFiles { mode: DeletedMode::Archive, days: 30 },
            deep_check: true,
        })
        .unwrap();
        m
    }

    fn now() -> chrono::DateTime<chrono::Local> {
        chrono::Local::now()
    }

    #[test]
    fn what_is_exported_reads_back_the_same() {
        let settings = Settings { write_checksum_file: false, ..Settings::default() };
        let (c, m) = (copy_presets(), mirror_presets());
        let text = export_text(Some(&settings), &c.presets, &m.presets, "0.11.0", now());
        let read = read(text.as_bytes()).unwrap();
        assert_eq!(read.settings, Some(Ok(settings)));
        let copies: Vec<CopyPresetInput> = read.copy_presets.into_iter().map(Result::unwrap).collect();
        assert_eq!(copies, c.presets.iter().map(|p| p.input()).collect::<Vec<_>>());
        let mirrors: Vec<MirrorPresetInput> = read.mirror_presets.into_iter().map(Result::unwrap).collect();
        assert_eq!(mirrors, m.presets.iter().map(|p| p.input()).collect::<Vec<_>>());
        assert!(!text.contains("\"id\""), "ids belong to one Mac: {text}");
    }

    #[test]
    fn only_what_was_chosen_is_in_the_file() {
        let text = export_text(None, &copy_presets().presets[..1], &[], "0.11.0", now());
        let read = read(text.as_bytes()).unwrap();
        assert_eq!(read.settings, None);
        assert_eq!(read.copy_presets.len(), 1);
        assert!(read.mirror_presets.is_empty());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let text = r#"{"secopy":1,"later":true,"copyPresets":[{"name":"A","source":"","includeFolder":true,"extensions":null,"colour":"red"}]}"#;
        assert!(read(text.as_bytes()).unwrap().copy_presets[0].is_ok());
    }

    #[test]
    fn a_newer_file_is_refused_whole() {
        let text = r#"{"secopy":2,"copyPresets":[]}"#;
        assert_eq!(
            read(text.as_bytes()).unwrap_err(),
            "This file was made by a newer Secopy (format 2). Update Secopy to import it."
        );
    }

    #[test]
    fn other_files_are_not_secopy_files() {
        for text in ["", "hello", "[1,2]", r#"{"version":1}"#, r#"{"secopy":"one"}"#] {
            assert_eq!(read(text.as_bytes()).unwrap_err(), NOT_SECOPY, "{text:?}");
        }
        assert_eq!(read(br#"{"secopy":1}"#).unwrap_err(), NOTHING);
    }

    #[test]
    fn nested_json_is_refused_not_a_crash() {
        let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
        assert_eq!(read(deep.as_bytes()).unwrap_err(), NOT_SECOPY);
    }

    #[test]
    fn wrong_field_types_are_one_unreadable_preset() {
        let text = r#"{"secopy":1,"copyPresets":[
            {"name":"Good","source":"","includeFolder":true,"extensions":null},
            {"name":"Bad","source":42,"includeFolder":true,"extensions":null},
            {"source":""}
        ]}"#;
        let read = read(text.as_bytes()).unwrap();
        assert!(read.copy_presets[0].is_ok());
        let bad = read.copy_presets[1].clone().unwrap_err();
        assert_eq!(bad.name, "Bad");
        assert!(bad.why.starts_with("Its details can't be read"), "{}", bad.why);
        assert_eq!(read.copy_presets[2].clone().unwrap_err().name, "A copy preset with no name");
    }

    #[test]
    fn unreadable_settings_dont_block_the_presets() {
        let text = r#"{"secopy":1,"settings":"loud","copyPresets":[{"name":"A","source":"","includeFolder":true,"extensions":null}]}"#;
        let read = read(text.as_bytes()).unwrap();
        assert!(matches!(read.settings, Some(Err(_))));
        assert!(read.copy_presets[0].is_ok());
    }

    #[test]
    fn a_huge_file_is_refused_unread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.secopy");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_BYTES + 1).unwrap();
        assert_eq!(read_file(&path).unwrap_err(), "This file is too big to be a Secopy file.");
    }

    #[test]
    fn writing_leaves_no_half_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Sony FX3.secopy");
        write_file(&path, "{\"secopy\":1}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"secopy\":1}");
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 1, "no temporary file left: {names:?}");
        let missing = dir.path().join("no-such-dir").join("x.secopy");
        assert!(write_file(&missing, "{}").is_err());
        assert!(!missing.exists());
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Add `mod transfer;` to `crates/secopy-app/src/lib.rs` next to the other `mod` lines.
Run: `cargo test -q -p secopy-app --lib transfer 2>&1 | tail -5`
Expected: compile errors: `export_text`, `read`, `read_file`, `write_file`, `Contents`, … not found.

- [ ] **Step 3: Implement** (top of `transfer.rs`, above the tests)

```rust
//! The `.secopy` file (#77): settings, copy presets and mirror presets, in any mix, to move a
//! setup between Macs and people. Presets carry no ids: ids belong to one Mac.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::store::{CopyPreset, CopyPresetInput, MirrorPreset, MirrorPresetInput, Settings};

/// The format this Secopy writes and reads.
pub const FORMAT: u32 = 1;
/// A real file is a few KB; anything this big isn't one, and isn't read.
pub const MAX_BYTES: u64 = 10 << 20;
pub const NOT_SECOPY: &str = "This isn't a Secopy file.";
pub const NOTHING: &str = "There is nothing in this file to import.";

/// A file's contents: each preset read on its own, so one bad preset doesn't block the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Contents {
    pub settings: Option<Result<Settings, String>>,
    pub copy_presets: Vec<Result<CopyPresetInput, Unreadable>>,
    pub mirror_presets: Vec<Result<MirrorPresetInput, Unreadable>>,
}

/// A preset in the file that can't be read: its name if it has one, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Unreadable {
    pub name: String,
    pub why: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileOut<'a> {
    secopy: u32,
    app: &'a str,
    exported: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<&'a Settings>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    copy_presets: Vec<CopyPresetInput>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mirror_presets: Vec<MirrorPresetInput>,
}

/// The file's text: what was chosen, without ids.
pub fn export_text(
    settings: Option<&Settings>,
    copy: &[CopyPreset],
    mirrors: &[MirrorPreset],
    app: &str,
    now: chrono::DateTime<chrono::Local>,
) -> String {
    serde_json::to_string_pretty(&FileOut {
        secopy: FORMAT,
        app,
        exported: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        settings,
        copy_presets: copy.iter().map(CopyPreset::input).collect(),
        mirror_presets: mirrors.iter().map(MirrorPreset::input).collect(),
    })
    .expect("presets serialize")
}

/// Writes `text` to `path`: a temporary name next to it, synced, then renamed into place, so
/// a failure never leaves half a file under the final name.
pub fn write_file(path: &Path, text: &str) -> Result<(), String> {
    let name = path.file_name().ok_or("That isn't a file name.")?.to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.secopy-tmp"));
    let written = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written.map_err(|e| format!("The file couldn't be saved: {e}"))
}

/// Reads the file at `path`; one over [`MAX_BYTES`] isn't read at all.
pub fn read_file(path: &Path) -> Result<Contents, String> {
    let size = fs::metadata(path).map_err(|e| format!("The file can't be opened: {e}"))?.len();
    if size > MAX_BYTES {
        return Err("This file is too big to be a Secopy file.".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("The file can't be read: {e}"))?;
    read(&bytes)
}

/// Reads a file's bytes. Strict about what it is (a Secopy file of a format this Secopy
/// knows), lenient inside: unknown fields are ignored, and a preset that can't be read is
/// kept as [`Unreadable`] next to the others.
pub fn read(bytes: &[u8]) -> Result<Contents, String> {
    // serde_json stops at 128 levels of nesting: a hostile file is an error, not a crash.
    let value: Value = serde_json::from_slice(bytes).map_err(|_| NOT_SECOPY.to_string())?;
    let Value::Object(mut file) = value else {
        return Err(NOT_SECOPY.into());
    };
    let format = file.get("secopy").and_then(Value::as_u64).ok_or(NOT_SECOPY)?;
    if format > u64::from(FORMAT) {
        return Err(format!(
            "This file was made by a newer Secopy (format {format}). Update Secopy to import it."
        ));
    }
    let settings = file.remove("settings").map(|v| {
        serde_json::from_value::<Settings>(v)
            .map_err(|_| "The settings in this file can't be read.".to_string())
    });
    let copy_presets = presets(file.remove("copyPresets"), "A copy preset with no name");
    let mirror_presets = presets(file.remove("mirrorPresets"), "A mirror preset with no name");
    if settings.is_none() && copy_presets.is_empty() && mirror_presets.is_empty() {
        return Err(NOTHING.into());
    }
    Ok(Contents { settings, copy_presets, mirror_presets })
}

/// Each entry of a preset list, read on its own.
fn presets<T: serde::de::DeserializeOwned>(
    list: Option<Value>,
    unnamed: &str,
) -> Vec<Result<T, Unreadable>> {
    let Some(Value::Array(items)) = list else {
        return Vec::new();
    };
    items
        .into_iter()
        .map(|item| {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .map(|n| n.trim().chars().take(200).collect::<String>())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| unnamed.to_string());
            serde_json::from_value::<T>(item).map_err(|e| Unreadable {
                name,
                why: format!("Its details can't be read ({e})."),
            })
        })
        .collect()
}
```

Note: `Settings` must deserialize from the camelCase keys it serializes to; its existing `Deserialize` impl (via `SettingsOnDisk`) already does, with defaults for missing fields.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -q -p secopy-app --lib transfer 2>&1 | grep -E "test result|panicked"`
Expected: `test result: ok. 10 passed`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -q -- -D warnings
git add crates/secopy-app/src/transfer.rs crates/secopy-app/src/lib.rs
git commit -F - <<'EOF'
feat(app): write and read .secopy files

Refs #77

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3: What an import would do, and doing it (pure)

**Files:**
- Modify: `crates/secopy-app/src/transfer.rs`
- Test: `crates/secopy-app/src/transfer.rs` (tests module)

**Interfaces:**
- Consumes: Task 1 (`normalized`, `free_name`, `named`, `add`, `edit`), Task 2 (`Contents`, `Unreadable`).
- Produces (all `Serialize`/`Deserialize` + `specta::Type`, `#[serde(rename_all = "camelCase")]`):
  - `pub struct ImportView { pub file_name: String, pub settings: Option<SettingsImport>, pub copy_presets: Vec<PresetImport>, pub mirror_presets: Vec<PresetImport> }`
  - `pub struct SettingsImport { pub changes: Vec<String>, pub problem: Option<String> }`
  - `pub struct PresetImport { pub name: String, pub paths: Vec<String>, pub clash: Option<String>, pub new_name: String, pub missing: Vec<String>, pub problem: Option<String> }`
  - `pub struct ImportChoices { pub settings: bool, pub copy_presets: Vec<PresetChoice>, pub mirror_presets: Vec<PresetChoice> }` (Deserialize, Type; also Clone, Debug, Default)
  - `pub struct PresetChoice { pub index: u32, pub replace: bool }`
  - `pub fn plan(file_name: &str, c: &Contents, copy: &CopyPresets, mirrors: &MirrorPresets, settings: &Settings, exists: &dyn Fn(&str) -> bool) -> ImportView`
  - `pub fn apply_copy(c: &Contents, chosen: &[PresetChoice], copy: &CopyPresets) -> Result<(CopyPresets, usize), String>`
  - `pub fn apply_mirrors(c: &Contents, chosen: &[PresetChoice], mirrors: &MirrorPresets) -> Result<(MirrorPresets, usize), String>`
  - `pub fn settings_changes(from: &Settings, to: &Settings) -> Vec<String>`

- [ ] **Step 1: Write the failing tests** (add to the tests module)

```rust
    fn contents(text: &str) -> Contents {
        read(text.as_bytes()).unwrap()
    }

    const TWO_COPIES: &str = r#"{"secopy":1,"copyPresets":[
        {"name":"sony fx3","source":"/Volumes/NEW/CLIP","includeFolder":false,"extensions":["mov"]},
        {"name":"DJI","source":"/nowhere/DCIM","includeFolder":true,"extensions":null}
    ]}"#;

    #[test]
    fn a_clash_offers_keep_both_with_a_free_name() {
        let view = plan("x.secopy", &contents(TWO_COPIES), &copy_presets(), &MirrorPresets::default(),
            &Settings::default(), &|p| p.starts_with("/Volumes"));
        let fx3 = &view.copy_presets[0];
        assert_eq!(fx3.clash.as_deref(), Some("Sony FX3"));
        assert_eq!(fx3.new_name, "sony fx3 (2)");
        assert!(fx3.missing.is_empty());
        let dji = &view.copy_presets[1];
        assert_eq!((dji.clash.as_deref(), dji.new_name.as_str()), (None, "DJI"));
        assert_eq!(dji.missing, ["/nowhere/DCIM"]);
    }

    #[test]
    fn names_in_the_file_dont_clash_with_each_other() {
        let text = r#"{"secopy":1,"copyPresets":[
            {"name":"A","source":"","includeFolder":true,"extensions":null},
            {"name":"a","source":"","includeFolder":true,"extensions":null}
        ]}"#;
        let c = contents(text);
        let view = plan("x", &c, &CopyPresets::default(), &MirrorPresets::default(), &Settings::default(), &|_| true);
        assert_eq!(view.copy_presets[0].new_name, "A");
        assert_eq!(view.copy_presets[1].new_name, "a (2)");
        let all = [PresetChoice { index: 0, replace: false }, PresetChoice { index: 1, replace: false }];
        let (after, n) = apply_copy(&c, &all, &CopyPresets::default()).unwrap();
        assert_eq!(n, 2);
        let names: Vec<_> = after.presets.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["A", "a (2)"]);
    }

    #[test]
    fn keep_both_adds_and_replace_keeps_the_id() {
        let c = contents(TWO_COPIES);
        let before = copy_presets();
        let id = before.named("Sony FX3").unwrap().id.clone();
        let (kept, _) = apply_copy(&c, &[PresetChoice { index: 0, replace: false }], &before).unwrap();
        assert_eq!(kept.presets.len(), 3);
        assert!(kept.named("sony fx3 (2)").is_some());
        let (replaced, _) = apply_copy(&c, &[PresetChoice { index: 0, replace: true }], &before).unwrap();
        assert_eq!(replaced.presets.len(), 2);
        let p = replaced.get(&id).unwrap();
        assert_eq!((p.name.as_str(), p.source.as_str()), ("sony fx3", "/Volumes/NEW/CLIP"));
    }

    #[test]
    fn replace_keeps_the_id_a_queued_mirror_uses() {
        let before = mirror_presets();
        let id = before.presets[0].id.clone();
        let text = r#"{"secopy":1,"mirrorPresets":[{"name":"FOOTAGE","origin":"/Volumes/SSD/Footage",
            "destination":"/Volumes/NAS/Footage","deleted":{"mode":"delete","days":0},"deepCheck":false}]}"#;
        let (after, n) = apply_mirrors(&contents(text), &[PresetChoice { index: 0, replace: true }], &before).unwrap();
        assert_eq!(n, 1);
        assert_eq!(after.get(&id).unwrap().destination, "/Volumes/NAS/Footage");
    }

    #[test]
    fn a_preset_that_fails_the_checks_is_a_problem_and_cant_be_chosen() {
        let text = r#"{"secopy":1,"mirrorPresets":[{"name":"Loop","origin":"/a","destination":"/a/b",
            "deleted":{"mode":"archive","days":30},"deepCheck":false}]}"#;
        let c = contents(text);
        let view = plan("x", &c, &CopyPresets::default(), &MirrorPresets::default(), &Settings::default(), &|_| true);
        assert_eq!(view.mirror_presets[0].problem.as_deref(), Some("The destination can't be inside the origin."));
        assert!(apply_mirrors(&c, &[PresetChoice { index: 0, replace: false }], &MirrorPresets::default()).is_err());
    }

    #[test]
    fn unreadable_presets_are_listed_with_why() {
        let text = r#"{"secopy":1,"copyPresets":[{"name":"Bad","source":7}]}"#;
        let view = plan("x", &contents(text), &CopyPresets::default(), &MirrorPresets::default(), &Settings::default(), &|_| true);
        assert_eq!(view.copy_presets[0].name, "Bad");
        assert!(view.copy_presets[0].problem.as_deref().unwrap().starts_with("Its details can't be read"));
    }

    #[test]
    fn settings_show_only_what_changes() {
        let mine = Settings::default();
        let theirs = Settings { write_checksum_file: false, notify_when_done: false, ..Settings::default() };
        assert_eq!(
            settings_changes(&mine, &theirs),
            ["Write the checksum file: on → off", "Notify when a copy finishes: on → off"]
        );
        assert!(settings_changes(&mine, &mine).is_empty());
        let text = format!(r#"{{"secopy":1,"settings":{}}}"#, serde_json::to_string(&theirs).unwrap());
        let view = plan("x", &contents(&text), &CopyPresets::default(), &MirrorPresets::default(), &mine, &|_| true);
        assert_eq!(view.settings.unwrap().changes.len(), 2);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p secopy-app --lib transfer 2>&1 | tail -5`
Expected: compile errors: `plan`, `apply_copy`, `apply_mirrors`, `PresetChoice`, `settings_changes` not found.

- [ ] **Step 3: Implement** (in `transfer.rs`, before the tests; add `use serde::Deserialize; use specta::Type; use crate::store::{CopyPresets, MirrorPresets};`)

```rust
/// What the Import screen shows: nothing is changed by making it.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportView {
    pub file_name: String,
    /// `None`: the file has no settings.
    pub settings: Option<SettingsImport>,
    pub copy_presets: Vec<PresetImport>,
    pub mirror_presets: Vec<PresetImport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsImport {
    /// "Write the checksum file: on → off"; empty when they're the same as yours.
    pub changes: Vec<String>,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetImport {
    pub name: String,
    /// A copy preset's source, or a mirror's origin and destination.
    pub paths: Vec<String>,
    /// The name of your preset it has, in any letter case.
    pub clash: Option<String>,
    /// The name Keep both gives it (its own when nothing clashes).
    pub new_name: String,
    /// Paths that aren't on this Mac now: a note, not an error.
    pub missing: Vec<String>,
    /// Why it can't be imported.
    pub problem: Option<String>,
}

/// What the user ticked: presets by their index in the file.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportChoices {
    pub settings: bool,
    pub copy_presets: Vec<PresetChoice>,
    pub mirror_presets: Vec<PresetChoice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetChoice {
    pub index: u32,
    /// Replace your preset with the same name; otherwise Keep both.
    pub replace: bool,
}

/// The Import screen for `c`, against what this Mac has now. `exists` tells whether a path
/// is there (a test passes its own).
pub fn plan(
    file_name: &str,
    c: &Contents,
    copy: &CopyPresets,
    mirrors: &MirrorPresets,
    settings: &Settings,
    exists: &dyn Fn(&str) -> bool,
) -> ImportView {
    let settings = c.settings.as_ref().map(|s| match s {
        Ok(theirs) => SettingsImport { changes: settings_changes(settings, theirs), problem: None },
        Err(why) => SettingsImport { changes: Vec::new(), problem: Some(why.clone()) },
    });
    // Keep both for every preset, in order, so names in the file don't clash with each other.
    let mut copy_names = copy.clone();
    let copy_presets = c
        .copy_presets
        .iter()
        .map(|p| match p.as_ref().map_err(|u| u.clone()).and_then(|p| {
            CopyPresets::normalized(p.clone()).map_err(|why| Unreadable { name: p.name.clone(), why })
        }) {
            Err(u) => unreadable(u),
            Ok(p) => {
                let row = PresetImport {
                    clash: copy.named(&p.name).map(|x| x.name.clone()),
                    new_name: copy_names.free_name(&p.name),
                    missing: [&p.source].into_iter().filter(|s| !s.is_empty() && !exists(s)).cloned().collect(),
                    paths: [p.source.clone()].into_iter().filter(|s| !s.is_empty()).collect(),
                    name: p.name.clone(),
                    problem: None,
                };
                let _ = copy_names.add(CopyPresetInput { name: row.new_name.clone(), ..p });
                row
            }
        })
        .collect();
    let mut mirror_names = mirrors.clone();
    let mirror_presets = c
        .mirror_presets
        .iter()
        .map(|p| match p.as_ref().map_err(|u| u.clone()).and_then(|p| {
            MirrorPresets::normalized(p.clone()).map_err(|why| Unreadable { name: p.name.clone(), why })
        }) {
            Err(u) => unreadable(u),
            Ok(p) => {
                let paths = vec![p.origin.clone(), p.destination.clone()];
                let row = PresetImport {
                    clash: mirrors.named(&p.name).map(|x| x.name.clone()),
                    new_name: mirror_names.free_name(&p.name),
                    missing: paths.iter().filter(|s| !exists(s)).cloned().collect(),
                    paths,
                    name: p.name.clone(),
                    problem: None,
                };
                let _ = mirror_names.add(MirrorPresetInput { name: row.new_name.clone(), ..p });
                row
            }
        })
        .collect();
    ImportView { file_name: file_name.to_string(), settings, copy_presets, mirror_presets }
}

fn unreadable(u: Unreadable) -> PresetImport {
    PresetImport {
        new_name: u.name.clone(),
        name: u.name,
        paths: Vec::new(),
        clash: None,
        missing: Vec::new(),
        problem: Some(u.why),
    }
}

/// The copy presets after importing the chosen ones, in the file's order, and how many.
pub fn apply_copy(c: &Contents, chosen: &[PresetChoice], copy: &CopyPresets) -> Result<(CopyPresets, usize), String> {
    let mut next = copy.clone();
    let chosen = sorted(chosen);
    for &choice in &chosen {
        let input = c
            .copy_presets
            .get(choice.index as usize)
            .and_then(|p| p.as_ref().ok())
            .ok_or("That preset can't be imported.")?;
        let input = CopyPresets::normalized(input.clone())?;
        match next.named(&input.name).map(|p| p.id.clone()) {
            Some(id) if choice.replace => {
                next.edit(&id, input)?;
            }
            _ => {
                let name = next.free_name(&input.name);
                next.add(CopyPresetInput { name, ..input })?;
            }
        }
    }
    Ok((next, chosen.len()))
}

/// The mirror presets after importing the chosen ones, and how many.
pub fn apply_mirrors(c: &Contents, chosen: &[PresetChoice], mirrors: &MirrorPresets) -> Result<(MirrorPresets, usize), String> {
    let mut next = mirrors.clone();
    let chosen = sorted(chosen);
    for &choice in &chosen {
        let input = c
            .mirror_presets
            .get(choice.index as usize)
            .and_then(|p| p.as_ref().ok())
            .ok_or("That preset can't be imported.")?;
        let input = MirrorPresets::normalized(input.clone())?;
        match next.named(&input.name).map(|p| p.id.clone()) {
            Some(id) if choice.replace => {
                next.edit(&id, input)?;
            }
            _ => {
                let name = next.free_name(&input.name);
                next.add(MirrorPresetInput { name, ..input })?;
            }
        }
    }
    Ok((next, chosen.len()))
}

/// In the file's order, each index once.
fn sorted(chosen: &[PresetChoice]) -> Vec<PresetChoice> {
    let mut chosen = chosen.to_vec();
    chosen.sort_by_key(|c| c.index);
    chosen.dedup_by_key(|c| c.index);
    chosen
}

/// Each setting that differs, in the words of the Settings screen.
pub fn settings_changes(from: &Settings, to: &Settings) -> Vec<String> {
    let on = |b: bool| if b { "on" } else { "off" };
    [
        ("Write the checksum file", from.write_checksum_file, to.write_checksum_file),
        ("Show the count of skipped system files", from.show_system_count, to.show_system_count),
        ("Save the report next to the checksum file", from.report_next_to_checksum, to.report_next_to_checksum),
        ("Notify when a copy finishes", from.notify_when_done, to.notify_when_done),
    ]
    .into_iter()
    .filter(|(_, a, b)| a != b)
    .map(|(what, a, b)| format!("{what}: {} → {}", on(a), on(b)))
    .collect()
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -q -p secopy-app --lib transfer 2>&1 | grep -E "test result|panicked"`
Expected: `test result: ok. 17 passed`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -q -- -D warnings
git add crates/secopy-app/src/transfer.rs
git commit -F - <<'EOF'
feat(app): work out and apply an import

Keep both takes the first free name, Replace keeps the preset's id, and
a preset that fails the usual checks can't be chosen.

Refs #77

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 4: Export and import commands

**Files:**
- Modify: `crates/secopy-app/src/commands.rs`, `crates/secopy-app/src/lib.rs` (commands list), `ui/src/lib/bindings.ts` (regenerated)
- Test: `crates/secopy-app/src/commands.rs` (tests module)

**Interfaces:**
- Consumes: Task 2 (`export_text`, `write_file`, `read_file`, `Contents`), Task 3 (`plan`, `apply_copy`, `apply_mirrors`, `ImportView`, `ImportChoices`).
- Produces:
  - `AppState` fields: `importing: Mutex<Option<Contents>>`, `opened: Mutex<Option<PathBuf>>`.
  - `pub const IMPORT_WAITS: &str = "Import it when the copy has finished.";`
  - `#[derive(Deserialize, Type)] #[serde(rename_all = "camelCase")] pub struct ExportWhat { pub settings: bool, pub copy_presets: bool, pub mirror_presets: bool }`
  - `#[derive(Serialize, Type)] #[serde(rename_all = "camelCase")] pub struct ImportDone { pub message: String, pub failed: bool, pub settings: Settings, pub copy_presets: Vec<CopyPreset>, pub mirror_presets: Vec<MirrorPreset> }`
  - AppState methods: `export_all(&self, path: &Path, what: &ExportWhat) -> Result<String, String>`, `export_copy_preset(&self, id: &str, path: &Path) -> Result<String, String>`, `export_mirror_preset(&self, id: &str, path: &Path) -> Result<String, String>`, `open_import(&self, path: &Path) -> Result<ImportView, String>`, `apply_import(&self, choices: &ImportChoices) -> Result<ImportDone, String>`, `set_opened(&self, path: PathBuf)`, `take_opened(&self) -> Option<String>`.
  - Tauri commands (specta): `export_all(app, path: String, what: ExportWhat) -> Result<String, String>`, `export_copy_preset(app, id: String, path: String)`, `export_mirror_preset(app, id: String, path: String)`, `open_import(app, path: String) -> Result<ImportView, String>`, `apply_import(app, choices: ImportChoices) -> Result<ImportDone, String>`, `take_opened_file(app) -> Option<String>`.

- [ ] **Step 1: Write the failing tests** (commands.rs tests module; `AppState::new(dir)` is how existing tests build a state)

```rust
    fn with_presets(dir: &Path) -> AppState {
        let state = AppState::new(dir.join("data"));
        state
            .change_copy_presets(|p| p.add(crate::store::CopyPresetInput {
                name: "Sony FX3".into(),
                source: "/Volumes/CARD_A/CLIP".into(),
                include_folder: true,
                extensions: None,
            }))
            .unwrap();
        state
    }

    /// #77: export on one Mac, import on another: the same presets and settings.
    #[test]
    fn exported_presets_import_on_another_mac() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let file = dir.path().join("all.secopy");
        let line = a
            .export_all(&file, &ExportWhat { settings: true, copy_presets: true, mirror_presets: true })
            .unwrap();
        assert_eq!(line, "Exported 1 copy preset and the settings.");
        let b = AppState::new(dir.path().join("other"));
        let view = b.open_import(&file).unwrap();
        assert_eq!(view.copy_presets[0].name, "Sony FX3");
        let done = b
            .apply_import(&ImportChoices {
                settings: true,
                copy_presets: vec![PresetChoice { index: 0, replace: false }],
                mirror_presets: vec![],
            })
            .unwrap();
        assert!(!done.failed);
        assert_eq!(done.message, "Imported 1 copy preset and the settings.");
        assert_eq!(done.copy_presets[0].source, "/Volumes/CARD_A/CLIP");
        let reloaded = AppState::new(dir.path().join("other"));
        assert_eq!(lock(&reloaded.copy_presets).presets.len(), 1, "saved to disk");
    }

    /// #77: opening the Import screen changes nothing.
    #[test]
    fn opening_an_import_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let file = dir.path().join("one.secopy");
        let id = lock(&a.copy_presets).presets[0].id.clone();
        a.export_copy_preset(&id, &file).unwrap();
        let before = lock(&a.copy_presets).clone();
        a.open_import(&file).unwrap();
        assert_eq!(*lock(&a.copy_presets), before);
    }

    /// Review focus 5: nothing is imported while a job or the queue runs.
    #[test]
    fn importing_waits_while_a_job_runs() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let file = dir.path().join("one.secopy");
        let id = lock(&a.copy_presets).presets[0].id.clone();
        a.export_copy_preset(&id, &file).unwrap();
        a.open_import(&file).unwrap();
        lock(&a.queue_run).running = true;
        assert_eq!(a.open_import(&file).unwrap_err(), IMPORT_WAITS);
        let choices = ImportChoices { copy_presets: vec![PresetChoice { index: 0, replace: false }], ..Default::default() };
        assert_eq!(a.apply_import(&choices).unwrap_err(), IMPORT_WAITS);
        assert_eq!(lock(&a.copy_presets).presets.len(), 1);
    }

    /// Review focus 4: a save that fails partway says what was imported.
    #[test]
    fn a_failed_save_says_what_was_imported() {
        let dir = tempfile::tempdir().unwrap();
        let a = with_presets(dir.path());
        let text = r#"{"secopy":1,
            "copyPresets":[{"name":"DJI","source":"","includeFolder":true,"extensions":null}],
            "mirrorPresets":[{"name":"M","origin":"/a","destination":"/b","deleted":{"mode":"delete","days":0},"deepCheck":false}]}"#;
        let file = dir.path().join("mixed.secopy");
        std::fs::write(&file, text).unwrap();
        a.open_import(&file).unwrap();
        // mirrors.json can't be written: a directory is in its way.
        std::fs::create_dir_all(dir.path().join("data").join("mirrors.json.tmp")).unwrap();
        let done = a
            .apply_import(&ImportChoices {
                settings: false,
                copy_presets: vec![PresetChoice { index: 0, replace: false }],
                mirror_presets: vec![PresetChoice { index: 0, replace: false }],
            })
            .unwrap();
        assert!(done.failed);
        assert!(done.message.starts_with("Imported 1 copy preset. The mirror presets couldn't be saved:"), "{}", done.message);
        assert_eq!(lock(&a.copy_presets).presets.len(), 2);
        assert!(lock(&a.mirrors).presets.is_empty());
    }

    #[test]
    fn a_file_opened_from_finder_is_taken_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = AppState::new(dir.path().join("data"));
        a.set_opened(PathBuf::from("/Users/me/Sony FX3.secopy"));
        assert_eq!(a.take_opened().as_deref(), Some("/Users/me/Sony FX3.secopy"));
        assert_eq!(a.take_opened(), None);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p secopy-app --lib commands 2>&1 | tail -5`
Expected: compile errors: no `export_all`, `ExportWhat`, `open_import`, `IMPORT_WAITS`, …

- [ ] **Step 3: Implement** (commands.rs)

Add to `AppState` (and `None`-initialize both in `AppState::new`):

```rust
    /// The file on the Import screen, as read when it opened: Import applies exactly this.
    importing: Mutex<Option<crate::transfer::Contents>>,
    /// A `.secopy` file opened from Finder that the window hasn't shown yet.
    opened: Mutex<Option<PathBuf>>,
```

Then:

```rust
use crate::transfer::{self, ImportChoices, ImportView, PresetChoice};

pub const IMPORT_WAITS: &str = "Import it when the copy has finished.";

/// What goes in an export.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportWhat {
    pub settings: bool,
    pub copy_presets: bool,
    pub mirror_presets: bool,
}

/// After Import: what to say, and everything the window shows, as saved.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportDone {
    pub message: String,
    /// Part of it couldn't be saved; `message` says what.
    pub failed: bool,
    pub settings: Settings,
    pub copy_presets: Vec<CopyPreset>,
    pub mirror_presets: Vec<MirrorPreset>,
}

/// "1 copy preset, 2 mirror presets and the settings".
fn what_line(copies: usize, mirrors: usize, settings: bool) -> String {
    let mut parts = Vec::new();
    if copies > 0 {
        parts.push(format!("{copies} copy preset{}", if copies == 1 { "" } else { "s" }));
    }
    if mirrors > 0 {
        parts.push(format!("{mirrors} mirror preset{}", if mirrors == 1 { "" } else { "s" }));
    }
    if settings {
        parts.push("the settings".to_string());
    }
    match parts.len() {
        0 => "nothing".to_string(),
        1 => parts.remove(0),
        n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
    }
}

impl AppState {
    pub fn export_all(&self, path: &Path, what: &ExportWhat) -> Result<String, String> {
        let settings = what.settings.then(|| lock(&self.settings).clone());
        let copies = if what.copy_presets { lock(&self.copy_presets).presets.clone() } else { Vec::new() };
        let mirrors = if what.mirror_presets { lock(&self.mirrors).presets.clone() } else { Vec::new() };
        if settings.is_none() && copies.is_empty() && mirrors.is_empty() {
            return Err("Choose something to export.".into());
        }
        let text = transfer::export_text(settings.as_ref(), &copies, &mirrors, env!("CARGO_PKG_VERSION"), Local::now());
        transfer::write_file(path, &text)?;
        Ok(format!("Exported {}.", what_line(copies.len(), mirrors.len(), settings.is_some())))
    }

    pub fn export_copy_preset(&self, id: &str, path: &Path) -> Result<String, String> {
        let preset = lock(&self.copy_presets).get(id).cloned().ok_or("That preset no longer exists.")?;
        let text = transfer::export_text(None, std::slice::from_ref(&preset), &[], env!("CARGO_PKG_VERSION"), Local::now());
        transfer::write_file(path, &text)?;
        Ok(format!("Exported “{}”.", preset.name))
    }

    pub fn export_mirror_preset(&self, id: &str, path: &Path) -> Result<String, String> {
        let preset = lock(&self.mirrors).get(id).cloned().ok_or("That mirror no longer exists.")?;
        let text = transfer::export_text(None, &[], std::slice::from_ref(&preset), env!("CARGO_PKG_VERSION"), Local::now());
        transfer::write_file(path, &text)?;
        Ok(format!("Exported “{}”.", preset.name))
    }

    /// Reads `path` for the Import screen. Changes nothing.
    pub fn open_import(&self, path: &Path) -> Result<ImportView, String> {
        if self.busy() {
            return Err(IMPORT_WAITS.into());
        }
        let contents = transfer::read_file(path)?;
        let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let view = transfer::plan(
            &name,
            &contents,
            &lock(&self.copy_presets),
            &lock(&self.mirrors),
            &lock(&self.settings),
            &|p| Path::new(p).exists(),
        );
        *lock(&self.importing) = Some(contents);
        Ok(view)
    }

    /// Imports what was ticked, kind by kind (copy presets, mirror presets, settings), each
    /// saved before the next; the first failure stops it, and the message says what went in.
    pub fn apply_import(&self, choices: &ImportChoices) -> Result<ImportDone, String> {
        if self.busy() {
            return Err(IMPORT_WAITS.into());
        }
        let contents = lock(&self.importing).clone().ok_or("There is no file to import.")?;
        let mut done = (0usize, 0usize, false);
        let failure = (|| -> Result<(), (String, String)> {
            if !choices.copy_presets.is_empty() {
                let n = self
                    .change_copy_presets(|p| {
                        let (next, n) = transfer::apply_copy(&contents, &choices.copy_presets, p)?;
                        *p = next;
                        Ok(n)
                    })
                    .map_err(|e| ("The copy presets".to_string(), e))?;
                done.0 = n;
            }
            if !choices.mirror_presets.is_empty() {
                let mut n = 0;
                self.change_mirrors(|m| {
                    let (next, applied) = transfer::apply_mirrors(&contents, &choices.mirror_presets, m)?;
                    *m = next;
                    n = applied;
                    Ok(())
                })
                .map_err(|e| ("The mirror presets".to_string(), e))?;
                done.1 = n;
            }
            if choices.settings
                && let Some(Ok(theirs)) = &contents.settings
            {
                self.set_settings(theirs.clone()).map_err(|e| ("The settings".to_string(), e))?;
                done.2 = true;
            }
            Ok(())
        })()
        .err();
        let imported = what_line(done.0, done.1, done.2);
        let (message, failed) = match failure {
            None => (format!("Imported {imported}."), false),
            Some((what, why)) if done == (0, 0, false) => (format!("{what} couldn't be saved: {why}"), true),
            Some((what, why)) => (format!("Imported {imported}. {what} couldn't be saved: {why}"), true),
        };
        if !failed {
            *lock(&self.importing) = None;
        }
        Ok(ImportDone {
            message,
            failed,
            settings: lock(&self.settings).clone(),
            copy_presets: lock(&self.copy_presets).presets.clone(),
            mirror_presets: lock(&self.mirrors).presets.clone(),
        })
    }

    pub fn set_opened(&self, path: PathBuf) {
        *lock(&self.opened) = Some(path);
    }

    pub fn take_opened(&self) -> Option<String> {
        lock(&self.opened).take().map(|p| show(&p))
    }
}
```

Adjust to the real helpers: `change_mirrors` returns `Result<Vec<MirrorPreset>, String>` and wraps errors as "Couldn't save the mirror: …" — that's the `why` in the message, fine. `change_copy_presets` wraps as "Couldn't save the preset: …". If the message reads awkwardly ("The mirror presets couldn't be saved: Couldn't save the mirror: …"), strip the helper's prefix by mapping inside the closure instead, and make the test's `starts_with` match the final wording. `show` is `crate::dto::show`.

Tauri commands (next to the other `#[tauri::command] #[specta::specta]` functions; use `blocking` like the others that touch the disk):

```rust
#[tauri::command]
#[specta::specta]
pub async fn export_all(app: AppHandle, path: String, what: ExportWhat) -> Result<String, String> {
    blocking(app, move |s| s.export_all(Path::new(&path), &what)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn export_copy_preset(app: AppHandle, id: String, path: String) -> Result<String, String> {
    blocking(app, move |s| s.export_copy_preset(&id, Path::new(&path))).await?
}

#[tauri::command]
#[specta::specta]
pub async fn export_mirror_preset(app: AppHandle, id: String, path: String) -> Result<String, String> {
    blocking(app, move |s| s.export_mirror_preset(&id, Path::new(&path))).await?
}

#[tauri::command]
#[specta::specta]
pub async fn open_import(app: AppHandle, path: String) -> Result<ImportView, String> {
    blocking(app, move |s| s.open_import(Path::new(&path))).await?
}

#[tauri::command]
#[specta::specta]
pub async fn apply_import(app: AppHandle, choices: ImportChoices) -> Result<ImportDone, String> {
    blocking(app, move |s| s.apply_import(&choices)).await?
}

/// A `.secopy` file opened from Finder, once.
#[tauri::command]
#[specta::specta]
pub fn take_opened_file(state: State<'_, AppState>) -> Option<String> {
    state.take_opened()
}
```

Register all six in the `collect_commands![…]` list in `lib.rs`, then regenerate the bindings.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -q -p secopy-app 2>&1 | grep -E "test result|panicked"` then `SECOPY_UPDATE_BINDINGS=1 cargo test -q -p secopy-app --lib ui_bindings`
Expected: all pass; `ui/src/lib/bindings.ts` gains `exportAll`, `exportCopyPreset`, `exportMirrorPreset`, `openImport`, `applyImport`, `takeOpenedFile` and the types.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -q -- -D warnings && (cd ui && npm run check)
git add crates/secopy-app/src ui/src/lib/bindings.ts
git commit -F - <<'EOF'
feat(app): export and import commands

Import reads the file when its screen opens and applies exactly that;
each kind is saved before the next, and a failure says what went in.
Nothing is imported while a job or the queue runs.

Refs #77

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 5: File menu and files opened from Finder

**Files:**
- Modify: `crates/secopy-app/src/lib.rs`, `crates/secopy-app/tauri.conf.json`
- Test: `crates/secopy-app/src/lib.rs` (tests module)

**Interfaces:**
- Consumes: Task 4 (`AppState::set_opened`).
- Produces: menu ids `IMPORT_FILE = "import-file"`, `EXPORT_FILE = "export-file"` sent on `MENU_EVENT` like the others; event `OPEN_FILE = "open-file"` (no payload) when Finder opens a `.secopy`; `menu_state` returns `[bool; 5]` = [Choose Source, Choose Destination, Start Copy, Cancel Copy, Import…].

- [ ] **Step 1: Write the failing test** (lib.rs tests; replace the existing `menu_state` assertions with these)

```rust
    #[test]
    fn import_is_off_while_copying() {
        assert_eq!(menu_state(true, true, false), [true, true, true, false, true]);
        assert_eq!(menu_state(false, false, true), [false, false, false, true, false]);
    }

    #[test]
    fn only_secopy_files_are_imported_from_finder() {
        assert_eq!(opened_file(&["file:///Users/me/Sony%20FX3.secopy".parse().unwrap()]),
            Some(PathBuf::from("/Users/me/Sony FX3.secopy")));
        assert_eq!(opened_file(&["file:///Users/me/notes.txt".parse().unwrap()]), None);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -q -p secopy-app --lib import_is_off only_secopy 2>&1 | tail -3`
Expected: compile errors (array length 4, no `opened_file`).

- [ ] **Step 3: Implement**

```rust
const IMPORT_FILE: &str = "import-file";
const EXPORT_FILE: &str = "export-file";
/// Tells the window a `.secopy` was opened from Finder; it asks for it with `take_opened_file`.
pub const OPEN_FILE: &str = "open-file";
```

- Add both ids to `MENU_ITEMS` (length 10).
- `FileMenu.items: [MenuItem<R>; 5]` with `item(IMPORT_FILE)?` last; `menu_state` → `[setup, setup, setup && can_start, copying, !copying]`.
- In `menu()`: after `&cancel`, add `&PredefinedMenuItem::separator(app)?`, `&import`, `&export` where

```rust
    let import = MenuItem::with_id(app, IMPORT_FILE, "Import…", true, None::<&str>)?;
    let export = MenuItem::with_id(app, EXPORT_FILE, "Export…", true, None::<&str>)?;
```

- The first `.secopy` URL among those Finder opens:

```rust
/// The first `.secopy` file among `urls` Finder opened Secopy with.
fn opened_file(urls: &[tauri::Url]) -> Option<PathBuf> {
    urls.iter()
        .filter_map(|u| u.to_file_path().ok())
        .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("secopy")))
}
```

- In `.run(|app, event| …)`, before the exit branch:

```rust
            if let RunEvent::Opened { urls } = &event
                && let Some(path) = opened_file(urls)
            {
                app.state::<AppState>().set_opened(path);
                let _ = app.emit(OPEN_FILE, ());
            }
```

- `tauri.conf.json`, inside `"bundle"`:

```json
    "fileAssociations": [
      {
        "ext": ["secopy"],
        "name": "Secopy settings",
        "description": "Secopy settings and presets",
        "role": "Editor",
        "mimeType": "application/json"
      }
    ],
```

  Check the Tauri CLI's schema (`ui/node_modules/@tauri-apps/cli/config.schema.json`, `FileAssociation`) for an `exportedType` field; if present, add `"exportedType": { "identifier": "com.latecommits.secopy.settings", "conformsTo": ["public.json"] }` so macOS knows the extension. After Task 9's build, confirm with `plutil -p target/release/bundle/macos/Secopy.app/Contents/Info.plist | grep -A8 CFBundleDocumentTypes`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -q -p secopy-app 2>&1 | grep -E "test result|panicked"` and `cargo build -q -p secopy-app`
Expected: all pass; builds.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -q -- -D warnings
git add crates/secopy-app/src/lib.rs crates/secopy-app/tauri.conf.json
git commit -F - <<'EOF'
feat(app): Import… and Export… in the File menu; open .secopy from Finder

Refs #77

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 6: Export in the UI

**Files:**
- Create: `ui/src/components/ExportDialog.svelte`, `ui/src/components/ExportDialog.test.ts`
- Modify: `ui/src/lib/api.ts`, `ui/src/test/fake-api.ts`, `ui/src/App.svelte`, `ui/src/App.test.ts`, `ui/src/components/SettingsScreen.svelte`, `CopyPresetsScreen.svelte`, `MirrorScreen.svelte` (+ their tests)

**Interfaces:**
- Consumes: Task 4 bindings (`commands.exportAll`, `exportCopyPreset`, `exportMirrorPreset`, types `ExportWhat`), Task 5 menu item `export-file`.
- Produces (api.ts):
  - `exportAll(path: string, what: ExportWhat): Promise<string>`
  - `exportCopyPreset(id: string, path: string): Promise<string>`, `exportMirrorPreset(id: string, path: string): Promise<string>`
  - `pickExportPath(suggested: string): Promise<string | null>` — `save({ defaultPath: suggested, filters: [{ name: "Secopy settings", extensions: ["secopy"] }] })`
  - App: `info: string | null` shown in `banner` as `<Notice tone="success">`; `exportAll()` opens the dialog.

- [ ] **Step 1: Write the failing tests**

`ui/src/components/ExportDialog.test.ts`:

```ts
import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import ExportDialog from "./ExportDialog.svelte";

describe("ExportDialog", () => {
  test("everything there is ticked; an empty kind is off", async () => {
    const onExport = vi.fn();
    render(ExportDialog, { props: { copyPresets: 3, mirrorPresets: 0, onExport, onClose: () => {} } });
    expect(screen.getByRole("checkbox", { name: "Settings" })).toHaveProperty("checked", true);
    expect(screen.getByRole("checkbox", { name: "Copy presets (3)" })).toHaveProperty("checked", true);
    const mirrors = screen.getByRole("checkbox", { name: "Mirror presets (none)" });
    expect(mirrors).toHaveProperty("checked", false);
    expect(mirrors).toHaveProperty("disabled", true);
    await fireEvent.click(screen.getByRole("checkbox", { name: "Settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    expect(onExport).toHaveBeenCalledWith({ settings: false, copyPresets: true, mirrorPresets: false });
  });

  test("with nothing ticked, Export… is off", async () => {
    render(ExportDialog, { props: { copyPresets: 0, mirrorPresets: 0, onExport: () => {}, onClose: () => {} } });
    await fireEvent.click(screen.getByRole("checkbox", { name: "Settings" }));
    expect(screen.getByRole("button", { name: "Export…" })).toHaveProperty("disabled", true);
  });
});
```

In `App.test.ts` (use the file's `app()` helper and `state.menu`):

```ts
  test("File › Export… asks what, then where, and says what was exported", async () => {
    const { api, state } = app();
    await startButton();
    api.pickExportPath.mockResolvedValue("/Users/me/Secopy settings.secopy");
    api.exportAll.mockResolvedValue("Exported 1 copy preset and the settings.");
    state.menu!("export-file");
    await fireEvent.click(await screen.findByRole("button", { name: "Export…" }));
    await waitFor(() => expect(api.exportAll).toHaveBeenCalledWith("/Users/me/Secopy settings.secopy", expect.objectContaining({ settings: true })));
    expect(api.pickExportPath.mock.calls[0][0]).toMatch(/^Secopy settings \d{4}-\d{2}-\d{2}\.secopy$/);
    await screen.findByText("Exported 1 copy preset and the settings.");
  });
```

In `CopyPresetsScreen.test.ts`:

```ts
  test("Export… saves the selected preset as its name", async () => {
    const { api } = fakeApi();
    api.pickExportPath.mockResolvedValue("/Users/me/Sony FX3.secopy");
    api.exportCopyPreset.mockResolvedValue("Exported “Sony FX3”.");
    render(CopyPresetsScreen, { props: { presets: [copyPreset()], onPresets: () => {}, onView: () => {}, onDone: () => {} }, context: apiContext(api) });
    await fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    expect(api.pickExportPath).toHaveBeenCalledWith("Sony FX3.secopy");
    await waitFor(() => expect(api.exportCopyPreset).toHaveBeenCalledWith(copyPreset().id, "/Users/me/Sony FX3.secopy"));
    await screen.findByText("Exported “Sony FX3”.");
  });
```

Add the same test for `MirrorScreen` (mirror preset fixture from the file's existing tests, `exportMirrorPreset`). For unsaved edits, add: with a changed name, Export… asks `api.confirm` "Discard changes?" first, and with "Keep editing" (`confirm` resolves false) nothing is exported.

- [ ] **Step 2: Run to verify they fail**

Run: `cd ui && npx vitest run src/components/ExportDialog.test.ts src/App.test.ts src/components/CopyPresetsScreen.test.ts src/components/MirrorScreen.test.ts 2>&1 | grep -E "Tests |FAIL" | head`
Expected: FAIL (no ExportDialog; no api.exportAll / pickExportPath; no Export… buttons).

- [ ] **Step 3: Implement**

`api.ts` (in the api object; import `ExportWhat` from bindings):

```ts
  exportAll: (path: string, what: ExportWhat): Promise<string> => unwrap(commands.exportAll(path, what)),
  exportCopyPreset: (id: string, path: string): Promise<string> => unwrap(commands.exportCopyPreset(id, path)),
  exportMirrorPreset: (id: string, path: string): Promise<string> => unwrap(commands.exportMirrorPreset(id, path)),
  /** Where to save a .secopy file. */
  pickExportPath: (suggested: string): Promise<string | null> =>
    save({ defaultPath: suggested, filters: [{ name: "Secopy settings", extensions: ["secopy"] }] }),
```

`fake-api.ts`: add `exportAll`, `exportCopyPreset`, `exportMirrorPreset` (`vi.fn().mockResolvedValue("Exported.")`) and `pickExportPath` (`vi.fn().mockResolvedValue(null)`), following how the other functions are declared there.

`ExportDialog.svelte`:

```svelte
<script lang="ts">
  // File › Export…: what goes in the .secopy file. Kinds with nothing in them are off.
  import type { ExportWhat } from "../lib/bindings";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";

  let {
    copyPresets,
    mirrorPresets,
    onExport,
    onClose,
  }: { copyPresets: number; mirrorPresets: number; onExport: (what: ExportWhat) => void; onClose: () => void } = $props();

  // svelte-ignore state_referenced_locally
  let what: ExportWhat = $state({ settings: true, copyPresets: copyPresets > 0, mirrorPresets: mirrorPresets > 0 });
  const any = $derived(what.settings || what.copyPresets || what.mirrorPresets);
  const count = (n: number) => (n > 0 ? `(${n})` : "(none)");
</script>

<Dialog title="Export" {onClose}>
  <p>Choose what goes in the file. The queue and this Mac's recent destinations stay here.</p>
  <div class="kinds">
    <Checkbox label="Settings" checked={what.settings} onChange={(on) => (what.settings = on)} />
    <Checkbox
      label="Copy presets {count(copyPresets)}"
      checked={what.copyPresets}
      disabled={copyPresets === 0}
      onChange={(on) => (what.copyPresets = on)}
    />
    <Checkbox
      label="Mirror presets {count(mirrorPresets)}"
      checked={what.mirrorPresets}
      disabled={mirrorPresets === 0}
      onChange={(on) => (what.mirrorPresets = on)}
    />
  </div>
  {#snippet actions()}
    <Button onclick={onClose} data-autofocus>Cancel</Button>
    <Button variant="primary" disabled={!any} onclick={() => onExport({ ...what })}>Export…</Button>
  {/snippet}
</Dialog>

<style>
  .kinds {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }
</style>
```

`App.svelte`:
- state: `let exporting = $state(false); let info: string | null = $state(null);`
- banner: `{#if info}<Notice tone="success">{info}</Notice>{/if}` before the error notice; clear `info` when the screen changes (in the same `$effect` that tracks `screen`, or in `go()`).
- `onMenu`: `else if (item === "export-file") exporting = true;`
- functions:

```ts
  const today = () => new Date().toISOString().slice(0, 10);
  async function exportChosen(what: ExportWhat) {
    exporting = false;
    const path = await api.pickExportPath(`Secopy settings ${today()}.secopy`);
    if (!path) return;
    const said = await run(() => api.exportAll(path, what));
    if (said) info = said;
  }
```

  (`run` is App's existing wrapper that sets `error` on failure; reuse it.)
- markup, after the screen block: `{#if exporting}<ExportDialog copyPresets={copyPresets.length} mirrorPresets={mirrorPresets.length} onExport={exportChosen} onClose={() => (exporting = false)} />{/if}`

`SettingsScreen.svelte`: a new `Section title="Settings and presets"` after "Every copy", with a `FormRow label="Move to another Mac"` holding `<Button onclick={onExport}>Export…</Button>` and `<Button onclick={onImport}>Import…</Button>` (new props `onExport`, `onImport`; App passes `() => (exporting = true)` and Task 7's `chooseImport`). Export uses the saved settings, so with unsaved changes the button's `help` says "Exports your saved settings; save first to include these changes."

`CopyPresetsScreen.svelte` and `MirrorScreen.svelte`: an `Export…` button in the action bar's `start` snippet next to `Delete…` (secondary, shown only for a saved selected preset), running:

```ts
  async function exportSelected() {
    if (!selected || asking) return;
    if (changed) {
      asking = true;
      try {
        const discard = await api.confirm(`Your changes to “${selected.name}” aren't saved.`, "Discard changes?", "Discard", "Keep editing");
        if (!discard) return;
        editor?.revert();
      } finally {
        asking = false;
      }
    }
    const path = await api.pickExportPath(`${selected.name}.secopy`);
    if (!path) return;
    try {
      message = await api.exportCopyPreset(selected.id, path); // exportMirrorPreset on MirrorScreen
      error = null;
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    }
  }
```

  Use each screen's existing names for the selected preset, the dirty flag, the "asking" guard, the revert and its message/error state (read the file; `CopyPresetsScreen` has `asking`, `changed`, `editor?.revert()`; add `message` shown as `<Notice tone="success">` where the screen shows its errors if it has no such state). A preset name with `/` or `:` becomes `-` in the suggested file name: `selected.name.replace(/[/:]/g, "-")`.

- [ ] **Step 4: Run to verify they pass**

Run: `cd ui && npx vitest run 2>&1 | grep -E "Tests |×"` and `npm run check 2>&1 | grep COMPLETED`
Expected: all pass; 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src
git commit -F - <<'EOF'
feat(ui): export settings and presets

Refs #77

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 7: The Import screen

**Files:**
- Create: `ui/src/components/ImportScreen.svelte`, `ui/src/components/ImportScreen.test.ts`
- Modify: `ui/src/lib/api.ts`, `ui/src/test/fake-api.ts`, `ui/src/App.svelte`, `ui/src/App.test.ts`, `ui/src/a11y.test.ts`

**Interfaces:**
- Consumes: Task 4 bindings (`openImport`, `applyImport`, `takeOpenedFile`, types `ImportView`, `ImportChoices`, `ImportDone`), Task 5 (`import-file` menu item, `open-file` event), Task 6 (`info`, Settings' `onImport`).
- Produces (api.ts):
  - `pickImportFile(): Promise<string | null>` — `asList(await open({ multiple: false, directory: false, filters: [{ name: "Secopy settings", extensions: ["secopy"] }] }))?.[0] ?? null`
  - `openImport(path: string): Promise<ImportView>`, `applyImport(choices: ImportChoices): Promise<ImportDone>`
  - `takeOpenedFile(): Promise<string | null>` — `commands.takeOpenedFile()`
  - `onOpenFile(cb: () => void): Promise<() => void>` — `listen("open-file", () => cb())`
  - App screen `"import"`; `chooseImport()`; `showImport(path)`.

- [ ] **Step 1: Write the failing tests**

`ui/src/components/ImportScreen.test.ts` (build views inline; `ImportView` fields per Task 3):

```ts
import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import type { ImportView } from "../lib/bindings";
import ImportScreen from "./ImportScreen.svelte";

const view = (over: Partial<ImportView> = {}): ImportView => ({
  fileName: "Team presets.secopy",
  settings: { changes: ["Write the checksum file: on → off"], problem: null },
  copyPresets: [
    { name: "Sony FX3", paths: ["/Volumes/CARD_A/CLIP"], clash: "Sony FX3", newName: "Sony FX3 (2)", missing: [], problem: null },
    { name: "DJI", paths: ["/Volumes/DJI/DCIM"], clash: null, newName: "DJI", missing: ["/Volumes/DJI/DCIM"], problem: null },
    { name: "Bad", paths: [], clash: null, newName: "Bad", missing: [], problem: "Its details can't be read (…)." },
  ],
  mirrorPresets: [],
  ...over,
});

describe("ImportScreen", () => {
  test("shows what's in the file, what changes and what clashes", () => {
    render(ImportScreen, { props: { view: view(), onImport: vi.fn(), onBack: () => {} } });
    screen.getByRole("heading", { name: "Import" });
    screen.getByText("Team presets.secopy");
    screen.getByText("Write the checksum file: on → off");
    screen.getByText("/Volumes/DJI/DCIM isn't connected now.");
    screen.getByText("Its details can't be read (…).");
    expect(screen.getByRole("checkbox", { name: "Bad" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("radio", { name: "Keep both, as “Sony FX3 (2)”" })).toHaveProperty("checked", true);
  });

  test("Import sends what's ticked, with Replace where chosen", async () => {
    const onImport = vi.fn();
    render(ImportScreen, { props: { view: view(), onImport, onBack: () => {} } });
    await fireEvent.click(screen.getByRole("radio", { name: "Replace yours" }));
    await fireEvent.click(screen.getByRole("checkbox", { name: "DJI" }));
    await fireEvent.click(screen.getByRole("button", { name: "Import" }));
    expect(onImport).toHaveBeenCalledWith({
      settings: true,
      copyPresets: [{ index: 0, replace: true }],
      mirrorPresets: [],
    });
  });

  test("settings that are the same as yours can't be ticked; nothing ticked, no Import", async () => {
    const v = view({ settings: { changes: [], problem: null }, copyPresets: [view().copyPresets[1]] });
    render(ImportScreen, { props: { view: v, onImport: vi.fn(), onBack: () => {} } });
    screen.getByText("Same as yours");
    await fireEvent.click(screen.getByRole("checkbox", { name: "DJI" }));
    expect(screen.getByRole("button", { name: "Import" })).toHaveProperty("disabled", true);
  });

  test("Back changes nothing", async () => {
    const onImport = vi.fn();
    const onBack = vi.fn();
    render(ImportScreen, { props: { view: view(), onImport, onBack } });
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(onBack).toHaveBeenCalled();
    expect(onImport).not.toHaveBeenCalled();
  });
});
```

In `App.test.ts`:

```ts
  test("File › Import… opens the file's Import screen; Import says what went in", async () => {
    const { api, state } = app();
    await startButton();
    api.pickImportFile.mockResolvedValue("/Users/me/Team.secopy");
    api.openImport.mockResolvedValue(importView());
    api.applyImport.mockResolvedValue({ message: "Imported 1 copy preset.", failed: false, settings: settings(), copyPresets: [copyPreset()], mirrorPresets: [] });
    state.menu!("import-file");
    await screen.findByRole("heading", { level: 1, name: "Import" });
    await fireEvent.click(screen.getByRole("button", { name: "Import" }));
    await screen.findByText("Imported 1 copy preset.");
    expect(screen.queryByRole("heading", { level: 1, name: "Import" })).toBeNull();
  });

  test("a file opened from Finder while copying says to wait", async () => {
    const { api, state } = app();
    await startButton();
    api.takeOpenedFile.mockResolvedValue("/Users/me/Team.secopy");
    api.openImport.mockRejectedValue(new Error("Import it when the copy has finished."));
    state.openFile!();
    await screen.findByText("Import it when the copy has finished.");
  });
```

Add `importView()` (the `view()` above) and `settings()` fixtures to `ui/src/test/fake-api.ts` if they're not there, and have `fakeApi` capture the `onOpenFile` callback as `state.openFile` the way it captures `onMenu` as `state.menu`. Add the Import screen to `a11y.test.ts` the way the other screens are.

- [ ] **Step 2: Run to verify they fail**

Run: `cd ui && npx vitest run src/components/ImportScreen.test.ts src/App.test.ts 2>&1 | grep -E "Tests |FAIL" | head`
Expected: FAIL (no ImportScreen; no api.openImport).

- [ ] **Step 3: Implement**

`api.ts` additions (import `ImportView`, `ImportChoices`, `ImportDone`):

```ts
  pickImportFile: async (): Promise<string | null> =>
    asList(await open({ multiple: false, directory: false, filters: [{ name: "Secopy settings", extensions: ["secopy"] }] }))?.[0] ?? null,
  openImport: (path: string): Promise<ImportView> => unwrap(commands.openImport(path)),
  applyImport: (choices: ImportChoices): Promise<ImportDone> => unwrap(commands.applyImport(choices)),
  /** A .secopy file opened from Finder, once. */
  takeOpenedFile: (): Promise<string | null> => commands.takeOpenedFile(),
  onOpenFile: (cb: () => void): Promise<() => void> => listen("open-file", () => cb()),
```

`ImportScreen.svelte`:

```svelte
<script lang="ts">
  // Import (#77): what a .secopy file holds, against what this Mac has. Nothing changes
  // until Import; Back leaves everything as it was.
  import type { ImportChoices, ImportView, PresetImport } from "../lib/bindings";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import RadioGroup from "../lib/ui/RadioGroup.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";

  let { view, onImport, onBack }: { view: ImportView; onImport: (c: ImportChoices) => void; onBack: () => void } = $props();

  type Row = { on: boolean; replace: boolean };
  const rows = (list: PresetImport[]): Row[] => list.map((p) => ({ on: !p.problem, replace: false }));
  // svelte-ignore state_referenced_locally
  let settingsOn = $state(!!view.settings && !view.settings.problem && view.settings.changes.length > 0);
  // svelte-ignore state_referenced_locally
  let copy = $state(rows(view.copyPresets));
  // svelte-ignore state_referenced_locally
  let mirrors = $state(rows(view.mirrorPresets));

  const chosen = (list: Row[]) =>
    list.flatMap((r, index) => (r.on ? [{ index, replace: r.replace }] : []));
  const choices = $derived<ImportChoices>({
    settings: settingsOn,
    copyPresets: chosen(copy),
    mirrorPresets: chosen(mirrors),
  });
  const any = $derived(choices.settings || choices.copyPresets.length > 0 || choices.mirrorPresets.length > 0);
</script>

<!-- A held Esc repeats: only the first press counts. -->
<svelte:window onkeydown={(e) => e.key === "Escape" && !e.repeat && onBack()} />

{#snippet presets(title: string, list: PresetImport[], state: Row[])}
  {#if list.length > 0}
    <Section {title}>
      <ul class="items">
        {#each list as p, i (i)}
          <li>
            <Checkbox label={p.name} checked={state[i].on} disabled={!!p.problem} onChange={(on) => (state[i].on = on)} />
            {#each p.paths as path (path)}<p class="mono path">{path}</p>{/each}
            {#if p.problem}<p class="problem">{p.problem}</p>{/if}
            {#each p.missing as path (path)}<p class="muted">{path} isn't connected now.</p>{/each}
            {#if p.clash && !p.problem}
              <RadioGroup
                legend="You already have “{p.clash}”"
                options={[
                  { value: false, label: `Keep both, as “${p.newName}”` },
                  { value: true, label: "Replace yours" },
                ]}
                value={state[i].replace}
                onChange={(v) => (state[i].replace = v)}
              />
            {/if}
          </li>
        {/each}
      </ul>
    </Section>
  {/if}
{/snippet}

<AppShell>
  {#snippet header()}<ScreenHeader title="Import" />{/snippet}

  <p class="file mono">{view.fileName}</p>

  {#if view.settings}
    <Section title="Settings">
      {#if view.settings.problem}
        <p class="problem">{view.settings.problem}</p>
      {:else if view.settings.changes.length === 0}
        <p class="muted">Same as yours</p>
      {:else}
        <Checkbox label="Import the settings" checked={settingsOn} onChange={(on) => (settingsOn = on)} />
        <ul class="changes">{#each view.settings.changes as c (c)}<li>{c}</li>{/each}</ul>
      {/if}
    </Section>
  {/if}
  {@render presets("Copy presets", view.copyPresets, copy)}
  {@render presets("Mirror presets", view.mirrorPresets, mirrors)}

  {#snippet actions()}
    <ActionBar>
      {#snippet start()}<Button icon="chevron-left" onclick={onBack}>Back</Button>{/snippet}
      {#snippet end()}<Button variant="primary" disabled={!any} onclick={() => onImport(choices)}>Import</Button>{/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .file { color: var(--text-muted); margin: 0 0 var(--space-2); }
  .items, .changes { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-3); }
  .changes { gap: var(--space-1); margin-top: var(--space-2); }
  .path { margin: var(--space-1) 0 0 var(--space-6); color: var(--text-muted); }
  .muted { color: var(--text-muted); margin: var(--space-1) 0 0 var(--space-6); }
  .problem { color: var(--danger); margin: var(--space-1) 0 0 var(--space-6); }
  .mono { font-family: var(--font-mono); }
</style>
```

Check `RadioGroup`'s generic value type accepts booleans; if its options must be strings, use `"keep"`/`"replace"` and map. Check the tokens used (`--space-6`, `--font-mono`) exist in the design tokens; use the existing ones (e.g. the `.mono` class other screens use).

`App.svelte`:
- `Screen` gains `"import"`; `back` covers where Import returns to (the screen it was opened from).
- state: `let importing: ImportView | null = $state(null);`
- functions:

```ts
  async function showImport(path: string) {
    const v = await run(() => api.openImport(path));
    if (!v) return;
    back = backFrom(screen); // however App records where Settings returns to; reuse that
    importing = v;
    screen = "import";
  }
  async function chooseImport() {
    const path = await api.pickImportFile();
    if (path) await showImport(path);
  }
  async function doImport(choices: ImportChoices) {
    const done = await run(() => api.applyImport(choices));
    if (!done) return;
    settings = done.settings;
    copyPresets = done.copyPresets;
    mirrorPresets = done.mirrorPresets;
    screen = back;
    importing = null;
    if (done.failed) error = done.message;
    else info = done.message;
  }
  async function openedFromFinder() {
    const path = await api.takeOpenedFile();
    if (path) await showImport(path);
  }
```

- `onMenu`: `else if (item === "import-file") void chooseImport();`
- `onMount`: after the start view loads, `void openedFromFinder();` and `const unlistenOpen = api.onOpenFile(() => void openedFromFinder());` (unlisten in the cleanup like the others).
- markup: `{:else if screen === "import" && importing}<ImportScreen view={importing} onImport={doImport} onBack={() => { importing = null; screen = back; }} />`
- Settings' `onImport={chooseImport}` (Task 6 added the prop).
- The tab bar is hidden on Import like on Settings (add `"import"` wherever Settings is excluded from `showTabs`).

- [ ] **Step 4: Run to verify they pass**

Run: `cd ui && npx vitest run 2>&1 | grep -E "Tests |×"` and `npm run check 2>&1 | grep COMPLETED`
Expected: all pass; 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src
git commit -F - <<'EOF'
feat(ui): the Import screen

Shows what a .secopy file holds against what this Mac has: settings
that change, clashes (Keep both or Replace), paths not connected, and
presets that can't be imported. Nothing changes until Import.

Refs #77

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 8: Gallery, docs and checklist

**Files:**
- Modify: `ui/src/gallery/Gallery.svelte`, `ui/src/gallery/fake.ts`, `README.md`, `docs/rfd/0001-secopy.md`, `docs/design/design-system.md`, `docs/testing/macos-app-checklist.md`, `docs/superpowers/plans/2026-09-26-v1-roadmap.md`

- [ ] **Step 1: Gallery pages** `#import` (ImportScreen with a view that has a clash, a missing path, a problem and settings changes) and `#export` (ExportDialog over the setup screen), following how `#cancel` renders a dialog. Add both to the page list in `docs/design/design-system.md`.

- [ ] **Step 2: Screenshots** — `cd ui && npx vite --port 5199 --strictPort` in the background; `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --window-size=1000,640 --virtual-time-budget=3000 --screenshot=/tmp/import.png "http://localhost:5199/gallery.html#import"` and the same for `#export` → `/tmp/export.png`; stop vite. Read both and fix anything cramped or misaligned.

- [ ] **Step 3: Docs**
  - `README.md` features list: "**Export and import:** settings and presets in a `.secopy` file, for a new Mac or to share presets; importing shows what's inside first."
  - RFD: new requirements after FR-52 in a "5.x Export and import" table: FR-53 (export, S), FR-54 (import with preview, clashes, never overwrite silently, S), FR-55 (open from Finder, C); a decision-log row: `| 2026-09-29 | **Export and import** (#77): one .secopy file for settings and presets, for backups and sharing; clashes Keep both by default or Replace; paths not on this Mac are a note; nothing imported while a job runs. |`
  - Design system: `ExportDialog` and `ImportScreen` in the components/screens list; the Settings screen's new section.
  - Checklist: items for export all / one preset, import with a clash (Keep both, Replace), a file with a missing path, double-click a `.secopy` with Secopy closed and open, and File › Import… greyed out while copying.
  - Roadmap: a row "Export and import (#77)".

- [ ] **Step 4: Run every check**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -q -- -D warnings && cargo test --workspace 2>&1 | grep -E "test result" | awk '{p+=$4; f+=$6} END {print p, f}' && cd ui && npx vitest run 2>&1 | grep "Tests " && npm run check 2>&1 | grep COMPLETED`
Expected: 0 failures, 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/gallery README.md docs
git commit -F - <<'EOF'
docs: export and import in the RFD, README, design system and checklist

Refs #77

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
EOF
```

- [ ] **Step 6: Build and check the document type** — `cd ui && npm run tauri build -- --bundles app`, then `plutil -p ../target/release/bundle/macos/Secopy.app/Contents/Info.plist | grep -A12 CFBundleDocumentTypes` shows the `secopy` extension. Install only after the build succeeds.
