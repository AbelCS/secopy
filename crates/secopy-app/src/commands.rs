//! The commands the UI calls (see `ui/src/lib/bindings.ts`, generated from these). Thin
//! wrappers: the work is in `session` and `jobs`, run off the main thread so the window
//! never freezes (NFR-5).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::dto::{
    ConflictPolicy, ExtensionKey, FinishedRow, ProfilesView, ProgressView, SessionView, StartView,
    SummaryView, show,
};
use crate::jobs::{JobSettings, Jobs, ProgressSink};
use crate::session::{Change, Session, scan_source as scan};
use crate::store::{
    PROFILES, Profile, ProfileInput, Profiles, REMEMBERED, Remembered, SETTINGS, Settings, Store,
    WindowSize,
};

/// Everything the app keeps between commands.
pub struct AppState {
    pub session: Mutex<Session>,
    pub jobs: Jobs,
    pub store: Store,
    pub settings: Mutex<Settings>,
    pub profiles: Mutex<Profiles>,
    pub remembered: Mutex<Remembered>,
    /// Saved files that couldn't be read, handed to the UI once.
    warnings: Mutex<Vec<String>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().expect("app state lock poisoned")
}

impl AppState {
    /// Loads what was saved in `data_dir`; reports go to `data_dir/reports`.
    pub fn new(data_dir: PathBuf) -> Self {
        let store = Store::new(data_dir.clone());
        let (settings, w1) = store.load::<Settings>(SETTINGS);
        let (profiles, w2) = store.load::<Profiles>(PROFILES);
        let (remembered, w3) = store.load::<Remembered>(REMEMBERED);
        let mut session = Session::new();
        // The last profile is selected again (B6); nothing is picked yet, so nothing scans.
        if let Some(p) = remembered
            .last_profile
            .as_deref()
            .and_then(|id| profiles.get(id))
        {
            let _ = session.begin(Change::Profile(Some(p.clone())));
        }
        Self {
            session: Mutex::new(session),
            jobs: Jobs::new(data_dir.join("reports")),
            store,
            settings: Mutex::new(settings),
            profiles: Mutex::new(profiles),
            remembered: Mutex::new(remembered),
            warnings: Mutex::new([w1, w2, w3].into_iter().flatten().collect()),
        }
    }

    pub fn start_view(&self) -> StartView {
        // One lock at a time: a guard in a struct literal lives to the end of it.
        let session = session(self).view();
        let settings = lock(&self.settings).clone();
        let profiles = lock(&self.profiles).profiles.clone();
        let verify = lock(&self.remembered).verify;
        let warnings = std::mem::take(&mut *lock(&self.warnings));
        StartView {
            session,
            settings,
            profiles,
            verify,
            recent_destinations: self.recent(),
            warnings,
        }
    }

    /// The recent destinations that still exist (B7).
    pub fn recent(&self) -> Vec<String> {
        lock(&self.remembered)
            .recent_destinations
            .iter()
            .filter(|d| Path::new(d).is_dir())
            .cloned()
            .collect()
    }

    /// Applies a change to FROM and runs the scan it needs without the session locked.
    pub fn rescan(&self, change: Change) -> SessionView {
        let pending = match session(self).begin(change) {
            Ok(pending) => pending,
            Err(view) => return *view,
        };
        let scanned = scan(&pending.source);
        session(self).finish_scan(pending, scanned)
    }

    /// Changes what the app remembers by itself and saves it; a failed save is only logged.
    pub fn remember(&self, change: impl FnOnce(&mut Remembered)) {
        let mut remembered = lock(&self.remembered);
        change(&mut remembered);
        if let Err(e) = self.store.save(REMEMBERED, &*remembered) {
            eprintln!("Secopy: couldn't save {REMEMBERED}: {e}");
        }
    }

    /// Kept in memory; written when the app quits.
    pub fn window_resized(&self, size: WindowSize) {
        lock(&self.remembered).window = Some(size);
    }

    pub fn saved_window(&self) -> Option<WindowSize> {
        lock(&self.remembered).window
    }

    pub fn select_profile(&self, id: Option<String>) -> Result<SessionView, String> {
        let profile = match &id {
            Some(id) => Some(
                lock(&self.profiles)
                    .get(id)
                    .cloned()
                    .ok_or("That profile no longer exists.")?,
            ),
            None => None,
        };
        self.remember(|r| r.last_profile = id);
        Ok(self.rescan(Change::Profile(profile)))
    }

