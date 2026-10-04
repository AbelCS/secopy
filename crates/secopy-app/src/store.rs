//! What the app keeps between launches (plan 3b-1): settings, copy presets and remembered
//! state, as small versioned JSON files in the app's data folder.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use secopy_core::ignore::Patterns;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::dto::ExtensionKey;
use crate::message::Message;
use crate::msg;

/// The newest file version this Secopy reads; it writes 1 unless a file needs more.
const VERSION: u32 = 2;
pub const SETTINGS: &str = "settings.json";
/// Copy presets. Their file kept its name from when they were called profiles (#72).
pub const COPY_PRESETS: &str = "profiles.json";
pub const REMEMBERED: &str = "state.json";
/// Recent destinations kept (spec B7).
pub const RECENT: usize = 5;

/// The Settings screen (RFD §5.5).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub write_checksum_file: bool,
    /// "Show the count of ignored files"; named from before the ignore list (#158), kept
    /// as it is in `settings.json`.
    pub show_system_count: bool,
    pub report_next_to_checksum: bool,
    /// A notification when a copy ends while the window isn't in front (3b-2).
    pub notify_when_done: bool,
    /// Closing the window during a job hides it, with a menu bar icon (#80).
    pub keep_in_menu_bar: bool,
    /// Each copy also writes an ASC MHL history (#154).
    pub write_mhl: bool,
    /// Names never copied or mirrored (#158), checked (`ignore::Patterns`).
    pub ignore: Vec<String>,
    /// Secopy's language (#181): a catalog's tag, or `None` for Automatic (the Mac's).
    pub language: Option<String>,
}

/// `settings.json` as read: missing fields take their defaults. Kept apart from
/// [`Settings`] so the UI's type has every field.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsOnDisk {
    #[serde(default = "yes")]
    write_checksum_file: bool,
    #[serde(default = "yes")]
    show_system_count: bool,
    #[serde(default)]
    report_next_to_checksum: bool,
    #[serde(default = "yes")]
    notify_when_done: bool,
    #[serde(default = "yes")]
    keep_in_menu_bar: bool,
    #[serde(default)]
    write_mhl: bool,
    /// Read leniently: a bad pattern is dropped, never the settings.
    #[serde(default = "default_ignore")]
    ignore: Vec<String>,
    /// Read leniently: a language Secopy has no catalog for is Automatic.
    #[serde(default)]
    language: Option<String>,
}

fn default_ignore() -> Vec<String> {
    Patterns::defaults().as_slice().to_vec()
}

fn yes() -> bool {
    true
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = SettingsOnDisk::deserialize(d)?;
        Ok(Self {
            write_checksum_file: s.write_checksum_file,
            show_system_count: s.show_system_count,
            report_next_to_checksum: s.report_next_to_checksum,
            notify_when_done: s.notify_when_done,
            keep_in_menu_bar: s.keep_in_menu_bar,
            write_mhl: s.write_mhl,
            ignore: Patterns::lenient(s.ignore).as_slice().to_vec(),
            language: s
                .language
                .filter(|tag| crate::message::CATALOGS.iter().any(|(t, _)| t == tag)),
        })
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            write_checksum_file: true,
            show_system_count: true,
            report_next_to_checksum: false,
            notify_when_done: true,
            keep_in_menu_bar: true,
            write_mhl: false,
            ignore: default_ignore(),
            language: None,
        }
    }
}

impl Settings {
    /// The ignore list, checked.
    pub fn patterns(&self) -> Patterns {
        Patterns::lenient(self.ignore.clone())
    }
}

/// A saved copy setup for FROM (FR-38): choosing it loads its source and settings.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CopyPreset {
    /// Stays the same when the preset is renamed.
    pub id: String,
    pub name: String,
    /// The directory it loads, as a full path; empty until one is saved into it.
    pub source: String,
    /// "Include the folder" (FR-4).
    pub include_folder: bool,
    /// `None` = every file type, including ones never seen.
    pub extensions: Option<Vec<ExtensionKey>>,
    /// Also ignore (#164): patterns on top of Settings' list, for this preset. Left out when
    /// empty: the file is as an older Secopy wrote it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub ignore: Vec<String>,
}

impl CopyPreset {
    /// Whether the preset copies files of type `key`.
    pub fn selects(&self, key: &ExtensionKey) -> bool {
        self.extensions
            .as_ref()
            .is_none_or(|keys| keys.contains(key))
    }
}

/// A copy preset in `profiles.json` as read. Presets saved before 0.6 kept a path on the card
/// (`folder`) instead of a source; they load with no source.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopyPresetOnDisk {
    id: String,
    name: String,
    #[serde(default)]
    source: String,
    include_folder: bool,
    extensions: Option<Vec<ExtensionKey>>,
    #[serde(default, deserialize_with = "lenient_patterns")]
    ignore: Vec<String>,
}

/// A saved list of patterns: a bad one is dropped, never the preset (#164).
fn lenient_patterns<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list = Vec::<String>::deserialize(d)?;
    Ok(Patterns::lenient(list).as_slice().to_vec())
}

/// A typed list (a preset editor, an import): checked, the first bad pattern said with why.
pub fn checked_list(list: &[String]) -> Result<Vec<String>, Message> {
    for p in list {
        if let Err(e) = Patterns::new([p.clone()]) {
            return Err(msg!(
                "errors.field.ignore",
                pattern = p.trim(),
                why = pattern_why(e)
            ));
        }
    }
    Patterns::new(list.iter().cloned())
        .map(|p| p.as_slice().to_vec())
        .map_err(|e| msg!("errors.field.ignore", pattern = "", why = pattern_why(e)))
}

