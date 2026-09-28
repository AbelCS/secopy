//! The `.secopy` file (#77): settings, copy presets and mirror presets, in any mix, to move a
//! setup between Macs and people. Presets carry no ids: ids belong to one Mac.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use crate::store::{
    CopyPreset, CopyPresetInput, CopyPresets, MirrorPreset, MirrorPresetInput, MirrorPresets,
    Settings,
};

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
    let name = path
        .file_name()
        .ok_or("That isn't a file name.")?
        .to_string_lossy();
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
    let size = fs::metadata(path)
        .map_err(|e| format!("The file can't be opened: {e}"))?
        .len();
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
    let format = file
        .get("secopy")
        .and_then(Value::as_u64)
        .ok_or(NOT_SECOPY)?;
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
    Ok(Contents {
        settings,
        copy_presets,
        mirror_presets,
    })
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
        Ok(theirs) => SettingsImport {
            changes: settings_changes(settings, theirs),
            problem: None,
        },
        Err(why) => SettingsImport {
            changes: Vec::new(),
            problem: Some(why.clone()),
        },
    });
    // Keep both for every preset, in order, so names in the file don't clash with each other.
    let mut copy_names = copy.clone();
    let copy_presets = c
        .copy_presets
        .iter()
        .map(|p| {
            match p.as_ref().map_err(|u| u.clone()).and_then(|p| {
                CopyPresets::normalized(p.clone()).map_err(|why| Unreadable {
                    name: p.name.clone(),
                    why,
                })
            }) {
                Err(u) => unreadable(u),
                Ok(p) => {
                    let row = PresetImport {
                        clash: copy.named(&p.name).map(|x| x.name.clone()),
                        new_name: copy_names.free_name(&p.name),
                        missing: [&p.source]
                            .into_iter()
                            .filter(|s| !s.is_empty() && !exists(s))
                            .cloned()
                            .collect(),
                        paths: [p.source.clone()]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect(),
                        name: p.name.clone(),
                        problem: None,
                    };
                    let _ = copy_names.add(CopyPresetInput {
                        name: row.new_name.clone(),
                        ..p
                    });
                    row
                }
            }
        })
        .collect();
    let mut mirror_names = mirrors.clone();
    let mirror_presets = c
        .mirror_presets
        .iter()
        .map(|p| {
            match p.as_ref().map_err(|u| u.clone()).and_then(|p| {
                MirrorPresets::normalized(p.clone()).map_err(|why| Unreadable {
                    name: p.name.clone(),
                    why,
                })
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
                    let _ = mirror_names.add(MirrorPresetInput {
                        name: row.new_name.clone(),
                        ..p
                    });
                    row
                }
            }
        })
        .collect();
    ImportView {
        file_name: file_name.to_string(),
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
    }
}

/// The copy presets after importing the chosen ones, in the file's order, and how many.
pub fn apply_copy(
    c: &Contents,
    chosen: &[PresetChoice],
    copy: &CopyPresets,
) -> Result<(CopyPresets, usize), String> {
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
pub fn apply_mirrors(
    c: &Contents,
    chosen: &[PresetChoice],
    mirrors: &MirrorPresets,
) -> Result<(MirrorPresets, usize), String> {
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
        (
            "Write the checksum file",
            from.write_checksum_file,
            to.write_checksum_file,
        ),
        (
            "Show the count of skipped system files",
            from.show_system_count,
            to.show_system_count,
        ),
        (
            "Save the report next to the checksum file",
            from.report_next_to_checksum,
            to.report_next_to_checksum,
        ),
        (
            "Notify when a copy finishes",
            from.notify_when_done,
            to.notify_when_done,
        ),
    ]
    .into_iter()
    .filter(|(_, a, b)| a != b)
    .map(|(what, a, b)| format!("{what}: {} → {}", on(a), on(b)))
    .collect()
}

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
        for text in [
            "",
            "hello",
            "[1,2]",
            r#"{"version":1}"#,
            r#"{"secopy":"one"}"#,
        ] {
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
        assert!(
            bad.why.starts_with("Its details can't be read"),
            "{}",
            bad.why
        );
        assert_eq!(
            read.copy_presets[2].clone().unwrap_err().name,
            "A copy preset with no name"
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

    #[test]
    fn replace_keeps_the_id_a_queued_mirror_uses() {
        let before = mirror_presets();
        let id = before.presets[0].id.clone();
        let text = r#"{"secopy":1,"mirrorPresets":[{"name":"FOOTAGE","origin":"/Volumes/SSD/Footage",
            "destination":"/Volumes/NAS/Footage","deleted":{"mode":"delete","days":0},"deepCheck":false}]}"#;
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
            view.mirror_presets[0].problem.as_deref(),
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
}