    /// Changes the profiles and saves them, holding their lock throughout so saves at the
    /// same time can't undo each other; memory changes only when the file is written.
    fn change_profiles<T>(
        &self,
        change: impl FnOnce(&mut Profiles) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut profiles = lock(&self.profiles);
        let mut next = profiles.clone();
        let result = change(&mut next)?;
        self.store
            .save(PROFILES, &next)
            .map_err(|e| format!("Couldn't save the profile: {e}"))?;
        *profiles = next;
        Ok(result)
    }

    fn profiles_view(&self, session: SessionView) -> ProfilesView {
        ProfilesView {
            profiles: lock(&self.profiles).profiles.clone(),
            session,
        }
    }

    /// Update profile: this run's choices go into the selected profile.
    pub fn update_profile(&self) -> Result<ProfilesView, String> {
        let updated = {
            let s = session(self);
            if s.scan_pending() {
                return Err("Wait until the scan finishes.".into());
            }
            s.updated_profile().ok_or("No profile is selected.")?
        };
        self.change_profiles(|p| {
            p.replace(updated.clone());
            Ok(())
        })?;
        let view = session(self).profile_saved(updated);
        Ok(self.profiles_view(view))
    }

    /// Save as new…: this run's choices under a new name, then selected.
    pub fn save_profile_as(&self, name: String, folder: String) -> Result<ProfilesView, String> {
        let (include_folder, extensions) = session(self)
            .choices()
            .ok_or("Wait until the scan finishes.")?;
        let profile = self.change_profiles(|p| {
            p.add(ProfileInput {
                name,
                folder,
                include_folder,
                extensions,
            })
        })?;
        self.remember(|r| r.last_profile = Some(profile.id.clone()));
        let view = self.rescan(Change::Profile(Some(profile)));
        Ok(self.profiles_view(view))
    }

    /// Settings → New.
    pub fn create_profile(&self, input: ProfileInput) -> Result<Vec<Profile>, String> {
        self.change_profiles(|p| p.add(input))?;
        Ok(lock(&self.profiles).profiles.clone())
    }

    /// Settings → Edit; the selected profile is applied again.
    pub fn edit_profile(&self, id: &str, input: ProfileInput) -> Result<ProfilesView, String> {
        let edited = self.change_profiles(|p| p.edit(id, input))?;
        let selected = session(self).profile().is_some_and(|p| p.id == id);
        let view = if selected {
            self.rescan(Change::Profile(Some(edited)))
        } else {
            session(self).view()
        };
        Ok(self.profiles_view(view))
    }

    /// Settings → Delete; a selected profile becomes None.
    pub fn delete_profile(&self, id: &str) -> Result<ProfilesView, String> {
        self.change_profiles(|p| {
            p.delete(id);
            Ok(())
        })?;
        let selected = session(self).profile().is_some_and(|p| p.id == id);
        let view = if selected {
            self.remember(|r| r.last_profile = None);
            self.rescan(Change::Profile(None))
        } else {
            session(self).view()
        };
        Ok(self.profiles_view(view))
    }

    /// Applies at once (the next job uses it) and saves.
    pub fn set_settings(&self, settings: Settings) -> Result<Settings, String> {
        // Held across the save, so the last change in memory is also the last one on disk.
        let mut current = lock(&self.settings);
        *current = settings.clone();
        self.store
            .save(SETTINGS, &*current)
            .map_err(|e| format!("Couldn't save the settings: {e}"))?;
        Ok(settings)
    }

    /// Starts the job with the current settings and remembers the destination (B7).
    pub fn start(&self, verify: bool, sink: impl ProgressSink) -> Result<(), String> {
        let (ready, dest) = {
            let s = session(self);
            let ready = s
                .ready()
                .ok_or("Nothing to copy, or something blocks the copy.")?;
            (ready, s.destination().map(show))
        };
        let settings = JobSettings::from(&*lock(&self.settings));
        self.jobs.start(ready, verify, settings, sink)?;
        if let Some(dest) = dest {
            self.remember(|r| r.used_destination(&dest));
        }
        Ok(())
    }

    /// "Retry failed": the failed files of the last job become the source.
    pub fn retry_failed(&self) -> Result<SessionView, String> {
        let (source, selection) = self.jobs.retry().ok_or("No files failed.")?;
        // With the card pulled out: say so instead of failing every file.
        if let Some(path) = crate::jobs::source_path(&source)
            && !path.exists()
        {
            return Err(format!(
                "The source isn't there any more ({}). Connect the card again to retry.",
                path.display()
            ));
        }
        Ok(session(self).install_retry(source, selection))
    }
}