/// Why a pattern is refused (#158).
pub fn pattern_why(e: secopy_core::ignore::PatternError) -> Message {
    use secopy_core::ignore::{MAX_LEN, MAX_PATTERNS, PatternError};
    match e {
        PatternError::HasSlash => msg!("errors.pattern.slash"),
        PatternError::TooLong => msg!("errors.pattern.tooLong", max = MAX_LEN),
        PatternError::TooMany => msg!("errors.pattern.tooMany", max = MAX_PATTERNS),
        PatternError::BadChar => msg!("errors.pattern.badChar"),
    }
}

impl<'de> Deserialize<'de> for CopyPreset {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let p = CopyPresetOnDisk::deserialize(d)?;
        Ok(Self {
            id: p.id,
            name: p.name,
            source: p.source,
            include_folder: p.include_folder,
            extensions: p.extensions,
            ignore: p.ignore,
        })
    }
}

/// A copy preset as typed in a form (Save as…, the Copy presets screen).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CopyPresetInput {
    pub name: String,
    /// A full path, or empty.
    pub source: String,
    pub include_folder: bool,
    pub extensions: Option<Vec<ExtensionKey>>,
    /// Also ignore (#164); a preset written before has none. Left out when empty, so an
    /// older Secopy still imports a preset without one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CopyPresets {
    /// Saved under the key they had as profiles (#72).
    #[serde(rename = "profiles")]
    pub presets: Vec<CopyPreset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

/// What the app remembers by itself (FR-36). Never the destination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Remembered {
    /// Copy & Verify (true) or Copy.
    pub verify: bool,
    pub window: Option<WindowSize>,
    /// The copy preset last used; saved under its name from when presets were profiles (#72).
    #[serde(rename = "lastProfile")]
    pub last_preset: Option<String>,
    /// The last destinations a job started with, most recent first (B7).
    pub recent_destinations: Vec<String>,
}

impl Default for Remembered {
    fn default() -> Self {
        Self {
            verify: true,
            window: None,
            last_preset: None,
            recent_destinations: Vec::new(),
        }
    }
}

impl Remembered {
    /// Puts `dest` first, without duplicates, keeping the last [`RECENT`].
    pub fn used_destination(&mut self, dest: &str) {
        self.recent_destinations.retain(|d| d != dest);
        self.recent_destinations.insert(0, dest.to_string());
        self.recent_destinations.truncate(RECENT);
    }
}

impl CopyPresets {
    pub fn get(&self, id: &str) -> Option<&CopyPreset> {
        self.presets.iter().find(|p| p.id == id)
    }

    pub fn add(&mut self, input: CopyPresetInput) -> Result<CopyPreset, Message> {
        let input = self.check(input, None)?;
        let preset = CopyPreset {
            id: self.new_id(),
            name: input.name,
            source: input.source,
            include_folder: input.include_folder,
            extensions: input.extensions,
            ignore: input.ignore,
        };
        self.presets.push(preset.clone());
        Ok(preset)
    }

    pub fn edit(&mut self, id: &str, input: CopyPresetInput) -> Result<CopyPreset, Message> {
        let input = self.check(input, Some(id))?;
        let preset = self
            .presets
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| msg!("errors.preset.gone"))?;
        preset.name = input.name;
        preset.source = input.source;
        preset.include_folder = input.include_folder;
        preset.extensions = input.extensions;
        preset.ignore = input.ignore;
        Ok(preset.clone())
    }

    /// Stores `preset` over the one with its id (Update).
    pub fn replace(&mut self, preset: CopyPreset) {
        if let Some(old) = self.presets.iter_mut().find(|p| p.id == preset.id) {
            *old = preset;
        }
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| p.id != id);
        self.presets.len() < before
    }

    /// `input` checked and put right like a preset typed by hand, except for its name being
    /// taken: trimmed name, full-path source, file types in lowercase without dots.
    pub fn normalized(input: CopyPresetInput) -> Result<CopyPresetInput, Message> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(msg!("errors.field.name.presetEmpty"));
        }
        long_name(&name)?;
        Ok(CopyPresetInput {
            name,
            source: source(&input.source)?,
            include_folder: input.include_folder,
            extensions: extensions(input.extensions),
            ignore: checked_list(&input.ignore)?,
        })
    }

    /// The preset called `name`, in any letter case.
    pub fn named(&self, name: &str) -> Option<&CopyPreset> {
        let name = name.to_lowercase();
        self.presets.iter().find(|p| p.name.to_lowercase() == name)
    }

    /// Validates and normalizes `input`; `editing` is the id of the preset being edited.
    fn check(
        &self,
        input: CopyPresetInput,
        editing: Option<&str>,
    ) -> Result<CopyPresetInput, Message> {
        let input = Self::normalized(input)?;
        if self
            .named(&input.name)
            .is_some_and(|p| Some(p.id.as_str()) != editing)
        {
            return Err(msg!("errors.field.name.presetTaken", name = &input.name));
        }
        Ok(input)
    }

    fn new_id(&self) -> String {
        new_id(|id| self.get(id).is_some())
    }

    /// Presets read from `profiles.json`, put right the way new ones are (#69): names
    /// trimmed, file types in lowercase without dots, sources as full paths. One with no name,
    /// or whose name or id another has, gets a free one instead of being dropped; the changes
    /// the user would notice come back as messages.
    pub fn repaired(self) -> (Self, Vec<Message>) {
        let loaded_ids: Vec<String> = self.presets.iter().map(|p| p.id.clone()).collect();
        let mut out = Self::default();
        let mut notes = Vec::new();
        for p in self.presets {
            let trimmed = p.name.trim();
            let wanted = if trimmed.is_empty() {
                "Unnamed preset"
            } else {
                trimmed
            };
            let name = out.free_name(wanted);
            if trimmed.is_empty() {
                notes.push(msg!(
                    "presets.repaired.noName",
                    file = COPY_PRESETS,
                    name = &name
                ));
            } else if name != wanted {
                notes.push(msg!(
                    "presets.repaired.sameName",
                    file = COPY_PRESETS,
                    wanted = wanted,
                    name = &name,
                ));
            }
            let source = source(&p.source).unwrap_or_else(|_| {
                notes.push(msg!(
                    "presets.repaired.notFullPath",
                    name = &name,
                    source = p.source.trim(),
                ));
                String::new()
            });
            let id = if p.id.is_empty() || out.get(&p.id).is_some() {
                new_id(|id| loaded_ids.iter().any(|l| l == id) || out.get(id).is_some())
            } else {
                p.id
            };
            out.presets.push(CopyPreset {
                ignore: p.ignore.clone(),
                id,
                name,
                source,
                include_folder: p.include_folder,
                extensions: extensions(p.extensions),
            });
        }
        (out, notes)
    }

    /// `name`, or else `name (2)`, `name (3)`…: the first no preset has, in any letter case.
    pub fn free_name(&self, name: &str) -> String {
        free_name(name, |n| self.named(n).is_some())
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
            ignore: self.ignore.clone(),
        }
    }
}

