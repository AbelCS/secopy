//! The `.secopy` file (#77): settings, copy presets and mirror presets, in any mix, to move a
//! setup between Macs and people. Presets carry no ids: ids belong to one Mac.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use crate::message::Message;
use crate::msg;
use crate::store::{
    CopyPreset, CopyPresetInput, CopyPresets, DeletedMode, MirrorPreset, MirrorPresetInput,
    MirrorPresets, Settings,
};

/// The format this Secopy writes and reads.
pub const FORMAT: u32 = 1;
/// A real file is a few KB; anything this big isn't one, and isn't read.
pub const MAX_BYTES: u64 = 10 << 20;
/// Presets of each kind a file may hold: more is no real setup, and isn't worked through.
pub const MAX_PRESETS: usize = 1000;
pub fn not_secopy() -> Message {
    msg!("errors.import.notSecopy")
}

pub fn nothing() -> Message {
    msg!("errors.import.nothing")
}

/// A file's contents: each preset read on its own, so one bad preset doesn't block the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Contents {
    /// The version of Secopy that wrote the file, if it says.
    pub app: Option<String>,
    pub settings: Option<Result<Settings, Message>>,
    /// Settings in the file this Secopy doesn't know (a newer one's): left out (#149).
    pub settings_unknown: Vec<String>,
    /// Settings this Secopy has that the file lacks (an older one's): they take their
    /// defaults (#149).
    pub settings_defaulted: Vec<String>,
    pub copy_presets: Vec<Result<CopyPresetInput, Unreadable>>,
    pub mirror_presets: Vec<Result<MirrorPresetInput, Unreadable>>,
}