impl ProgressSink for Channel<ProgressView> {
    fn send(&self, view: ProgressView) {
        // The window may be gone (the app is quitting); the job carries on regardless.
        let _ = Channel::send(self, view);
    }
}

/// Runs `f` on a blocking thread with the app state: scanning and checking touch the disk.
async fn blocking<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())
}

fn session(state: &AppState) -> std::sync::MutexGuard<'_, Session> {
    state.session.lock().expect("session lock poisoned")
}

/// FROM's Choose…: a folder or files, in one panel (FR-1, FR-2). `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn pick_source(app: AppHandle) -> Result<Option<Vec<String>>, String> {
    let (done, picked) = std::sync::mpsc::channel();
    crate::picker::pick_source(&app, done).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || picked.recv().ok().flatten())
        .await
        .map_err(|e| e.to_string())
}

/// Scans a picked, dropped or chosen drive or source (FR-1..FR-3). A newer scan replaces
/// an older one.
#[tauri::command]
#[specta::specta]
pub async fn scan_source(app: AppHandle, paths: Vec<String>) -> Result<SessionView, String> {
    blocking(app, move |state| {
        state.rescan(Change::Pick(paths.into_iter().map(PathBuf::from).collect()))
    })
    .await
}

/// The "Include the folder" checkbox (FR-4); this run's file types stay.
#[tauri::command]
#[specta::specta]
pub async fn set_include_folder(app: AppHandle, include: bool) -> Result<SessionView, String> {
    blocking(app, move |state| {
        state.rescan(Change::IncludeFolder(include))
    })
    .await
}

/// Clears the source; the destination stays ("New copy", RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn clear_source(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| session(state).clear_source()).await
}