/// Longest preset name, in characters.
const MAX_NAME: usize = 200;
/// A hundred years: longer can't be counted back from today.
const MAX_ARCHIVE_DAYS: u32 = 36_500;

fn long_name(name: &str) -> Result<(), Message> {
    if name.chars().count() > MAX_NAME {
        return Err(msg!("errors.field.name.tooLong", max = MAX_NAME));
    }
    Ok(())
}

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

/// A copy preset's file types: lowercase, without leading dots, sorted and without duplicates.
fn extensions(keys: Option<Vec<ExtensionKey>>) -> Option<Vec<ExtensionKey>> {
    keys.map(|keys| {
        let keys: BTreeSet<ExtensionKey> = keys
            .into_iter()
            .map(|key| {
                key.map(|e| e.trim().trim_start_matches('.').to_lowercase())
                    .filter(|e| !e.is_empty())
            })
            .collect();
        keys.into_iter().collect()
    })
}

/// A random id that `taken` doesn't know yet.
fn new_id(taken: impl Fn(&str) -> bool) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    loop {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        let id = format!(
            "{:x}",
            nanos ^ NEXT.fetch_add(1, Ordering::Relaxed).rotate_left(32)
        );
        if !taken(&id) {
            return id;
        }
    }
}

pub const MIRRORS: &str = "mirrors.json";

/// What a mirror does with files deleted in the origin (FR-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DeletedMode {
    Archive,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeletedFiles {
    pub mode: DeletedMode,
    /// Days archived files are kept (archive mode).
    pub days: u32,
}

impl From<&DeletedFiles> for secopy_core::mirror::Deleted {
    fn from(d: &DeletedFiles) -> Self {
        match d.mode {
            DeletedMode::Archive => Self::Archive,
            DeletedMode::Delete => Self::Delete,
        }
    }
}

/// A saved one-way mirror (plan 7, FR-44).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorPreset {
    pub id: String,
    pub name: String,
    pub origin: String,
    pub destination: String,
    pub deleted: DeletedFiles,
    /// Also compare contents by checksum (FR-46).
    pub deep_check: bool,
    /// "Delete it at the next run" (#101): the destination whose archive the next run deletes
    /// first. Kept with its path, so a later change of destination never points it elsewhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_archive: Option<String>,
    /// Also ignore (#164): patterns on top of Settings' list, for this mirror.
    #[serde(
        default,
        deserialize_with = "lenient_patterns",
        skip_serializing_if = "Vec::is_empty"
    )]
    #[specta(type = Vec<String>, optional)]
    pub ignore: Vec<String>,
}

/// A mirror preset as typed in its editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorPresetInput {
    pub name: String,
    pub origin: String,
    pub destination: String,
    pub deleted: DeletedFiles,
    pub deep_check: bool,
    /// Also ignore (#164); a preset written before has none. Left out when empty, so an
    /// older Secopy still imports a preset without one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MirrorPresets {
    pub presets: Vec<MirrorPreset>,
}

impl MirrorPresets {
    pub fn get(&self, id: &str) -> Option<&MirrorPreset> {
        self.presets.iter().find(|p| p.id == id)
    }

    pub fn add(&mut self, input: MirrorPresetInput) -> Result<MirrorPreset, Message> {
        let input = self.check(input, None)?;
        let preset = MirrorPreset {
            id: new_id(|id| self.get(id).is_some()),
            name: input.name,
            origin: input.origin,
            destination: input.destination,
            deleted: input.deleted,
            deep_check: input.deep_check,
            clear_archive: None,
            ignore: input.ignore,
        };
        self.presets.push(preset.clone());
        Ok(preset)
    }

