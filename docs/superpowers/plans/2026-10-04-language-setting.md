# Language Setting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Settings › General › Language chooses Secopy's language (Automatic by default: the Mac's when Secopy has it, English otherwise).

**Architecture:** `Settings::language: Option<String>` (None = Automatic). Rust keeps the language in use in `message.rs` (set at launch and on save), reads the Mac's languages from `NSGlobalDomain`, tells macOS the choice through the app's `AppleLanguages`, rebuilds the native menus and emits an event; the UI asks Rust for the language before drawing and reloads on the event.

**Tech Stack:** Rust (secopy-app, Tauri 2, objc2-foundation), Svelte 5 UI.

**Spec:** `docs/superpowers/specs/2026-10-04-language-setting-design.md`

## Global Constraints

- Default Automatic: `language_for(the Mac's languages)`; a chosen tag wins; an unknown tag, a missing key or `null` is Automatic.
- The Mac's languages come from `NSGlobalDomain`'s `AppleLanguages`, never from the app's own preferred languages.
- A failed `AppleLanguages` write never fails Settings' save.
- Every UI text in `en.json` **and** `es.json`; bindings via `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app`.
- Commits: Conventional Commits, `(#181)` (the issue opened at start); noreply author and committer exported in the same shell; gate on `bash /tmp/i18n/checks.sh`.

## Review Focus

1. A `settings.json` with `"language": "fr"` (no French catalog): loads, Automatic, every other setting kept. Pinned in Task 1 (`an_unknown_language_is_automatic`).
2. Automatic after a chosen language, without relaunching: the Mac's language comes back at once (the app's `AppleLanguages` doesn't leak in). Pinned in Task 1 (`automatic_looks_past_secopys_own_choice`, on the pure `resolve`).
3. Saving Settings with the same language: no reload, no menu rebuild. Pinned in Task 2 (`saving_the_same_language_changes_nothing`).
4. The File menu's greyed items after the menus are rebuilt: the last state is applied again. Pinned in Task 2 (by hand in the app: rebuild with a job set up, Start still enabled/disabled as before).
5. Import of a file without `language`: Automatic, listed as defaulted; an older Secopy importing ours lists `language` as unknown. Pinned in Task 3.

---

### Task 1: The setting and choosing the language

**Files:** `crates/secopy-app/src/store.rs`, `crates/secopy-app/src/message.rs`.

**Interfaces (produces):**
```rust
// store.rs
pub struct Settings { /* … */ pub language: Option<String> }   // serde "language"; Default None
// message.rs
pub fn resolve(choice: Option<&str>, mac: &[String]) -> &'static str; // chosen tag if a catalog, else language_for(mac)
pub fn set_language(choice: Option<&str>);                         // resolve(choice, &mac_languages()) → in use
pub fn language_in_use() -> &'static str;
pub fn mac_languages() -> Vec<String>;                             // NSGlobalDomain AppleLanguages
pub fn tell_macos(choice: Option<&str>);                           // app AppleLanguages = [tag], or removed
```