/// `None` selects every extension (FR-8, FR-10).
#[tauri::command]
#[specta::specta]
pub async fn set_filter(
    app: AppHandle,
    selected: Option<Vec<ExtensionKey>>,
) -> Result<SessionView, String> {
    blocking(app, move |state| session(state).set_filter(selected)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_destination(app: AppHandle, path: Option<String>) -> Result<SessionView, String> {
    blocking(app, move |state| {
        session(state).set_destination(path.map(PathBuf::from))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_conflicts(app: AppHandle, policy: ConflictPolicy) -> Result<SessionView, String> {
    blocking(app, move |state| session(state).set_policy(policy)).await
}

#[tauri::command]
#[specta::specta]
pub async fn session_view(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| session(state).view()).await
}

/// Starts copying what the main window shows; progress arrives on `on_progress`.
#[tauri::command]
#[specta::specta]
pub async fn start_job(
    app: AppHandle,
    verify: bool,
    on_progress: Channel<ProgressView>,
) -> Result<(), String> {
    blocking(app, move |state| state.start(verify, on_progress)).await?
}

#[tauri::command]
#[specta::specta]
pub fn pause_job(app: AppHandle) {
    app.state::<AppState>().jobs.pause();
}

#[tauri::command]
#[specta::specta]
pub fn resume_job(app: AppHandle) {
    app.state::<AppState>().jobs.resume();
}

#[tauri::command]
#[specta::specta]
pub fn cancel_job(app: AppHandle) {
    app.state::<AppState>().jobs.cancel();
}

#[tauri::command]
#[specta::specta]
pub fn job_running(app: AppHandle) -> bool {
    app.state::<AppState>().jobs.is_running()
}

#[tauri::command]
#[specta::specta]
pub async fn finished_page(
    app: AppHandle,
    offset: u32,
    limit: u32,
    failed_only: bool,
) -> Result<Vec<FinishedRow>, String> {
    blocking(app, move |state| {
        state.jobs.finished_page(offset, limit, failed_only)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn job_summary(app: AppHandle) -> Result<Option<SummaryView>, String> {
    blocking(app, |state| state.jobs.summary()).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_report(app: AppHandle, path: String) -> Result<(), String> {
    blocking(app, move |state| {
        state.jobs.save_report(&PathBuf::from(path))
    })
    .await?
}

/// The UI says which File menu items apply.
#[tauri::command]
#[specta::specta]
pub fn set_menu_state(app: AppHandle, setup: bool, can_start: bool, copying: bool) {
    if let Some(menu) = app.try_state::<crate::FileMenu<tauri::Wry>>() {
        menu.update(setup, can_start, copying);
    }
}

/// "Retry failed": only the failed files, checked again (RFD §5.4).
#[tauri::command]
#[specta::specta]
pub async fn retry_failed(app: AppHandle) -> Result<SessionView, String> {
    blocking(app, |state| state.retry_failed()).await?
}

/// Everything the window needs at start; load problems are handed out once.
#[tauri::command]
#[specta::specta]
pub async fn app_start(app: AppHandle) -> Result<StartView, String> {
    blocking(app, |state| state.start_view()).await
}

#[tauri::command]
#[specta::specta]
pub async fn recent_destinations(app: AppHandle) -> Result<Vec<String>, String> {
    blocking(app, |state| state.recent()).await
}

#[tauri::command]
#[specta::specta]
pub async fn select_profile(app: AppHandle, id: Option<String>) -> Result<SessionView, String> {
    blocking(app, move |state| state.select_profile(id)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn update_profile(app: AppHandle) -> Result<ProfilesView, String> {
    blocking(app, |state| state.update_profile()).await?
}

#[tauri::command]
#[specta::specta]
pub async fn save_profile_as(
    app: AppHandle,
    name: String,
    folder: String,
) -> Result<ProfilesView, String> {
    blocking(app, move |state| state.save_profile_as(name, folder)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn create_profile(app: AppHandle, input: ProfileInput) -> Result<Vec<Profile>, String> {
    blocking(app, move |state| state.create_profile(input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn edit_profile(
    app: AppHandle,
    id: String,
    input: ProfileInput,
) -> Result<ProfilesView, String> {
    blocking(app, move |state| state.edit_profile(&id, input)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn delete_profile(app: AppHandle, id: String) -> Result<ProfilesView, String> {
    blocking(app, move |state| state.delete_profile(&id)).await?
}

#[tauri::command]
#[specta::specta]
pub async fn set_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    blocking(app, move |state| state.set_settings(settings)).await?
}

/// Copy or Copy & Verify, remembered for the next launch (FR-36).
#[tauri::command]
#[specta::specta]
pub async fn set_mode(app: AppHandle, verify: bool) -> Result<(), String> {
    blocking(app, move |state| state.remember(|r| r.verify = verify)).await
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Mutex as StdMutex};

    use super::*;
    use crate::store::{PROFILES, REMEMBERED, SETTINGS};

    #[derive(Clone, Default)]
    struct Sink(Arc<StdMutex<Vec<ProgressView>>>);

    impl ProgressSink for Sink {
        fn send(&self, view: ProgressView) {
            self.0.lock().unwrap().push(view);
        }
    }

    fn input(name: &str, folder: &str) -> ProfileInput {
        ProfileInput {
            name: name.into(),
            folder: folder.into(),
            include_folder: true,
            extensions: None,
        }
    }

    #[test]
    fn a_missing_last_profile_starts_with_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(REMEMBERED),
            r#"{"version": 1, "lastProfile": "gone"}"#,
        )
        .unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let start = state.start_view();
        assert_eq!(start.session.profile_id, None);
        assert!(start.warnings.is_empty());
    }

    #[test]
    fn the_last_profile_and_mode_come_back() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let id = state.create_profile(input("FX3", "DCIM")).unwrap()[0]
            .id
            .clone();
        state.select_profile(Some(id.clone())).unwrap();
        state.remember(|r| r.verify = false);
        let again = AppState::new(dir.path().to_path_buf()).start_view();
        assert_eq!(again.session.profile_id, Some(id));
        assert!(!again.verify);
        assert!(
            again.session.destination.is_none(),
            "the destination is never restored"
        );
        assert_eq!(again.profiles.len(), 1);
    }

    #[test]
    fn a_damaged_file_is_reported_once() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SETTINGS), b"nonsense").unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let first = state.start_view();
        assert_eq!(first.warnings.len(), 1);
        assert!(first.settings.write_checksum_file, "defaults");
        assert!(state.start_view().warnings.is_empty());
    }

    #[test]
    fn deleting_the_selected_profile_selects_none() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let id = state.create_profile(input("FX3", "")).unwrap()[0]
            .id
            .clone();
        state.select_profile(Some(id.clone())).unwrap();
        let after = state.delete_profile(&id).unwrap();
        assert!(after.profiles.is_empty());
        assert_eq!(after.session.profile_id, None);
        assert_eq!(state.remembered.lock().unwrap().last_profile, None);
        let saved = fs::read_to_string(dir.path().join(PROFILES)).unwrap();
        assert!(!saved.contains("FX3"));
    }

    #[test]
    fn editing_the_selected_profile_applies_it() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let id = state.create_profile(input("FX3", "")).unwrap()[0]
            .id
            .clone();
        state.select_profile(Some(id.clone())).unwrap();
        let after = state.edit_profile(&id, input("FX3 A-cam", "CLIP")).unwrap();
        assert_eq!(after.profiles[0].name, "FX3 A-cam");
        assert_eq!(after.session.profile_id, Some(id));
        assert_eq!(
            state.session.lock().unwrap().profile().unwrap().folder,
            "CLIP"
        );
    }

    #[test]
    fn save_as_new_selects_and_saves_the_new_profile() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(card.join("DCIM")).unwrap();
        fs::write(card.join("DCIM/a.jpg"), b"a").unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.rescan(Change::Pick(vec![card.clone()]));
        let after = state
            .save_profile_as("Photos".into(), "DCIM".into())
            .unwrap();
        let id = after.profiles[0].id.clone();
        assert_eq!(after.session.profile_id, Some(id.clone()));
        assert_eq!(
            after.session.source.unwrap().label,
            show(&card.join("DCIM"))
        );
        assert_eq!(state.remembered.lock().unwrap().last_profile, Some(id));
        assert!(
            state
                .save_profile_as("photos".into(), String::new())
                .unwrap_err()
                .contains("already a profile")
        );
    }

    #[test]
    fn starting_a_job_remembers_the_destination_and_uses_the_settings() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("a.mov"), b"a").unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        let state = AppState::new(dir.path().join("data"));
        state
            .set_settings(Settings {
                write_checksum_file: false,
                ..Settings::default()
            })
            .unwrap();
        state.rescan(Change::Pick(vec![card]));
        state
            .session
            .lock()
            .unwrap()
            .set_destination(Some(dest.clone()));
        state.start(true, Sink::default()).unwrap();
        state.jobs.wait();
        assert!(state.jobs.summary().unwrap().checksum_off);
        assert_eq!(state.recent(), [show(&dest)]);
        fs::remove_dir_all(&dest).unwrap();
        assert!(
            state.recent().is_empty(),
            "folders that are gone are left out"
        );
    }

    #[test]
    fn saves_at_the_same_time_all_succeed_and_disk_matches_memory() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let errors = std::thread::scope(|s| {
            let tasks: Vec<_> = (0..16)
                .map(|i| {
                    let state = &state;
                    s.spawn(move || {
                        let settings = Settings {
                            write_checksum_file: i % 2 == 0,
                            show_system_count: i % 3 == 0,
                            report_next_to_checksum: i % 5 == 0,
                            notify_when_done: i % 7 == 0,
                        };
                        let a = state.set_settings(settings).err();
                        let b = state.create_profile(input(&format!("P{i}"), "")).err();
                        a.into_iter().chain(b).collect::<Vec<_>>()
                    })
                })
                .collect();
            tasks
                .into_iter()
                .flat_map(|t| t.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(errors.is_empty(), "{errors:?}");
        let store = Store::new(dir.path().to_path_buf());
        assert_eq!(
            store.load::<Settings>(SETTINGS).0,
            *state.settings.lock().unwrap()
        );
        let saved = store.load::<Profiles>(PROFILES).0;
        assert_eq!(saved.profiles.len(), 16, "no profile lost");
        assert_eq!(saved, *state.profiles.lock().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn retry_after_the_source_is_gone_has_nothing_to_start() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("a.mov"), b"a").unwrap();
        fs::write(card.join("b.mov"), b"b").unwrap();
        fs::set_permissions(card.join("b.mov"), fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(card.join("b.mov")).is_ok() {
            return; // running as root
        }
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.rescan(Change::Pick(vec![card.clone()]));
        state.session.lock().unwrap().set_destination(Some(dest));
        state.start(false, Sink::default()).unwrap();
        state.jobs.wait();
        fs::set_permissions(card.join("b.mov"), fs::Permissions::from_mode(0o644)).unwrap();
        fs::remove_dir_all(&card).unwrap(); // the card was ejected
        let error = state.retry_failed().unwrap_err();
        assert!(
            error.starts_with("The source isn't there any more"),
            "{error}"
        );
        assert!(!state.jobs.is_running(), "nothing started");
    }
}