    pub fn edit(&mut self, id: &str, input: MirrorPresetInput) -> Result<MirrorPreset, Message> {
        let input = self.check(input, Some(id))?;
        let preset = self
            .presets
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| msg!("errors.mirror.gone"))?;
        preset.name = input.name;
        preset.origin = input.origin;
        // A pending archive deletion belongs to the destination it was asked for (#101), and
        // to Delete mode: back to Archive, the archive is kept (#113).
        if preset.destination != input.destination || input.deleted.mode == DeletedMode::Archive {
            preset.clear_archive = None;
        }
        preset.destination = input.destination;
        preset.deleted = input.deleted;
        preset.deep_check = input.deep_check;
        preset.ignore = input.ignore;
        Ok(preset.clone())
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| p.id != id);
        self.presets.len() < before
    }

    /// The mirror called `name`, in any letter case.
    pub fn named(&self, name: &str) -> Option<&MirrorPreset> {
        let name = name.to_lowercase();
        self.presets.iter().find(|p| p.name.to_lowercase() == name)
    }

    /// `name`, or else `name (2)`, `name (3)`…: the first no mirror has, in any letter case.
    pub fn free_name(&self, name: &str) -> String {
        free_name(name, |n| self.named(n).is_some())
    }

    /// Validates and normalizes `input`; `editing` is the id of the preset being edited.
    fn check(
        &self,
        input: MirrorPresetInput,
        editing: Option<&str>,
    ) -> Result<MirrorPresetInput, Message> {
        let input = Self::normalized(input)?;
        if self
            .named(&input.name)
            .is_some_and(|p| Some(p.id.as_str()) != editing)
        {
            return Err(msg!("errors.field.name.mirrorTaken", name = &input.name));
        }
        Ok(input)
    }

    /// `input` checked and put right like a mirror typed by hand, except for its name being
    /// taken.
    pub fn normalized(input: MirrorPresetInput) -> Result<MirrorPresetInput, Message> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(msg!("errors.field.name.mirrorEmpty"));
        }
        long_name(&name)?;
        let origin = full_path(&input.origin).ok_or_else(|| msg!("errors.field.origin.notFull"))?;
        let destination = full_path(&input.destination)
            .ok_or_else(|| msg!("errors.field.destination.notFull"))?;
        let (o, d) = (Path::new(&origin), Path::new(&destination));
        if o == d {
            return Err(msg!("errors.field.origin.same"));
        }
        if d.starts_with(o) {
            return Err(msg!("errors.field.destination.inOrigin"));
        }
        if o.starts_with(d) {
            return Err(msg!("errors.field.origin.inDestination"));
        }
        if input.deleted.days == 0 {
            return Err(msg!("errors.field.days.tooFew"));
        }
        if input.deleted.days > MAX_ARCHIVE_DAYS {
            return Err(msg!("errors.field.days.tooMany", max = MAX_ARCHIVE_DAYS));
        }
        Ok(MirrorPresetInput {
            name,
            origin,
            destination,
            ignore: checked_list(&input.ignore)?,
            ..input
        })
    }
}

impl MirrorPreset {
    /// The mirror as its editor would hold it: everything but its id.
    pub fn input(&self) -> MirrorPresetInput {
        MirrorPresetInput {
            name: self.name.clone(),
            origin: self.origin.clone(),
            destination: self.destination.clone(),
            deleted: self.deleted,
            deep_check: self.deep_check,
            ignore: self.ignore.clone(),
        }
    }
}

/// A preset's origin or destination: a full path without a trailing `/`; `None` if it isn't one.
fn full_path(text: &str) -> Option<String> {
    let text = text.trim();
    if !text.starts_with('/') {
        return None;
    }
    let trimmed = text.trim_end_matches('/');
    Some(if trimmed.is_empty() { "/" } else { trimmed }.to_string())
}

/// A copy preset's source, normalized: a full path without a trailing `/`, or empty.
fn source(text: &str) -> Result<String, Message> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(String::new());
    }
    full_path(text).ok_or_else(|| msg!("errors.field.source.notFull"))
}

/// The version a file needs: 2 when a preset has its own ignore list (#164), so an older
/// Secopy, which would drop the list (and a mirror could then delete what it protects),
/// refuses the file instead; 1 otherwise, as every Secopy before wrote it.
fn version_needed(value: &serde_json::Value) -> u32 {
    let lists = ["presets", "profiles"]
        .into_iter()
        .filter_map(|key| value.get(key)?.as_array())
        .flatten()
        .any(|p| {
            p.get("ignore")
                .and_then(|i| i.as_array())
                .is_some_and(|i| !i.is_empty())
        });
    if lists { 2 } else { 1 }
}

/// On disk: the data next to a version number.
#[derive(Serialize, Deserialize)]
struct Versioned<T> {
    version: u32,
    #[serde(flatten)]
    data: T,
}

/// Just the version, to tell a newer file from a damaged one.
#[derive(Deserialize)]
struct VersionOnly {
    version: u32,
}

/// The app's data folder.
pub struct Store {
    dir: PathBuf,
    /// One save at a time: saves share the temp-file name.
    saving: Mutex<()>,
    /// Files left as they are and never saved over this session: a newer Secopy wrote them
    /// (#137), or they couldn't be read (#192).
    kept: Mutex<std::collections::HashMap<String, Kept>>,
}

