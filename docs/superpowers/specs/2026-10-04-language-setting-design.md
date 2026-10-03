# Language setting — design

Approved in conversation on 2026-10-04. Builds on #84 (translation) and #175 (Spanish).

## Goal

The user chooses Secopy's language in Settings, whatever the Mac's language is (a Mac in
Greek, Secopy in Italian). By default, Automatic: the Mac's first language Secopy has, English
otherwise.

## Data safety

Nothing here touches copying, verifying or files. The rules that keep Settings safe:

- A missing, unknown or removed language in `settings.json` reads as Automatic; it never stops
  the settings from loading, and the other settings are kept.
- Saving the language is part of saving Settings (one write, as today). Telling macOS the
  language (below) happens after the save; if that fails, the setting is still saved and
  Secopy's own words still change.
- A job can't run while Settings is open, so a language change never interrupts one.

## What the user sees

- **Settings › General › Language**: a menu. First **Automatic (<name>)**, naming the language
  Automatic gives now (e.g. "Automatic (Español)"), then each language in its own name:
  **English**, **Español** (more as catalogs are added). Default: Automatic.
- Help: "Automatic uses the Mac's language when Secopy has it, English otherwise. macOS's own
  windows (Open, Save, About) change at the next launch." (Spanish in `es.json`.)
- **On Save**, Secopy's words change at once: the window, the native menus, the menu bar icon
  and panel, notifications.
- **macOS's own windows** (Open/Save panels, About, Services) follow from the next launch: macOS
  fixes their language when the app starts.
- Reports and the CLI stay English.

## How it works

- **Setting:** `Settings::language: Option<String>`, saved as `"language"`: `null` is
  Automatic, otherwise a catalog tag (`"en"`, `"es"`). Read leniently: a tag Secopy has no
  catalog for is Automatic.
- **The Mac's languages** come from the global preference (`NSGlobalDomain`'s
  `AppleLanguages`), not from the app's own preferred languages: those carry Secopy's choice
  once it has told macOS, so Automatic must look past it.
- **The language Secopy uses** = the chosen tag, or `language_for(the Mac's languages)` when
  Automatic. Rust keeps it (no longer fixed at start); it's set from the settings at launch
  and again when Settings are saved.
- **Telling macOS:** a chosen language is written as Secopy's own `AppleLanguages` (what System
  Settings' per-app language writes), so macOS's windows follow at the next launch; Automatic
  removes it. Secopy's setting is the one place the language is chosen: a per-app language set
  in System Settings is replaced the next time Secopy's Settings are saved.
- **Rust's words:** `Message::text()` uses the language in use; the native menus are rebuilt
  when it changes; the menu bar icon's title is redrawn as it updates.
- **The UI:** a command `app_language()` gives the language in use; the main window and the
  menu bar panel set it before they first draw. When Settings change the language, Rust tells
  both windows (an event) and each reloads, drawing everything in the new language (the state
  lives in Rust; Settings is closed by then).
- **Language names:** each catalog has `language.name`, its own name ("English", "Español").
- **Export/import:** `language` travels with the settings. Import shows "Language: Automatic
  → Español". An older Secopy lists it as a setting it doesn't know (#149) and keeps its own.

## Tests

- Choosing: Automatic picks the Mac's first language with a catalog, English otherwise; a
  chosen tag wins; an unknown tag, a missing key or `null` is Automatic.
- Saving a new language changes `Message::text()`, rebuilds the menus, emits the event; saving
  the same language does none of that.
- Settings load with an unknown language as Automatic and keep the rest.
- Export/import round trip; the import change line; an older file without the key → Automatic.
- UI: the Language menu (Automatic named, each language in its own name), Save sends the tag
  (or `null`), the start sets the language before drawing; English and Spanish texts.

## Not now

Per-language reports; a CLI language option; translating the user guide.
