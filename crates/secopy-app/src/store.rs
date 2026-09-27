//! What the app keeps between launches (plan 3b-1): settings, source profiles and remembered
//! state, as small versioned JSON files in the app's data folder.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::dto::ExtensionKey;

const VERSION: u32 = 1;
pub const SETTINGS: &str = "settings.json";
pub const PROFILES: &str = "profiles.json";
pub const REMEMBERED: &str = "state.json";
/// Recent destinations kept (spec B7).
pub const RECENT: usize = 5;

/// The Settings screen (RFD §5.5).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub write_checksum_file: bool,
    pub show_hidden_count: bool,
    pub report_next_to_checksum: bool,
}

/// `settings.json` as read: missing fields take their defaults. Kept apart from
/// [`Settings`] so the UI's type has every field.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsOnDisk {
    #[serde(default = "yes")]
    write_checksum_file: bool,
    #[serde(default = "yes")]
    show_hidden_count: bool,
    #[serde(default)]
    report_next_to_checksum: bool,
}

fn yes() -> bool {
    true
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = SettingsOnDisk::deserialize(d)?;
        Ok(Self {
            write_checksum_file: s.write_checksum_file,
            show_hidden_count: s.show_hidden_count,
            report_next_to_checksum: s.report_next_to_checksum,
        })
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            write_checksum_file: true,
            show_hidden_count: true,
            report_next_to_checksum: false,
        }
    }
}

/// A source profile (FR-38).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Stays the same when the profile is renamed.
    pub id: String,
    pub name: String,
    /// Relative to what was picked, `/`-separated; empty = the picked folder itself.
    pub folder: String,
    /// "Include the folder" (FR-4).
    pub include_folder: bool,
    /// `None` = every file type, including ones never seen.
    pub extensions: Option<Vec<ExtensionKey>>,
}

impl Profile {
    /// Whether the profile copies files of type `key`.
    pub fn selects(&self, key: &ExtensionKey) -> bool {
        self.extensions
            .as_ref()
            .is_none_or(|keys| keys.contains(key))
    }
}

/// A profile as typed in a form (Save as new…, Settings).
#[derive(Debug, Clone, PartialEq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub name: String,
    pub folder: String,
    pub include_folder: bool,
    pub extensions: Option<Vec<ExtensionKey>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Profiles {
    pub profiles: Vec<Profile>,
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
    pub last_profile: Option<String>,
    /// The last destinations a job started with, most recent first (B7).
    pub recent_destinations: Vec<String>,
}

impl Default for Remembered {
    fn default() -> Self {
        Self {
            verify: true,
            window: None,
            last_profile: None,
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

impl Profiles {
    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn add(&mut self, input: ProfileInput) -> Result<Profile, String> {
        let input = self.check(input, None)?;
        let profile = Profile {
            id: self.new_id(),
            name: input.name,
            folder: input.folder,
            include_folder: input.include_folder,
            extensions: input.extensions,
        };
        self.profiles.push(profile.clone());
        Ok(profile)
    }

    pub fn edit(&mut self, id: &str, input: ProfileInput) -> Result<Profile, String> {
        let input = self.check(input, Some(id))?;
        let profile = self
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("That profile no longer exists.")?;
        profile.name = input.name;
        profile.folder = input.folder;
        profile.include_folder = input.include_folder;
        profile.extensions = input.extensions;
        Ok(profile.clone())
    }

    /// Stores `profile` over the one with its id (Update profile).
    pub fn replace(&mut self, profile: Profile) {
        if let Some(old) = self.profiles.iter_mut().find(|p| p.id == profile.id) {
            *old = profile;
        }
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.id != id);
        self.profiles.len() < before
    }

    /// Validates and normalizes `input`; `editing` is the id of the profile being edited.
    fn check(&self, input: ProfileInput, editing: Option<&str>) -> Result<ProfileInput, String> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err("The profile needs a name.".into());
        }
        let taken = self.profiles.iter().any(|p| {
            Some(p.id.as_str()) != editing && p.name.to_lowercase() == name.to_lowercase()
        });
        if taken {
            return Err(format!("There is already a profile called “{name}”."));
        }
        let extensions = input.extensions.map(|keys| {
            let keys: BTreeSet<ExtensionKey> = keys
                .into_iter()
                .map(|key| {
                    key.map(|e| e.trim().trim_start_matches('.').to_lowercase())
                        .filter(|e| !e.is_empty())
                })
                .collect();
            keys.into_iter().collect()
        });
        Ok(ProfileInput {
            name,
            folder: folder(&input.folder)?,
            include_folder: input.include_folder,
            extensions,
        })
    }