#[derive(Debug, Clone, Copy)]
enum Kept {
    Newer,
    Unread,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            saving: Mutex::new(()),
            kept: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Reads `name`: a missing file gives the defaults; a damaged one is set aside and the
    /// defaults are used, with a message saying so. One that can't be read just now
    /// (permissions, a disk error) is left as it is, and never saved over this session.
    pub fn load<T: Default + DeserializeOwned>(&self, name: &str) -> (T, Option<Message>) {
        let text = match fs::read_to_string(self.dir.join(name)) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (T::default(), None),
            Err(e) => {
                crate::lock(&self.kept).insert(name.to_string(), Kept::Unread);
                let why = crate::say::io_error(&e);
                return (
                    T::default(),
                    Some(msg!("app.warning.unreadKept", file = name, why = why)),
                );
            }
        };
        let why = match serde_json::from_str::<VersionOnly>(&text) {
            // An older Secopy opened after a newer one: its file stays as it is, so the newer
            // one finds it again, and nothing is saved over it this session (#137).
            Ok(v) if v.version > VERSION => {
                crate::lock(&self.kept).insert(name.to_string(), Kept::Newer);
                return (
                    T::default(),
                    Some(msg!(
                        "app.warning.newerKept",
                        file = name,
                        version = v.version
                    )),
                );
            }
            _ => match serde_json::from_str::<Versioned<T>>(&text) {
                Ok(v) => return (v.data, None),
                Err(e) => msg!("app.warning.damaged", detail = e.to_string()),
            },
        };
        (T::default(), Some(self.set_aside(name, why)))
    }

    /// Renames a file that can't be read to `<name>.damaged-<time>`; the message for the UI.
    fn set_aside(&self, name: &str, why: Message) -> Message {
        let aside = format!(
            "{name}.damaged-{}",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        match fs::rename(self.dir.join(name), self.dir.join(&aside)) {
            Ok(()) => msg!(
                "app.warning.setAside",
                file = name,
                why = why,
                aside = aside,
                dir = &self.dir,
            ),
            Err(e) => msg!(
                "app.warning.notSetAside",
                file = name,
                why = why,
                error = crate::say::io_error(&e),
            ),
        }
    }

    /// Writes `name` through a temp file and a rename, so a failed save never leaves half a
    /// file.
    pub fn save<T: Serialize>(&self, name: &str, data: &T) -> Result<(), Message> {
        match crate::lock(&self.kept).get(name) {
            Some(Kept::Newer) => return Err(msg!("errors.save.newer", file = name)),
            Some(Kept::Unread) => return Err(msg!("errors.save.unread", file = name)),
            None => {}
        }
        let _one_at_a_time = crate::lock(&self.saving);
        let path = self.dir.join(name);
        let failed = |e: io::Error| {
            msg!(
                "errors.save.file",
                path = &path,
                why = crate::say::io_error(&e)
            )
        };
        fs::create_dir_all(&self.dir).map_err(failed)?;
        let value = serde_json::to_value(data).expect("saved data serializes");
        let text = serde_json::to_string_pretty(&Versioned {
            version: version_needed(&value),
            data: value,
        })
        .expect("saved data serializes");
        let tmp = self.dir.join(format!("{name}.tmp"));
        let written = (|| {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&tmp, &path)
        })();
        if written.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        written.map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, source: &str) -> CopyPresetInput {
        CopyPresetInput {
            ignore: Vec::new(),
            name: name.into(),
            source: source.into(),
            include_folder: true,
            extensions: Some(vec![
                Some("MP4".into()),
                Some(".xml".into()),
                Some("".into()),
            ]),
        }
    }

    /// #181 review focus 1: a language Secopy has no catalog for reads as Automatic, and
    /// the rest of the settings are kept.
    #[test]
    fn an_unknown_language_is_automatic() {
        let s: Settings = serde_json::from_str(r#"{"language":"fr","writeMhl":true}"#).unwrap();
        assert_eq!(s.language, None);
        assert!(s.write_mhl);
        let s: Settings = serde_json::from_str(r#"{"writeMhl":true}"#).unwrap();
        assert_eq!(s.language, None);
    }

    #[test]
    fn a_language_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("data"));
        let settings = Settings {
            language: Some("es".into()),
            ..Settings::default()
        };
        store.save(SETTINGS, &settings).unwrap();
        assert_eq!(store.load::<Settings>(SETTINGS), (settings, None));
    }

    #[test]
    fn missing_files_give_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("data"));
        assert_eq!(
            store.load::<Settings>(SETTINGS),
            (Settings::default(), None)
        );
        assert!(store.load::<Remembered>(REMEMBERED).0.verify);
        assert!(store.load::<CopyPresets>(COPY_PRESETS).0.presets.is_empty());
    }

    #[test]
    fn files_round_trip_with_a_version() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("data"));
        let settings = Settings {
            write_checksum_file: false,
            ..Settings::default()
        };
        store.save(SETTINGS, &settings).unwrap();
        assert_eq!(store.load::<Settings>(SETTINGS), (settings, None));
        let text = fs::read_to_string(dir.path().join("data").join(SETTINGS)).unwrap();
        assert!(text.contains("\"version\": 1") && text.contains("\"writeChecksumFile\": false"));
        let mut presets = CopyPresets::default();
        presets
            .add(input("Sony FX3", "/Volumes/CARD/PRIVATE/M4ROOT/CLIP"))
            .unwrap();
        store.save(COPY_PRESETS, &presets).unwrap();
        assert_eq!(store.load::<CopyPresets>(COPY_PRESETS), (presets, None));
        assert!(!dir.path().join("data").join("profiles.json.tmp").exists());
    }

    #[test]
    fn a_damaged_file_is_set_aside_and_the_defaults_are_used() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SETTINGS), b"{ not json").unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let (settings, warning) = store.load::<Settings>(SETTINGS);
        assert_eq!(settings, Settings::default());
        let warning = warning.unwrap();
        assert!(
            warning.starts_with("settings.json couldn’t be read (key must be a string"),
            "the parser’s own words, as before: {warning}"
        );
        assert!(!dir.path().join(SETTINGS).exists());
        let aside: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(aside.len(), 1);
        assert!(aside[0].starts_with("settings.json.damaged-"), "{aside:?}");
        assert!(warning.contains(&aside[0]));
    }

    /// QA review (#137): a file from a newer Secopy (an older one opened after it) stays as it
    /// is, not set aside: going back to the newer Secopy finds it. The defaults are used, and
    /// nothing is saved over it.
    #[test]
    fn a_file_from_a_newer_secopy_is_left_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let newer = br#"{"version": 3, "writeChecksumFile": false}"#;
        fs::write(dir.path().join(SETTINGS), newer).unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let (settings, warning) = store.load::<Settings>(SETTINGS);
        assert_eq!(settings, Settings::default());
        assert_eq!(warning.unwrap().key, "app.warning.newerKept");
        assert!(store.save(SETTINGS, &Settings::default()).is_err());
        assert_eq!(fs::read(dir.path().join(SETTINGS)).unwrap(), newer);
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "nothing set aside"
        );
    }

    /// Code review (#192): a file that can't be read just now (permissions, a disk error) is
    /// no damaged file: it stays as it is, not set aside, and nothing is saved over it, so the
    /// presets and the queue are there once it reads again.
    #[test]
    fn a_file_that_cant_be_read_is_left_as_it_is() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SETTINGS);
        let mine = br#"{"version": 1, "writeChecksumFile": false}"#;
        fs::write(&path, mine).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let (settings, warning) = store.load::<Settings>(SETTINGS);
        assert_eq!(settings, Settings::default());
        assert_eq!(warning.unwrap().key, "app.warning.unreadKept");
        let refused = store.save(SETTINGS, &Settings::default()).unwrap_err();
        assert_eq!(refused.key, "errors.save.unread");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(fs::read(&path).unwrap(), mine);
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "nothing set aside"
        );
    }

    #[test]
    fn unknown_and_missing_fields_are_fine() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SETTINGS),
            br#"{"version": 1, "showSystemCount": false, "later": 1}"#,
        )
        .unwrap();
        let (settings, warning) = Store::new(dir.path().to_path_buf()).load::<Settings>(SETTINGS);
        assert_eq!(warning, None);
        assert!(!settings.show_system_count && settings.write_checksum_file);
    }

    #[test]
    fn a_failed_save_keeps_the_old_file_and_leaves_no_temp_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        store.save(SETTINGS, &Settings::default()).unwrap();
        let before = fs::read(dir.path().join(SETTINGS)).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let result = store.save(
            SETTINGS,
            &Settings {
                show_system_count: false,
                ..Settings::default()
            },
        );
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        if result.is_ok() {
            return; // running as root: permissions are not enforced
        }
        assert_eq!(fs::read(dir.path().join(SETTINGS)).unwrap(), before);
        assert!(!dir.path().join("settings.json.tmp").exists());
    }

    #[test]
    fn copy_presets_are_checked_and_normalized() {
        let mut presets = CopyPresets::default();
        let p = presets
            .add(input("  Sony FX3 ", " /Volumes/CARD/PRIVATE/M4ROOT/CLIP/ "))
            .unwrap();
        assert_eq!(p.name, "Sony FX3");
        assert_eq!(p.source, "/Volumes/CARD/PRIVATE/M4ROOT/CLIP");
        assert_eq!(
            p.extensions,
            Some(vec![None, Some("mp4".into()), Some("xml".into())])
        );
        assert!(!p.id.is_empty());
        let err = |r: Result<CopyPreset, Message>| r.unwrap_err();
        assert_eq!(err(presets.add(input(" ", ""))), "The preset needs a name.");
        assert_eq!(
            err(presets.add(input("sony fx3", ""))),
            "There is already a preset called “sony fx3”."
        );
        assert_eq!(
            err(presets.add(input("A", "DCIM"))),
            "The source must be a full path, like /Volumes/Untitled/DCIM."
        );
        assert_eq!(
            presets.add(input("B", " ")).unwrap().source,
            "",
            "no source yet"
        );
        assert_eq!(presets.presets.len(), 2);
    }

    #[test]
    fn editing_keeps_the_id_and_deleting_removes() {
        let mut presets = CopyPresets::default();
        let a = presets.add(input("A", "/Volumes/CARD/DCIM")).unwrap();
        let b = presets.add(input("B", "")).unwrap();
        assert_ne!(a.id, b.id);
        let edited = presets
            .edit(&a.id, input("A2", "/Volumes/CARD/DCIM/100MSDCF"))
            .unwrap();
        assert_eq!(
            (edited.id.as_str(), edited.name.as_str()),
            (a.id.as_str(), "A2")
        );
        assert_eq!(
            presets.get(&a.id).unwrap().source,
            "/Volumes/CARD/DCIM/100MSDCF"
        );
        assert!(
            presets.edit(&a.id, input("b", "")).is_err(),
            "B’s name is taken"
        );
        assert!(
            presets.edit(&a.id, input("a2", "")).is_ok(),
            "its own name is fine"
        );
        assert!(presets.delete(&b.id));
        assert!(!presets.delete(&b.id));
        assert_eq!(presets.presets.len(), 1);
        let mut replaced = presets.get(&a.id).unwrap().clone();
        replaced.include_folder = false;
        presets.replace(replaced.clone());
        assert_eq!(presets.get(&a.id), Some(&replaced));
    }

    #[test]
    fn recent_destinations_are_the_last_five_without_duplicates() {
        let mut r = Remembered::default();
        for d in ["/a", "/b", "/c", "/d", "/e", "/f", "/c"] {
            r.used_destination(d);
        }
        assert_eq!(r.recent_destinations, ["/c", "/f", "/e", "/d", "/b"]);
    }

    #[test]
    fn a_preset_selects_its_file_types_or_all() {
        let mut p = CopyPreset {
            ignore: Vec::new(),
            id: "x".into(),
            name: "A".into(),
            source: String::new(),
            include_folder: true,
            extensions: Some(vec![Some("mp4".into())]),
        };
        assert!(p.selects(&Some("mp4".into())) && !p.selects(&Some("xml".into())));
        p.extensions = None;
        assert!(p.selects(&Some("anything".into())) && p.selects(&None));
    }

    #[test]
    fn notify_when_done_is_on_unless_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SETTINGS), br#"{"version": 1}"#).unwrap();
        let store = Store::new(dir.path().to_path_buf());
        assert!(store.load::<Settings>(SETTINGS).0.notify_when_done);
        let off = Settings {
            notify_when_done: false,
            ..Settings::default()
        };
        store.save(SETTINGS, &off).unwrap();
        assert!(!store.load::<Settings>(SETTINGS).0.notify_when_done);
    }

    /// 0.5 presets kept a path on the card; they load with no source, not as damaged.
    #[test]
    fn a_preset_saved_with_a_card_folder_loads_with_no_source() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(COPY_PRESETS),
            r#"{"version": 1, "profiles": [{"id": "a", "name": "FX3", "folder": "PRIVATE/M4ROOT/CLIP", "includeFolder": true, "extensions": null}]}"#,
        )
        .unwrap();
        let (presets, warning) =
            Store::new(dir.path().to_path_buf()).load::<CopyPresets>(COPY_PRESETS);
        assert_eq!(warning, None);
        assert_eq!(presets.presets[0].name, "FX3");
        assert_eq!(presets.presets[0].source, "");
    }
    fn mirror_input(name: &str, origin: &str, destination: &str) -> MirrorPresetInput {
        MirrorPresetInput {
            ignore: Vec::new(),
            name: name.into(),
            origin: origin.into(),
            destination: destination.into(),
            deleted: DeletedFiles {
                mode: DeletedMode::Archive,
                days: 30,
            },
            deep_check: false,
        }
    }

    #[test]
    fn mirror_presets_are_checked() {
        let mut m = MirrorPresets::default();
        let p = m
            .add(mirror_input(
                " Footage → NAS ",
                "/Volumes/SSD/Footage/",
                "/Volumes/Media/Footage",
            ))
            .unwrap();
        assert_eq!(
            (p.name.as_str(), p.origin.as_str()),
            ("Footage → NAS", "/Volumes/SSD/Footage")
        );
        let err = |r: Result<MirrorPreset, Message>| r.unwrap_err();
        assert_eq!(
            err(m.add(mirror_input("footage → nas", "/a", "/b"))),
            "There is already a mirror called “footage → nas”."
        );
        assert_eq!(
            err(m.add(mirror_input("A", "Footage", "/b"))),
            "The origin must be a full path, like /Volumes/SSD/Footage."
        );
        assert_eq!(
            err(m.add(mirror_input("B", "/x", "/x"))),
            "The origin and the destination are the same directory."
        );
        assert_eq!(
            err(m.add(mirror_input("C", "/x", "/x/backup"))),
            "The destination can’t be inside the origin."
        );
        assert_eq!(
            err(m.add(mirror_input("D", "/x/sub", "/x"))),
            "The origin can’t be inside the destination."
        );
        let mut zero = mirror_input("E", "/a", "/b");
        zero.deleted.days = 0;
        assert_eq!(err(m.add(zero)), "Keep archived files for at least 1 day.");
    }

    #[test]
    fn mirror_presets_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let mut m = MirrorPresets::default();
        m.add(mirror_input("N", "/a", "/b")).unwrap();
        store.save(MIRRORS, &m).unwrap();
        assert_eq!(store.load::<MirrorPresets>(MIRRORS), (m, None));
    }

    #[test]
    fn normalized_checks_a_preset_without_its_name_clashing() {
        let input = CopyPresetInput {
            ignore: Vec::new(),
            name: "  Sony FX3 ".into(),
            source: "/Volumes/CARD_A/CLIP/".into(),
            include_folder: true,
            extensions: Some(vec![Some(".MP4".into())]),
        };
        let n = CopyPresets::normalized(input).unwrap();
        assert_eq!(
            (n.name.as_str(), n.source.as_str()),
            ("Sony FX3", "/Volumes/CARD_A/CLIP")
        );
        assert_eq!(n.extensions, Some(vec![Some("mp4".into())]));
        let bad = CopyPresetInput {
            name: " ".into(),
            ..n.clone()
        };
        assert_eq!(
            CopyPresets::normalized(bad).unwrap_err(),
            "The preset needs a name."
        );
        let mirror = MirrorPresetInput {
            ignore: Vec::new(),
            name: "Footage".into(),
            origin: "/Volumes/SSD/Footage".into(),
            destination: "/Volumes/SSD/Footage/Backup".into(),
            deleted: DeletedFiles {
                mode: DeletedMode::Archive,
                days: 30,
            },
            deep_check: false,
        };
        assert_eq!(
            MirrorPresets::normalized(mirror).unwrap_err(),
            "The destination can’t be inside the origin."
        );
    }

    #[test]
    fn free_names_skip_taken_ones_in_any_case() {
        let mut presets = CopyPresets::default();
        presets.add(input("Sony FX3", "")).unwrap();
        presets.add(input("sony fx3 (2)", "")).unwrap();
        assert_eq!(presets.free_name("Sony FX3"), "Sony FX3 (3)");
        assert_eq!(presets.free_name("DJI"), "DJI");
        assert_eq!(
            presets.named("SONY FX3").map(|p| p.name.as_str()),
            Some("Sony FX3")
        );
    }

    #[test]
    fn a_preset_gives_back_its_input() {
        let mut presets = CopyPresets::default();
        let p = presets.add(input("Sony FX3", "/Volumes/CARD_A")).unwrap();
        let back = p.input();
        assert_eq!(
            (back.name, back.source),
            ("Sony FX3".to_string(), "/Volumes/CARD_A".to_string())
        );
        let json = serde_json::to_value(p.input()).unwrap();
        assert!(json.get("includeFolder").is_some(), "{json}");
    }

    /// Review: names and archive days have limits, so a file can't bring in one that breaks.
    #[test]
    fn names_and_archive_days_have_limits() {
        let long = "x".repeat(201);
        assert_eq!(
            CopyPresets::normalized(input(&long, "")).unwrap_err(),
            "The name is too long: 200 characters at most."
        );
        let mirror = MirrorPresetInput {
            ignore: Vec::new(),
            name: "Footage".into(),
            origin: "/a".into(),
            destination: "/b".into(),
            deleted: DeletedFiles {
                mode: DeletedMode::Archive,
                days: 4_000_000_000,
            },
            deep_check: false,
        };
        assert_eq!(
            MirrorPresets::normalized(mirror.clone()).unwrap_err(),
            "Keep archived files for at most 36,500 days."
        );
        // In Delete mode the days are how long what's already archived stays: 0 would empty
        // the archive at the next run without asking (#113).
        let delete_now = MirrorPresetInput {
            deleted: DeletedFiles {
                mode: DeletedMode::Delete,
                days: 0,
            },
            ..mirror
        };
        assert_eq!(
            MirrorPresets::normalized(delete_now).unwrap_err().key,
            "errors.field.days.tooFew"
        );
    }

    /// #80: settings saved before the menu bar setting existed keep it on.
    #[test]
    fn the_menu_bar_setting_is_on_unless_turned_off() {
        let old: Settings = serde_json::from_str(r#"{"writeChecksumFile":true}"#).unwrap();
        assert!(old.keep_in_menu_bar);
        assert!(Settings::default().keep_in_menu_bar);
        let off: Settings = serde_json::from_str(r#"{"keepInMenuBar":false}"#).unwrap();
        assert!(!off.keep_in_menu_bar);
        let json = serde_json::to_value(&off).unwrap();
        assert_eq!(json["keepInMenuBar"], false);
    }

    /// #154: ASC MHL is off unless turned on, also in settings saved before it existed.
    #[test]
    fn write_mhl_is_off_by_default_and_saved() {
        assert!(!Settings::default().write_mhl);
        let old: Settings = serde_json::from_str(r#"{"writeChecksumFile":true}"#).unwrap();
        assert!(!old.write_mhl);
        let on: Settings = serde_json::from_str(r#"{"writeMhl":true}"#).unwrap();
        assert!(on.write_mhl);
        assert_eq!(serde_json::to_value(&on).unwrap()["writeMhl"], true);
    }

    /// #158: settings saved before the list existed start from the defaults.
    #[test]
    fn the_ignore_list_starts_as_the_defaults() {
        let old: Settings = serde_json::from_str(r#"{"writeChecksumFile":true}"#).unwrap();
        assert_eq!(
            old.ignore,
            secopy_core::ignore::Patterns::defaults().as_slice()
        );
        assert_eq!(Settings::default().ignore, old.ignore);
    }

    /// #158: a bad pattern in the file is dropped; the rest load.
    #[test]
    fn a_bad_pattern_in_the_file_is_dropped() {
        let s: Settings =
            serde_json::from_str(r#"{"writeChecksumFile":false,"ignore":["a/b","*.LRF"]}"#)
                .unwrap();
        assert_eq!(s.ignore, ["*.LRF"]);
        assert!(!s.write_checksum_file);
    }

    /// #164: presets saved before have no list of their own.
    #[test]
    fn a_preset_saved_before_has_no_list() {
        let copy: CopyPreset = serde_json::from_str(
            r#"{"id":"a","name":"A","source":"","includeFolder":true,"extensions":null}"#,
        )
        .unwrap();
        assert!(copy.ignore.is_empty());
        let mirror: MirrorPreset = serde_json::from_str(
            r#"{"id":"m","name":"M","origin":"/o","destination":"/d","deleted":{"mode":"archive","days":30},"deepCheck":false}"#,
        )
        .unwrap();
        assert!(mirror.ignore.is_empty());
    }

    /// #164: a preset's list is saved, read leniently, and checked when typed.
    #[test]
    fn a_presets_list_is_saved_and_checked() {
        let read: CopyPreset = serde_json::from_str(
            r#"{"id":"a","name":"A","source":"","includeFolder":true,"extensions":null,"ignore":["a/b",".gitkeep"]}"#,
        )
        .unwrap();
        assert_eq!(read.ignore, [".gitkeep"]);
        let mut presets = CopyPresets::default();
        let bad = CopyPresetInput {
            ignore: vec!["a/b".into()],
            ..input("Bad", "")
        };
        assert_eq!(presets.add(bad).unwrap_err().key, "errors.field.ignore");
        let good = CopyPresetInput {
            ignore: vec![" .gitkeep ".into()],
            ..input("Good", "")
        };
        assert_eq!(presets.add(good).unwrap().ignore, [".gitkeep"]);
    }

    /// #164 review: presets with their own list are saved as version 2, which an older
    /// Secopy refuses (it would drop the list, and a mirror could delete what it protects);
    /// without one, still version 1.
    #[test]
    fn presets_with_a_list_are_saved_for_this_version_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let mirror = |ignore: Vec<String>| MirrorPresets {
            presets: vec![MirrorPreset {
                id: "m".into(),
                name: "M".into(),
                origin: "/o".into(),
                destination: "/d".into(),
                deleted: DeletedFiles {
                    mode: DeletedMode::Delete,
                    days: 30,
                },
                deep_check: false,
                clear_archive: None,
                ignore,
            }],
        };
        let version = || -> u64 {
            let v: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(dir.path().join("mirrors.json")).unwrap())
                    .unwrap();
            v["version"].as_u64().unwrap()
        };
        store.save("mirrors.json", &mirror(Vec::new())).unwrap();
        assert_eq!(version(), 1);
        store
            .save("mirrors.json", &mirror(vec!["*.LRF".into()]))
            .unwrap();
        assert_eq!(version(), 2);
        let (read, warning) = store.load::<MirrorPresets>("mirrors.json");
        assert!(warning.is_none());
        assert_eq!(read.presets[0].ignore, ["*.LRF"]);
    }

    /// #164 review: repairing presets at start keeps their lists.
    #[test]
    fn repairing_presets_keeps_their_lists() {
        let presets: CopyPresets = serde_json::from_str(
            r#"{"profiles":[{"id":"a","name":"A","source":"","includeFolder":true,"extensions":null,"ignore":[".gitkeep"]}]}"#,
        )
        .unwrap();
        let (repaired, _) = presets.repaired();
        assert_eq!(repaired.presets[0].ignore, [".gitkeep"]);
    }
}