/// A preset in the file that can't be read: its name if it has one (else empty), and why;
/// `section` when the whole list of presets can't be read.
#[derive(Debug, Clone, PartialEq)]
pub struct Unreadable {
    pub name: String,
    pub why: Message,
    pub section: bool,
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
pub fn write_file(path: &Path, text: &str) -> Result<(), Message> {
    let name = path
        .file_name()
        .ok_or_else(|| msg!("errors.export.notAFileName"))?
        .to_string_lossy();
    // A name nobody can have put a link at, created only if nothing is there (never through
    // a link: a file the save panel didn't name is never written).
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp = path.with_file_name(format!(
        ".{name}.{}-{nanos:x}.secopy-tmp",
        std::process::id()
    ));
    let written = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written.map_err(|e| msg!("errors.export.notSaved", why = crate::say::io_error(&e)))
}

/// Reads the file at `path`: only a plain file, and never more than [`MAX_BYTES`] of it.
pub fn read_file(path: &Path) -> Result<Contents, Message> {
    use std::io::Read;
    // A device or a pipe could be read forever (or never open): not a Secopy file.
    let meta = fs::metadata(path)
        .map_err(|e| msg!("errors.import.cantOpen", why = crate::say::io_error(&e)))?;
    if !meta.is_file() {
        return Err(not_secopy());
    }
    if meta.len() > MAX_BYTES {
        return Err(msg!("errors.import.tooBig"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|f| f.take(MAX_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| msg!("errors.import.cantRead", why = crate::say::io_error(&e)))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(msg!("errors.import.tooBig"));
    }
    read(&bytes)
}

/// Reads a file's bytes. Strict about what it is (a Secopy file of a format this Secopy
/// knows), lenient inside: a preset that can't be read, or has a setting this Secopy doesn't
/// know, is kept as [`Unreadable`] next to the others; settings it doesn't know are left out,
/// and ones the file lacks take their defaults, each listed (#149).
pub fn read(bytes: &[u8]) -> Result<Contents, Message> {
    // serde_json stops at 128 levels of nesting: a hostile file is an error, not a crash.
    let value: Value = serde_json::from_slice(bytes).map_err(|_| not_secopy())?;
    let Value::Object(mut file) = value else {
        return Err(not_secopy());
    };
    let format = file
        .get("secopy")
        .and_then(Value::as_u64)
        .ok_or_else(not_secopy)?;
    if format > u64::from(FORMAT) {
        return Err(msg!("errors.import.newer", format = format.to_string()));
    }
    let app = file
        .get("app")
        .and_then(Value::as_str)
        .map(|a| a.trim().chars().take(40).collect::<String>())
        .filter(|a| !a.is_empty());
    let (mut settings_unknown, mut settings_defaulted) = (Vec::new(), Vec::new());
    let settings = file.remove("settings").map(|v| {
        let read = serde_json::from_value::<Settings>(v.clone())
            .map_err(|_| msg!("import.problem.settings"))?;
        let known = serde_json::to_value(&read).expect("settings serialize");
        settings_unknown = unknown_keys(&v, &known);
        settings_defaulted = unknown_keys(&known, &v);
        Ok(read)
    });
    let (copies, mirrors) = (file.remove("copyPresets"), file.remove("mirrorPresets"));
    let count = |list: &Option<Value>| list.as_ref().and_then(Value::as_array).map_or(0, Vec::len);
    if count(&copies) > MAX_PRESETS || count(&mirrors) > MAX_PRESETS {
        return Err(msg!("errors.import.tooMany", max = MAX_PRESETS));
    }
    let copy_presets = presets(copies);
    let mirror_presets = presets(mirrors);
    if settings.is_none() && copy_presets.is_empty() && mirror_presets.is_empty() {
        return Err(nothing());
    }
    Ok(Contents {
        app,
        settings,
        settings_unknown,
        settings_defaulted,
        copy_presets,
        mirror_presets,
    })
}

/// The keys of `theirs` that `ours` lacks, nested ones as `outer.inner`, in `theirs`' order.
fn unknown_keys(theirs: &Value, ours: &Value) -> Vec<String> {
    let (Value::Object(theirs), Value::Object(ours)) = (theirs, ours) else {
        return Vec::new();
    };
    theirs
        .iter()
        .flat_map(|(key, value)| match ours.get(key) {
            None => vec![key.clone()],
            Some(known) => unknown_keys(value, known)
                .into_iter()
                .map(|inner| format!("{key}.{inner}"))
                .collect(),
        })
        .collect()
}

/// Each entry of a preset list, read on its own. One with no name gets an empty one (the UI
/// says "no name"). One with a setting this Secopy doesn't know isn't read: imported without
/// it, it would do something other than what it did where it was made (#149).
fn presets<T: serde::de::DeserializeOwned + Serialize>(
    list: Option<Value>,
) -> Vec<Result<T, Unreadable>> {
    let items = match list {
        None => return Vec::new(),
        Some(Value::Array(items)) => items,
        // Shown, not dropped: the file meant to hold some.
        Some(_) => {
            return vec![Err(Unreadable {
                name: String::new(),
                why: msg!("import.problem.section"),
                section: true,
            })];
        }
    };
    items
        .into_iter()
        .map(|item| {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .map(|n| n.trim().chars().take(200).collect::<String>())
                .unwrap_or_default();
            let read = serde_json::from_value::<T>(item.clone()).map_err(|e| Unreadable {
                name: name.clone(),
                why: msg!("import.problem.details", detail = e.to_string()),
                section: false,
            })?;
            let known = serde_json::to_value(&read).expect("presets serialize");
            let unknown = unknown_keys(&item, &known);
            if unknown.is_empty() {
                Ok(read)
            } else {
                Err(Unreadable {
                    name,
                    why: msg!("import.problem.newer", keys = unknown.join(", ")),
                    section: false,
                })
            }
        })
        .collect()
}

/// What the Import screen shows: nothing is changed by making it.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportView {
    pub file_name: String,
    /// "Made by Secopy 0.1.0; this is 0.17.6.": only when the file says, and it's another
    /// version (#149).
    pub made_by: Option<Message>,
    /// `None`: the file has no settings.
    pub settings: Option<SettingsImport>,
    pub copy_presets: Vec<PresetImport>,
    pub mirror_presets: Vec<PresetImport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsImport {
    /// "Write the checksum file: on → off"; empty when they're the same as yours.
    pub changes: Vec<Message>,
    pub problem: Option<Message>,
    /// Settings in the file this Secopy doesn't know, as the file names them: left out.
    pub not_imported: Vec<String>,
    /// Settings the file lacks, which take their defaults.
    pub defaulted: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetImport {
    /// Empty when the preset in the file has none.
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
    pub problem: Option<Message>,
    /// What Replace does beyond the preset itself (#113): a mirror in Delete mode deletes
    /// what's removed from the origin; with fewer days, its next run removes archived files.
    pub replace_notes: Vec<Message>,
    /// It stands for the file's whole list of presets, which can't be read.
    pub section: bool,
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
        Ok(theirs) => SettingsImport {
            changes: settings_changes(settings, theirs),
            problem: None,
            not_imported: c.settings_unknown.clone(),
            defaulted: c
                .settings_defaulted
                .iter()
                .filter_map(|k| setting_label(k))
                .collect(),
        },
        Err(why) => SettingsImport {
            changes: Vec::new(),
            problem: Some(why.clone()),
            not_imported: Vec::new(),
            defaulted: Vec::new(),
        },
    });
    let ours = env!("CARGO_PKG_VERSION");
    let made_by = c
        .app
        .as_deref()
        .filter(|theirs| *theirs != ours)
        .map(|theirs| msg!("import.madeBy", theirs = theirs, ours = ours));
    let copy_presets = copy_rows(c, copy)
        .into_iter()
        .map(|row| match row {
            Err(u) => unreadable(u),
            Ok((p, new_name)) => {
                let paths: Vec<String> = [p.source.clone()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect();
                PresetImport {
                    clash: copy.named(&p.name).map(|x| x.name.clone()),
                    new_name,
                    missing: paths.iter().filter(|s| !exists(s)).cloned().collect(),
                    paths,
                    name: p.name,
                    problem: None,
                    replace_notes: Vec::new(),
                    section: false,
                }
            }
        })
        .collect();
    let mirror_presets = mirror_rows(c, mirrors)
        .into_iter()
        .map(|row| match row {
            Err(u) => unreadable(u),
            Ok((p, new_name)) => {
                let paths = vec![p.origin.clone(), p.destination.clone()];
                let yours = mirrors.named(&p.name);
                let mut replace_notes = Vec::new();
                if let Some(y) = yours {
                    if p.deleted.mode == DeletedMode::Delete
                        && y.deleted.mode == DeletedMode::Archive
                    {
                        replace_notes.push(msg!("import.replaceDeletes"));
                    }
                    if p.deleted.days < y.deleted.days {
                        replace_notes.push(msg!("import.replaceShortens", count = p.deleted.days));
                    }
                }
                PresetImport {
                    clash: yours.map(|x| x.name.clone()),
                    replace_notes,
                    new_name,
                    missing: paths.iter().filter(|s| !exists(s)).cloned().collect(),
                    paths,
                    name: p.name,
                    problem: None,
                    section: false,
                }
            }
        })
        .collect();
    ImportView {
        file_name: file_name.to_string(),
        made_by,
        settings,
        copy_presets,
        mirror_presets,
    }
}

fn unreadable(u: Unreadable) -> PresetImport {
    PresetImport {
        new_name: u.name.clone(),
        name: u.name,
        paths: Vec::new(),
        clash: None,
        missing: Vec::new(),
        problem: Some(u.why),
        replace_notes: Vec::new(),
        section: u.section,
    }
}

/// Each copy preset in the file, checked like one typed by hand, with the name Keep both
/// gives it. Every earlier preset in the file counts as kept, whatever is ticked, so the name
/// the Import screen shows is the name it gets.
fn copy_rows(
    c: &Contents,
    copy: &CopyPresets,
) -> Vec<Result<(CopyPresetInput, String), Unreadable>> {
    let mut names = copy.clone();
    c.copy_presets
        .iter()
        .map(|p| {
            let p = p.as_ref().map_err(Clone::clone)?;
            let p = CopyPresets::normalized(p.clone()).map_err(|why| Unreadable {
                name: p.name.clone(),
                why,
                section: false,
            })?;
            let name = names.free_name(&p.name);
            let _ = names.add(CopyPresetInput {
                name: name.clone(),
                ..p.clone()
            });
            Ok((p, name))
        })
        .collect()
}

/// [`copy_rows`] for mirror presets.
fn mirror_rows(
    c: &Contents,
    mirrors: &MirrorPresets,
) -> Vec<Result<(MirrorPresetInput, String), Unreadable>> {
    let mut names = mirrors.clone();
    c.mirror_presets
        .iter()
        .map(|p| {
            let p = p.as_ref().map_err(Clone::clone)?;
            let p = MirrorPresets::normalized(p.clone()).map_err(|why| Unreadable {
                name: p.name.clone(),
                why,
                section: false,
            })?;
            let name = names.free_name(&p.name);
            let _ = names.add(MirrorPresetInput {
                name: name.clone(),
                ..p.clone()
            });
            Ok((p, name))
        })
        .collect()
}

fn two_replace(name: &str) -> Message {
    msg!("errors.import.twoReplace", name = name)
}

/// The copy presets after importing the chosen ones, in the file's order, and how many.
/// Replace takes the place of your preset with that name; Keep both gives the name the Import
/// screen showed.
pub fn apply_copy(
    c: &Contents,
    chosen: &[PresetChoice],
    copy: &CopyPresets,
) -> Result<(CopyPresets, usize), Message> {
    let rows = copy_rows(c, copy);
    let mut next = copy.clone();
    let mut replaced = std::collections::HashSet::new();
    let chosen = sorted(chosen);
    for &choice in &chosen {
        let (input, keep_name) = rows
            .get(choice.index as usize)
            .and_then(|r| r.as_ref().ok())
            .cloned()
            .ok_or_else(|| msg!("errors.import.cantImport"))?;
        match copy.named(&input.name) {
            Some(yours) if choice.replace => {
                if !replaced.insert(yours.id.clone()) {
                    return Err(two_replace(&yours.name));
                }
                next.edit(&yours.id, input)?;
            }
            _ => {
                next.add(CopyPresetInput {
                    name: keep_name,
                    ..input
                })?;
            }
        }
    }
    Ok((next, chosen.len()))
}

/// The mirror presets after importing the chosen ones, and how many; like [`apply_copy`].
pub fn apply_mirrors(
    c: &Contents,
    chosen: &[PresetChoice],
    mirrors: &MirrorPresets,
) -> Result<(MirrorPresets, usize), Message> {
    let rows = mirror_rows(c, mirrors);
    let mut next = mirrors.clone();
    let mut replaced = std::collections::HashSet::new();
    let chosen = sorted(chosen);
    for &choice in &chosen {
        let (input, keep_name) = rows
            .get(choice.index as usize)
            .and_then(|r| r.as_ref().ok())
            .cloned()
            .ok_or_else(|| msg!("errors.import.cantImport"))?;
        match mirrors.named(&input.name) {
            Some(yours) if choice.replace => {
                if !replaced.insert(yours.id.clone()) {
                    return Err(two_replace(&yours.name));
                }
                next.edit(&yours.id, input)?;
            }
            _ => {
                next.add(MirrorPresetInput {
                    name: keep_name,
                    ..input
                })?;
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
/// A setting's name on the Import screen, from its key in the file.
fn setting_label(key: &str) -> Option<Message> {
    Some(match key {
        "writeChecksumFile" => msg!("import.setting.checksumFile"),
        "showSystemCount" => msg!("import.setting.systemCount"),
        "reportNextToChecksum" => msg!("import.setting.report"),
        "notifyWhenDone" => msg!("import.setting.notify"),
        "keepInMenuBar" => msg!("import.setting.menuBar"),
        _ => return None,
    })
}

pub fn settings_changes(from: &Settings, to: &Settings) -> Vec<Message> {
    let on = |b: bool| {
        if b {
            msg!("import.on")
        } else {
            msg!("import.off")
        }
    };
    [
        (
            msg!("import.setting.checksumFile"),
            from.write_checksum_file,
            to.write_checksum_file,
        ),
        (
            msg!("import.setting.systemCount"),
            from.show_system_count,
            to.show_system_count,
        ),
        (
            msg!("import.setting.report"),
            from.report_next_to_checksum,
            to.report_next_to_checksum,
        ),
        (
            msg!("import.setting.notify"),
            from.notify_when_done,
            to.notify_when_done,
        ),
        (
            msg!("import.setting.menuBar"),
            from.keep_in_menu_bar,
            to.keep_in_menu_bar,
        ),
    ]
    .into_iter()
    .filter(|(_, a, b)| a != b)
    .map(|(what, a, b)| msg!("import.change", setting = what, from = on(a), to = on(b)))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::En;
    use crate::store::{CopyPresets, DeletedFiles, MirrorPresetInput, MirrorPresets};

    /// Review: a section that can't be read is the section, not a preset with no name.
    #[test]
    fn an_unreadable_section_says_which_section() {
        let text = r#"{"secopy":1,"mirrorPresets":{"a":1}}"#;
        let view = plan(
            "x",
            &contents(text),
            &CopyPresets::default(),
            &MirrorPresets::default(),
            &Settings::default(),
            &|_| true,
        );
        assert!(view.mirror_presets[0].section);
        let text = r#"{"secopy":1,"copyPresets":[{"source":7}]}"#;
        let view = plan(
            "x",
            &contents(text),
            &CopyPresets::default(),
            &MirrorPresets::default(),
            &Settings::default(),
            &|_| true,
        );
        assert!(!view.copy_presets[0].section, "one preset with no name");
    }

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
            deleted: DeletedFiles {
                mode: DeletedMode::Archive,
                days: 30,
            },
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
        let settings = Settings {
            write_checksum_file: false,
            ..Settings::default()
        };
        let (c, m) = (copy_presets(), mirror_presets());
        let text = export_text(Some(&settings), &c.presets, &m.presets, "0.11.0", now());
        let read = read(text.as_bytes()).unwrap();
        assert_eq!(read.settings, Some(Ok(settings)));
        let copies: Vec<CopyPresetInput> =
            read.copy_presets.into_iter().map(Result::unwrap).collect();
        assert_eq!(
            copies,
            c.presets.iter().map(|p| p.input()).collect::<Vec<_>>()
        );
        let mirrors: Vec<MirrorPresetInput> = read
            .mirror_presets
            .into_iter()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            mirrors,
            m.presets.iter().map(|p| p.input()).collect::<Vec<_>>()
        );
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

    /// #149: a preset with a setting this Secopy doesn't know (a newer one's) isn't imported
    /// without it: it says so and can't be ticked. Unknown file-level keys are fine.
    #[test]
    fn a_preset_with_settings_this_secopy_doesnt_know_isnt_imported() {
        let text = r#"{"secopy":1,"later":true,"copyPresets":[
            {"name":"A","source":"","includeFolder":true,"extensions":null,"colour":"red"},
            {"name":"B","source":"","includeFolder":true,"extensions":null}]}"#;
        let c = read(text.as_bytes()).unwrap();
        let refused = c.copy_presets[0].as_ref().unwrap_err();
        assert_eq!(refused.name, "A");
        assert_eq!(refused.why.key, "import.problem.newer");
        assert!(c.copy_presets[1].is_ok());
        let nested = r#"{"secopy":1,"mirrorPresets":[{"name":"M","origin":"/o","destination":"/d",
            "deleted":{"mode":"archive","days":30,"trash":true},"deepCheck":false}]}"#;
        let c = read(nested.as_bytes()).unwrap();
        assert_eq!(
            c.mirror_presets[0].as_ref().unwrap_err().why.key,
            "import.problem.newer"
        );
    }

    /// #149: settings this Secopy doesn't know are left out, and said; settings an older
    /// Secopy's file lacks take their defaults, and that's said too.
    #[test]
    fn settings_say_what_they_leave_out_and_what_takes_its_default() {
        let text = r#"{"secopy":1,"app":"0.1.0","settings":{"writeChecksumFile":false,
            "showSystemCount":true,"reportNextToChecksum":false,"notifyWhenDone":true,
            "turbo":true}}"#;
        let c = read(text.as_bytes()).unwrap();
        assert_eq!(c.settings_unknown, ["turbo"]);
        assert_eq!(c.settings_defaulted, ["keepInMenuBar"]);
        assert_eq!(c.app.as_deref(), Some("0.1.0"));
        let view = plan(
            "x.secopy",
            &c,
            &CopyPresets::default(),
            &MirrorPresets::default(),
            &Settings::default(),
            &|_| true,
        );
        let s = view.settings.unwrap();
        assert_eq!(s.not_imported, ["turbo"]);
        assert_eq!(
            s.defaulted
                .iter()
                .map(|m| m.key.as_str())
                .collect::<Vec<_>>(),
            ["import.setting.menuBar"]
        );
        assert_eq!(view.made_by.unwrap().key, "import.madeBy");
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
        for text in [
            "",
            "hello",
            "[1,2]",
            r#"{"version":1}"#,
            r#"{"secopy":"one"}"#,
        ] {
            assert_eq!(read(text.as_bytes()).unwrap_err(), not_secopy(), "{text:?}");
        }
        assert_eq!(read(br#"{"secopy":1}"#).unwrap_err(), nothing());
    }

    #[test]
    fn nested_json_is_refused_not_a_crash() {
        let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
        assert_eq!(read(deep.as_bytes()).unwrap_err(), not_secopy());
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
        assert!(
            bad.why.starts_with("Its details can't be read"),
            "{}",
            bad.why
        );
        assert_eq!(
            read.copy_presets[2].clone().unwrap_err().name,
            "",
            "no name in the file: the UI says so"
        );
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
        assert_eq!(
            read_file(&path).unwrap_err(),
            "This file is too big to be a Secopy file."
        );
    }

    #[test]
    fn writing_leaves_no_half_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Sony FX3.secopy");
        write_file(&path, "{\"secopy\":1}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"secopy\":1}");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "no temporary file left: {names:?}");
        let missing = dir.path().join("no-such-dir").join("x.secopy");
        assert!(write_file(&missing, "{}").is_err());
        assert!(!missing.exists());
    }

    fn contents(text: &str) -> Contents {
        read(text.as_bytes()).unwrap()
    }

    const TWO_COPIES: &str = r#"{"secopy":1,"copyPresets":[
        {"name":"sony fx3","source":"/Volumes/NEW/CLIP","includeFolder":false,"extensions":["mov"]},
        {"name":"DJI","source":"/nowhere/DCIM","includeFolder":true,"extensions":null}
    ]}"#;

    #[test]
    fn a_clash_offers_keep_both_with_a_free_name() {
        let view = plan(
            "x.secopy",
            &contents(TWO_COPIES),
            &copy_presets(),
            &MirrorPresets::default(),
            &Settings::default(),
            &|p| p.starts_with("/Volumes"),
        );
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
        let view = plan(
            "x",
            &c,
            &CopyPresets::default(),
            &MirrorPresets::default(),
            &Settings::default(),
            &|_| true,
        );
        assert_eq!(view.copy_presets[0].new_name, "A");
        assert_eq!(view.copy_presets[1].new_name, "a (2)");
        let all = [
            PresetChoice {
                index: 0,
                replace: false,
            },
            PresetChoice {
                index: 1,
                replace: false,
            },
        ];
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
        let (kept, _) = apply_copy(
            &c,
            &[PresetChoice {
                index: 0,
                replace: false,
            }],
            &before,
        )
        .unwrap();
        assert_eq!(kept.presets.len(), 3);
        assert!(kept.named("sony fx3 (2)").is_some());
        let (replaced, _) = apply_copy(
            &c,
            &[PresetChoice {
                index: 0,
                replace: true,
            }],
            &before,
        )
        .unwrap();
        assert_eq!(replaced.presets.len(), 2);
        let p = replaced.get(&id).unwrap();
        assert_eq!(
            (p.name.as_str(), p.source.as_str()),
            ("sony fx3", "/Volumes/NEW/CLIP")
        );
    }

    /// QA review (#113): replacing a mirror with fewer days removes archived files at its next
    /// run; the preview says so, as the editor does.
    #[test]
    fn replacing_a_mirror_with_fewer_days_says_what_goes() {
        let text = r#"{"secopy":1,"mirrorPresets":[{"name":"footage","origin":"/o","destination":"/d",
            "deleted":{"mode":"delete","days":7},"deepCheck":false}]}"#;
        let view = plan(
            "x.secopy",
            &contents(text),
            &CopyPresets::default(),
            &mirror_presets(),
            &Settings::default(),
            &|_| true,
        );
        let keys = |v: &ImportView| -> Vec<String> {
            v.mirror_presets[0]
                .replace_notes
                .iter()
                .map(|m| m.key.clone())
                .collect()
        };
        assert_eq!(
            keys(&view),
            ["import.replaceDeletes", "import.replaceShortens"]
        );
        let same = text.replace("\"days\":7", "\"days\":30");
        let view = plan(
            "x.secopy",
            &contents(&same),
            &CopyPresets::default(),
            &mirror_presets(),
            &Settings::default(),
            &|_| true,
        );
        assert_eq!(
            keys(&view),
            ["import.replaceDeletes"],
            "Archive → Delete with the same days: removed files are no longer archived"
        );
        let archive = same.replace("\"delete\"", "\"archive\"");
        let view = plan(
            "x.secopy",
            &contents(&archive),
            &CopyPresets::default(),
            &mirror_presets(),
            &Settings::default(),
            &|_| true,
        );
        assert!(keys(&view).is_empty());
    }

    #[test]
    fn replace_keeps_the_id_a_queued_mirror_uses() {
        let before = mirror_presets();
        let id = before.presets[0].id.clone();
        let text = r#"{"secopy":1,"mirrorPresets":[{"name":"FOOTAGE","origin":"/Volumes/SSD/Footage",
            "destination":"/Volumes/NAS/Footage","deleted":{"mode":"delete","days":30},"deepCheck":false}]}"#;
        let (after, n) = apply_mirrors(
            &contents(text),
            &[PresetChoice {
                index: 0,
                replace: true,
            }],
            &before,
        )
        .unwrap();
        assert_eq!(n, 1);
        assert_eq!(after.get(&id).unwrap().destination, "/Volumes/NAS/Footage");
    }

    #[test]
    fn a_preset_that_fails_the_checks_is_a_problem_and_cant_be_chosen() {
        let text = r#"{"secopy":1,"mirrorPresets":[{"name":"Loop","origin":"/a","destination":"/a/b",
            "deleted":{"mode":"archive","days":30},"deepCheck":false}]}"#;
        let c = contents(text);
        let view = plan(
            "x",
            &c,
            &CopyPresets::default(),
            &MirrorPresets::default(),
            &Settings::default(),
            &|_| true,
        );
        assert_eq!(
            view.mirror_presets[0].problem.en().as_deref(),
            Some("The destination can't be inside the origin.")
        );
        assert!(
            apply_mirrors(
                &c,
                &[PresetChoice {
                    index: 0,
                    replace: false
                }],
                &MirrorPresets::default()
            )
            .is_err()
        );
    }

    #[test]
    fn unreadable_presets_are_listed_with_why() {
        let text = r#"{"secopy":1,"copyPresets":[{"name":"Bad","source":7}]}"#;
        let view = plan(
            "x",
            &contents(text),
            &CopyPresets::default(),
            &MirrorPresets::default(),
            &Settings::default(),
            &|_| true,
        );
        assert_eq!(view.copy_presets[0].name, "Bad");
        assert!(
            view.copy_presets[0]
                .problem
                .en()
                .as_deref()
                .unwrap()
                .starts_with("Its details can't be read")
        );
    }

    #[test]
    fn settings_show_only_what_changes() {
        let mine = Settings::default();
        let theirs = Settings {
            write_checksum_file: false,
            notify_when_done: false,
            ..Settings::default()
        };
        assert_eq!(
            settings_changes(&mine, &theirs),
            [
                "Write the checksum file: on → off",
                "Notify when a copy finishes: on → off"
            ]
        );
        assert!(settings_changes(&mine, &mine).is_empty());
        let text = format!(
            r#"{{"secopy":1,"settings":{}}}"#,
            serde_json::to_string(&theirs).unwrap()
        );
        let view = plan(
            "x",
            &contents(&text),
            &CopyPresets::default(),
            &MirrorPresets::default(),
            &mine,
            &|_| true,
        );
        assert_eq!(view.settings.unwrap().changes.len(), 2);
    }

    /// Review: a link at the temporary name can't make an export write through it.
    #[test]
    fn a_link_at_the_temporary_name_is_never_written_through() {
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("clip.mov");
        std::fs::write(&victim, b"footage").unwrap();
        let path = dir.path().join("x.secopy");
        std::os::unix::fs::symlink(&victim, dir.path().join(".x.secopy.secopy-tmp")).unwrap();
        write_file(&path, "{\"secopy\":1}").unwrap();
        assert_eq!(std::fs::read(&victim).unwrap(), b"footage");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"secopy\":1}");
    }

    /// Review: a device or a pipe is refused, not read forever.
    #[test]
    fn a_device_is_not_a_secopy_file() {
        assert_eq!(read_file(Path::new("/dev/zero")).unwrap_err(), not_secopy());
    }

    /// Review: a section that isn't a list is listed as unreadable, not left out.
    #[test]
    fn a_section_of_the_wrong_type_is_listed() {
        let text = r#"{"secopy":1,"settings":{"writeChecksumFile":false},"mirrorPresets":{"a":1}}"#;
        let read = read(text.as_bytes()).unwrap();
        let bad = read.mirror_presets[0].clone().unwrap_err();
        assert_eq!(bad.name, "", "a section has no name; the UI names it");
        assert_eq!(bad.why, "This part of the file can't be read.");
    }

    /// Review: a file with thousands of presets is refused before any work.
    #[test]
    fn too_many_presets_are_refused() {
        let one = r#"{"name":"A","source":"","includeFolder":true,"extensions":null}"#;
        let text = format!(
            r#"{{"secopy":1,"copyPresets":[{}]}}"#,
            vec![one; MAX_PRESETS + 1].join(",")
        );
        assert_eq!(
            read(text.as_bytes()).unwrap_err(),
            "This file has too many presets. Secopy imports up to 1,000 of each kind."
        );
    }

    /// Review: the name the Import screen shows is the name Keep both gives, whatever
    /// happens to the presets before it.
    #[test]
    fn the_name_shown_is_the_name_given() {
        let text = r#"{"secopy":1,"copyPresets":[
            {"name":"Sony FX3","source":"","includeFolder":true,"extensions":null},
            {"name":"SONY FX3","source":"","includeFolder":true,"extensions":null}
        ]}"#;
        let c = contents(text);
        let mine = copy_presets();
        let view = plan(
            "x",
            &c,
            &mine,
            &MirrorPresets::default(),
            &Settings::default(),
            &|_| true,
        );
        let shown = view.copy_presets[1].new_name.clone();
        for first in [None, Some(false), Some(true)] {
            let mut chosen: Vec<PresetChoice> = first
                .map(|replace| PresetChoice { index: 0, replace })
                .into_iter()
                .collect();
            chosen.push(PresetChoice {
                index: 1,
                replace: false,
            });
            let (after, _) = apply_copy(&c, &chosen, &mine).unwrap();
            assert!(
                after.presets.iter().any(|p| p.name == shown),
                "{first:?}: {:?}",
                after.presets
            );
        }
    }

    /// Review: two presets in the file can't both replace one of yours.
    #[test]
    fn two_replaces_of_one_preset_are_refused() {
        let text = r#"{"secopy":1,"copyPresets":[
            {"name":"Sony FX3","source":"/a","includeFolder":true,"extensions":null},
            {"name":"sony fx3","source":"/b","includeFolder":true,"extensions":null}
        ]}"#;
        let both = [
            PresetChoice {
                index: 0,
                replace: true,
            },
            PresetChoice {
                index: 1,
                replace: true,
            },
        ];
        assert_eq!(
            apply_copy(&contents(text), &both, &copy_presets()).unwrap_err(),
            "Two presets in the file would replace “Sony FX3”; choose Keep both for one of them."
        );
    }

    #[test]
    fn the_menu_bar_setting_travels_and_is_named() {
        let off = Settings {
            keep_in_menu_bar: false,
            ..Settings::default()
        };
        assert_eq!(
            settings_changes(&Settings::default(), &off),
            ["Keep copying in the menu bar: on → off"]
        );
        let text = export_text(Some(&off), &[], &[], "0.12.0", now());
        assert_eq!(read(text.as_bytes()).unwrap().settings, Some(Ok(off)));
    }
}