- [ ] **Step 1: Failing tests.** `store.rs`: `an_unknown_language_is_automatic` (`{"language":"fr","writeMhl":true}` → `language == None`, `write_mhl == true`); `a_language_round_trips` (`Some("es")` saved and loaded). `message.rs`: `resolve_picks_the_choice_then_the_mac` (`resolve(Some("es"), &["en-US"]) == "es"`, `resolve(None, &["es-ES"]) == "es"`, `resolve(None, &["el-GR"]) == "en"`, `resolve(Some("fr"), &["es-ES"]) == "es"`); `automatic_looks_past_secopys_own_choice` (`resolve(None, &["de-DE","es-ES"]) == "es"` while the language in use was "en" — `resolve` takes the Mac's list only); `text_follows_the_language_in_use` (`set_language(Some("es"))` → `msg!("menu.file.start").text() == "Empezar"`; `set_language(Some("en"))` → `"Start"`; restore at the end). Run `cargo test -p secopy-app --lib store:: message::` — Expected: compile errors (missing field/functions).
- [ ] **Step 2: Implement.** `SettingsOnDisk.language: Option<String>` with `#[serde(default)]`; in `Deserialize for Settings` keep it only if `CATALOGS` has the tag. `message.rs`: replace the `LazyLock` in `app_language()` with `static IN_USE: RwLock<Option<&'static str>>`; `language_in_use()` returns it or, when unset, `language_for(&mac_languages())`; `text()` uses `language_in_use()`. `mac_languages()`: `NSUserDefaults::standardUserDefaults().persistentDomainForName(ns_string!("NSGlobalDomain"))` → `AppleLanguages` array of strings (empty on any failure). `tell_macos`: `standardUserDefaults().setObject_forKey(NSArray of [tag], "AppleLanguages")` or `removeObjectForKey("AppleLanguages")`. Run the tests — Expected: pass.
- [ ] **Step 3: Commit** `feat(app): a language setting, Automatic by default (#181)`.

### Task 2: Applying it (launch, save, menus, UI event)

**Files:** `crates/secopy-app/src/commands.rs` (`AppState::set_settings`, the `set_settings` command, new `app_language` command, `set_menu_state`), `crates/secopy-app/src/lib.rs` (setup, `FileMenu` state, `LANGUAGE_CHANGED` event, `invoke_handler`), bindings.

**Interfaces:** `AppState::set_settings(&self, Settings) -> Result<(Settings, bool), Message>` (true when the language changed); command `app_language() -> String`; event `"language-changed"`; state `Mutex<MenuState>` with the last `FileMenu` and its last `(setup, can_start, copying, busy)`.

- [ ] **Step 1: Failing tests** (commands.rs): `saving_a_new_language_switches_rusts_words` (state `set_settings` with `language: Some("es")` → `changed == true`, `msg!("menu.file.start").text() == "Empezar"`); `saving_the_same_language_changes_nothing` (again with `Some("es")` → `changed == false`); restore `None` at the end. Run — Expected: compile error (tuple).
- [ ] **Step 2: Implement.** `AppState::set_settings` compares `current.language != settings.language` before saving, calls `message::set_language` after the save, returns the flag. The command: when changed, `message::tell_macos(lang)` (errors only logged), rebuild with `menu(&app)` + `app.set_menu`, `FileMenu::find` into the `Mutex<MenuState>` and re-apply the last flags, `app.emit("language-changed", ())`. Setup: after `AppState` is managed, `message::set_language(settings.language)`, rebuild the menu the same way (the builder's menu used the Mac's language). `set_menu_state` stores the flags and updates the current `FileMenu`. Add `app_language` (returns `language_in_use()`) to the specta builder; regenerate bindings. Run `cargo test -p secopy-app` — Expected: pass.
- [ ] **Step 3: Commit** `feat(app): the language applies at launch and on save (#181)`.

### Task 3: Export and import

**Files:** `crates/secopy-app/src/transfer.rs`.

- [ ] **Step 1: Failing tests:** `the_language_travels` (export with `Some("es")`, import → `Some("es")`; the change line reads `Language: Automatic → Español`); `a_file_without_language_is_automatic` (listed as defaulted). Expected: fail.
- [ ] **Step 2: Implement:** `setting_label("language") => msg!("import.setting.language")`; the change line's from/to: `msg!("import.automatic")` for None, else `Message::raw(<that catalog's language.name>)`. Run `cargo test -p secopy-app --lib transfer::` — Expected: pass.
- [ ] **Step 3: Commit** `feat(app): the language travels with the settings (#181)`.

### Task 4: The UI

**Files:** `ui/src/main.ts`, `ui/src/lib/i18n.ts`, `ui/src/lib/api.ts`, `ui/src/test/fake-api.ts`, `ui/src/gallery/fake.ts`, `ui/src/components/SettingsScreen.svelte` (+ test), `ui/src/locales/{en,es}.json`.

**Interfaces:** `i18n.languages(): { tag: string; name: string }[]` (each catalog's `language.name`); `api.appLanguage(): Promise<string>`; `api.onLanguageChanged(cb): Promise<() => void>`.

- [ ] **Step 1: Failing tests:** SettingsScreen: the Language menu lists "Automatic (English)", "English", "Español" (in that order, Automatic naming `locale()`'s name); choosing Español and Save calls `setSettings` with `language: "es"`; Automatic sends `null`. i18n: `languages()` returns `[{tag:"en",name:"English"},{tag:"es",name:"Español"}]`. Expected: fail.
- [ ] **Step 2: Implement.** Catalog keys: `language.name` ("English" / "Español"), `settings.language.label` ("Language" / "Idioma"), `settings.language.automatic` ("Automatic ({name})" / "Automático ({name})"), `settings.language.help` (spec text / Spanish), `import.setting.language` ("Language" / "Idioma"), `import.automatic` ("Automatic" / "Automático"). SettingsScreen: a `<select>` in General (the existing select styling), first option value `""` = Automatic. `main.ts`: `setLocale(await api.appLanguage())` (fallback: keep the default on error) before `mount`; `api.onLanguageChanged(() => location.reload())` for both the window and the panel. Fakes return `"en"`. Run `cd ui && npx vitest run` — Expected: pass.
- [ ] **Step 3: Look:** gallery `#settings` in English and Spanish (headless Chrome, as before).
- [ ] **Step 4: Commit** `feat(ui): Settings › Language (#181)`.

### Task 5: By hand and docs

- [ ] Build the app (`cd ui && npm run tauri -- build --bundles app`) and hand the user this check (menus and macOS panels can only be checked in the real app): Settings › Language › Español → Save: window, menus, menu bar icon in Spanish at once; relaunch: the Open panel in Spanish; back to Automatic → the Mac's language at once, and after relaunch the panels too.
- [ ] Docs: user guide (Settings › General › Language), RFD (Settings table + decision-log row), `docs/i18n.md` (replace "Secopy has no language setting of its own" with the setting and how Automatic works).
- [ ] Commit `docs: the language setting (#181)`.