    fn new_id(&self) -> String {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        loop {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64);
            let id = format!(
                "{:x}",
                nanos ^ NEXT.fetch_add(1, Ordering::Relaxed).rotate_left(32)
            );
            if self.get(&id).is_none() {
                return id;
            }
        }
    }
}

/// A profile's folder, normalized: relative, `/`-separated, no `.` or `..`. Empty = the
/// picked folder itself.
pub fn folder(text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.starts_with('/') {
        return Err("The directory is inside the card, so it can't start with “/”.".into());
    }
    let parts: Vec<&str> = text.split('/').filter(|p| !p.is_empty()).collect();
    if parts.iter().any(|p| *p == ".." || *p == ".") {
        return Err("The directory can't contain “..” or “.”.".into());
    }
    Ok(parts.join("/"))
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
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            saving: Mutex::new(()),
        }
    }

    /// Reads `name`: a missing file gives the defaults; a file that can't be read is set
    /// aside and the defaults are used, with a message saying so.
    pub fn load<T: Default + DeserializeOwned>(&self, name: &str) -> (T, Option<String>) {
        let text = match fs::read_to_string(self.dir.join(name)) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (T::default(), None),
            Err(e) => return (T::default(), Some(self.set_aside(name, &e.to_string()))),
        };
        let why = match serde_json::from_str::<VersionOnly>(&text) {
            Ok(v) if v.version > VERSION => {
                format!("it is from a newer Secopy, version {}", v.version)
            }
            _ => match serde_json::from_str::<Versioned<T>>(&text) {
                Ok(v) => return (v.data, None),
                Err(e) => e.to_string(),
            },
        };
        (T::default(), Some(self.set_aside(name, &why)))
    }

    /// Renames a file that can't be read to `<name>.damaged-<time>`; the message for the UI.
    fn set_aside(&self, name: &str, why: &str) -> String {
        let aside = format!(
            "{name}.damaged-{}",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        match fs::rename(self.dir.join(name), self.dir.join(&aside)) {
            Ok(()) => format!(
                "{name} couldn't be read ({why}). It was set aside as {aside} in {}, and the defaults are used.",
                self.dir.display()
            ),
            Err(e) => format!(
                "{name} couldn't be read ({why}) or set aside ({e}). The defaults are used."
            ),
        }
    }

    /// Writes `name` through a temp file and a rename, so a failed save never leaves half a
    /// file.
    pub fn save<T: Serialize>(&self, name: &str, data: &T) -> Result<(), String> {
        let _one_at_a_time = self.saving.lock().expect("store lock poisoned");
        let path = self.dir.join(name);
        let failed = |e: io::Error| format!("{}: {e}", path.display());
        fs::create_dir_all(&self.dir).map_err(failed)?;
        let text = serde_json::to_string_pretty(&Versioned {
            version: VERSION,
            data,
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

    fn input(name: &str, folder: &str) -> ProfileInput {
        ProfileInput {
            name: name.into(),
            folder: folder.into(),
            include_folder: true,
            extensions: Some(vec![
                Some("MP4".into()),
                Some(".xml".into()),
                Some("".into()),
            ]),
        }
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
        assert!(store.load::<Profiles>(PROFILES).0.profiles.is_empty());
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
        let mut profiles = Profiles::default();
        profiles
            .add(input("Sony FX3", "PRIVATE/M4ROOT/CLIP"))
            .unwrap();
        store.save(PROFILES, &profiles).unwrap();
        assert_eq!(store.load::<Profiles>(PROFILES), (profiles, None));
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
            warning.starts_with("settings.json couldn't be read"),
            "{warning}"
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

    #[test]
    fn a_file_from_a_newer_secopy_is_set_aside_too() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SETTINGS),
            br#"{"version": 2, "writeChecksumFile": false}"#,
        )
        .unwrap();
        let (settings, warning) = Store::new(dir.path().to_path_buf()).load::<Settings>(SETTINGS);
        assert_eq!(settings, Settings::default());
        assert!(warning.unwrap().contains("newer Secopy"));
    }

    #[test]
    fn unknown_and_missing_fields_are_fine() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SETTINGS),
            br#"{"version": 1, "showHiddenCount": false, "later": 1}"#,
        )
        .unwrap();
        let (settings, warning) = Store::new(dir.path().to_path_buf()).load::<Settings>(SETTINGS);
        assert_eq!(warning, None);
        assert!(!settings.show_hidden_count && settings.write_checksum_file);
    }

    #[cfg(unix)]
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
                show_hidden_count: false,
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
    fn profiles_are_checked_and_normalized() {
        let mut profiles = Profiles::default();
        let p = profiles
            .add(input("  Sony FX3 ", "PRIVATE//M4ROOT/CLIP/"))
            .unwrap();
        assert_eq!(p.name, "Sony FX3");
        assert_eq!(p.folder, "PRIVATE/M4ROOT/CLIP");
        assert_eq!(
            p.extensions,
            Some(vec![None, Some("mp4".into()), Some("xml".into())])
        );
        assert!(!p.id.is_empty());
        let err = |r: Result<Profile, String>| r.unwrap_err();
        assert_eq!(
            err(profiles.add(input(" ", ""))),
            "The profile needs a name."
        );
        assert_eq!(
            err(profiles.add(input("sony fx3", ""))),
            "There is already a profile called “sony fx3”."
        );
        assert_eq!(
            err(profiles.add(input("A", "/Volumes/CARD"))),
            "The directory is inside the card, so it can't start with “/”."
        );
        assert_eq!(
            err(profiles.add(input("B", "DCIM/../x"))),
            "The directory can't contain “..” or “.”."
        );
        assert_eq!(profiles.profiles.len(), 1);
    }

    #[test]
    fn editing_keeps_the_id_and_deleting_removes() {
        let mut profiles = Profiles::default();
        let a = profiles.add(input("A", "DCIM")).unwrap();
        let b = profiles.add(input("B", "")).unwrap();
        assert_ne!(a.id, b.id);
        let edited = profiles.edit(&a.id, input("A2", "DCIM/100MSDCF")).unwrap();
        assert_eq!(
            (edited.id.as_str(), edited.name.as_str()),
            (a.id.as_str(), "A2")
        );
        assert_eq!(profiles.get(&a.id).unwrap().folder, "DCIM/100MSDCF");
        assert!(
            profiles.edit(&a.id, input("b", "")).is_err(),
            "B's name is taken"
        );
        assert!(
            profiles.edit(&a.id, input("a2", "")).is_ok(),
            "its own name is fine"
        );
        assert!(profiles.delete(&b.id));
        assert!(!profiles.delete(&b.id));
        assert_eq!(profiles.profiles.len(), 1);
        let mut replaced = profiles.get(&a.id).unwrap().clone();
        replaced.include_folder = false;
        profiles.replace(replaced.clone());
        assert_eq!(profiles.get(&a.id), Some(&replaced));
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
    fn a_profile_selects_its_file_types_or_all() {
        let mut p = Profile {
            id: "x".into(),
            name: "A".into(),
            folder: String::new(),
            include_folder: true,
            extensions: Some(vec![Some("mp4".into())]),
        };
        assert!(p.selects(&Some("mp4".into())) && !p.selects(&Some("xml".into())));
        p.extensions = None;
        assert!(p.selects(&Some("anything".into())) && p.selects(&None));
    }
}
