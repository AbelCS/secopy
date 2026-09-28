//! The Secopy desktop app (RFD §5, milestone M2): a Tauri shell around `secopy-core`.

pub mod commands;
pub mod dto;
pub mod jobs;
mod migrate;
pub mod mirrors;
mod picker;
pub mod queue;
pub mod session;
pub mod store;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, Runtime, WindowEvent};

use store::WindowSize;

use commands::AppState;

/// The commands and types the UI sees; `ui/src/lib/bindings.ts` is generated from this.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::pick_source,
        commands::scan_source,
        commands::set_include_folder,
        commands::app_start,
        commands::recent_destinations,
        commands::select_profile,
        commands::update_profile,
        commands::save_profile_as,
        commands::create_profile,
        commands::edit_profile,
        commands::delete_profile,
        commands::set_settings,
        commands::set_mode,
        commands::clear_source,
        commands::set_filter,
        commands::set_destination,
        commands::set_conflicts,
        commands::session_view,
        commands::start_job,
        commands::pause_job,
        commands::resume_job,
        commands::cancel_job,
        commands::job_running,
        commands::finished_page,
        commands::job_summary,
        commands::save_report,
        commands::retry_failed,
        commands::set_menu_state,
        commands::queue,
        commands::add_to_queue,
        commands::remove_from_queue,
        commands::move_in_queue,
        commands::clear_queue,
        commands::set_queue_on_failure,
        commands::run_queue,
        commands::queue_finished_page,
        commands::queue_save_report,
        commands::mirror_presets,
        commands::create_mirror_preset,
        commands::edit_mirror_preset,
        commands::delete_mirror_preset,
        commands::preview_mirror,
        commands::mirror_preview_page,
        commands::run_mirror,
        commands::cancel_mirror_preview,
        commands::add_mirror_to_queue,
    ])
}

/// Writes the TypeScript bindings for the UI.
pub fn export_bindings(path: &std::path::Path) -> Result<(), String> {
    specta_builder()
        .export(
            specta_typescript::Typescript::default().header(
                "// Generated from crates/secopy-app by `cargo test -p secopy-app`. Don't edit.\n",
            ),
            path,
        )
        .map_err(|e| e.to_string())
}

/// Starts the app. Blocks until it quits.
pub fn run() {
    let builder = specta_builder();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(builder.invoke_handler())
        .menu(menu)
        .on_menu_event(|app, event| {
            if event.id() == QUIT {
                quit(app);
            } else if event.id() == SETTINGS_MENU {
                let _ = app.emit(OPEN_SETTINGS, ());
            } else if let Some(item) = MENU_ITEMS.into_iter().find(|id| event.id() == *id) {
                let _ = app.emit(MENU_EVENT, item);
            }
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Resized(size) = event {
                let scale = window.scale_factor().unwrap_or(1.0);
                let size = size.to_logical::<f64>(scale);
                // Minimizing reports a tiny size; keep the last real one.
                if size.width >= MIN_WIDTH
                    && size.height >= MIN_HEIGHT
                    && let Some(state) = window.try_state::<AppState>()
                {
                    state.window_resized(WindowSize {
                        width: size.width,
                        height: size.height,
                    });
                }
            }
        })
        .setup(move |app| {
            builder.mount_events(app);
            let data = app.path().app_data_dir()?;
            // Reports saved by 0.2.0 under its old identifier (RFD §14): moved once.
            if let Some(parent) = data.parent() {
                let old = parent.join(migrate::OLD_IDENTIFIER);
                if let Err(e) = migrate::move_old_reports(&old, &data) {
                    eprintln!("Secopy: the 0.2.0 reports stay in {}: {e}", old.display());
                }
            }
            app.manage(AppState::new(data));
            if let Some(window) = app.get_webview_window("main") {
                if let Some(saved) = app.state::<AppState>().saved_window() {
                    let screen = window.current_monitor().ok().flatten().map(|m| {
                        let size = m.size().to_logical::<f64>(m.scale_factor());
                        (size.width, size.height)
                    });
                    let (width, height) = window_size(saved, screen);
                    let _ = window.set_size(tauri::LogicalSize::new(width, height));
                }
                let _ = window.show();
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start Secopy")
        .run(|app, event| {
            // The app is going away: stop the copy first, so no partial file is left behind.
            // `Exit` also covers quitting from the Dock or at logout, which can't be refused.
            if let RunEvent::ExitRequested { .. } | RunEvent::Exit = event {
                let state = app.state::<AppState>();
                state.remember(|_| {}); // writes the window size
                state.cancel(false); // quitting keeps the files already copied
                state.jobs.wait();
                if let Some(thread) = state
                    .queue_run
                    .lock()
                    .ok()
                    .and_then(|mut r| r.thread.take())
                {
                    let _ = thread.join();
                }
            }
        });
}

const QUIT: &str = "quit";
const SETTINGS_MENU: &str = "settings";
/// Asks the UI to show Settings (Secopy → Settings…).
pub const OPEN_SETTINGS: &str = "open-settings";

/// File menu items, and the event that tells the UI one was chosen.
const CHOOSE_SOURCE: &str = "choose-source";
const CHOOSE_DESTINATION: &str = "choose-destination";
const START_COPY: &str = "start-copy";
const CANCEL_COPY: &str = "cancel-copy";
pub const MENU_EVENT: &str = "menu";
const SHOW_COPY: &str = "show-copy";
const SHOW_MIRROR: &str = "show-mirror";
const SHOW_QUEUE: &str = "show-queue";
/// Menu items the window handles (File and View).
const MENU_ITEMS: [&str; 7] = [
    CHOOSE_SOURCE,
    CHOOSE_DESTINATION,
    START_COPY,
    CANCEL_COPY,
    SHOW_COPY,
    SHOW_MIRROR,
    SHOW_QUEUE,
];

/// The File menu's items, kept to grey them out.
pub struct FileMenu<R: Runtime> {
    items: [MenuItem<R>; 4],
}

impl<R: Runtime> FileMenu<R> {
    pub fn update(&self, setup: bool, can_start: bool, copying: bool) {
        for (item, on) in self.items.iter().zip(menu_state(setup, can_start, copying)) {
            let _ = item.set_enabled(on);
        }
    }
}

/// Which File items apply: [Choose Source, Choose Destination, Start Copy, Cancel Copy].
fn menu_state(setup: bool, can_start: bool, copying: bool) -> [bool; 4] {
    [setup, setup, setup && can_start, copying]
}

/// The window's minimum size (`tauri.conf.json`).
const MIN_WIDTH: f64 = 720.0;
const MIN_HEIGHT: f64 = 560.0;

/// The saved window size, kept within the minimum and the screen.
fn window_size(saved: WindowSize, screen: Option<(f64, f64)>) -> (f64, f64) {
    let (max_w, max_h) = screen.unwrap_or((f64::MAX, f64::MAX));
    (
        saved.width.clamp(MIN_WIDTH, max_w.max(MIN_WIDTH)),
        saved.height.clamp(MIN_HEIGHT, max_h.max(MIN_HEIGHT)),
    )
}

#[derive(Debug, PartialEq)]
enum Quit {
    /// Close the window instead: the UI asks "Stop copying and quit?" first.
    AskFirst,
    Now,
}

fn quit_action(copying: bool) -> Quit {
    if copying { Quit::AskFirst } else { Quit::Now }
}

/// Quit Secopy (⌘Q): during a copy it asks first, like closing the window.
fn quit<R: Runtime>(app: &AppHandle<R>) {
    let copying = asks_before_quitting(&app.state::<AppState>());
    match (quit_action(copying), app.get_webview_window("main")) {
        (Quit::AskFirst, Some(window)) => {
            let _ = window.close();
        }
        _ => app.exit(0),
    }
}

/// A copy runs, or the queue does (even between two of its jobs).
fn asks_before_quitting(state: &AppState) -> bool {
    state.busy()
}

/// The standard macOS menu, except Quit: the standard one ends the app at once, without
/// the chance to ask.
fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let app_menu = Submenu::with_items(
        app,
        "Secopy",
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, SETTINGS_MENU, "Settings…", true, Some("CmdOrCtrl+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit Secopy", true, Some("CmdOrCtrl+Q"))?,
        ],
    )?;
    let source = MenuItem::with_id(
        app,
        CHOOSE_SOURCE,
        "Choose Source…",
        true,
        Some("CmdOrCtrl+O"),
    )?;
    let destination = MenuItem::with_id(
        app,
        CHOOSE_DESTINATION,
        "Choose Destination…",
        true,
        Some("CmdOrCtrl+D"),
    )?;
    let start = MenuItem::with_id(
        app,
        START_COPY,
        "Start Copy",
        false,
        Some("CmdOrCtrl+Enter"),
    )?;
    let cancel = MenuItem::with_id(
        app,
        CANCEL_COPY,
        "Cancel Copy",
        false,
        Some("CmdOrCtrl+Period"),
    )?;
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &source,
            &destination,
            &PredefinedMenuItem::separator(app)?,
            &start,
            &cancel,
        ],
    )?;
    app.manage(FileMenu {
        items: [source, destination, start, cancel],
    });
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &MenuItem::with_id(app, SHOW_COPY, "Copy", true, Some("CmdOrCtrl+1"))?,
            &MenuItem::with_id(app, SHOW_MIRROR, "Mirror", true, Some("CmdOrCtrl+2"))?,
            &MenuItem::with_id(app, SHOW_QUEUE, "Queue", true, Some("CmdOrCtrl+3"))?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &file, &view, &edit, &window])
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Quit, quit_action};

    /// ⌘Q asks between queue jobs too (a scan, or a job that couldn't start), like closing.
    #[test]
    fn quitting_asks_while_the_queue_runs_between_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::commands::AppState::new(dir.path().to_path_buf());
        assert!(!super::asks_before_quitting(&state));
        state.queue_run.lock().unwrap().running = true;
        assert!(super::asks_before_quitting(&state));
    }

    #[test]
    fn the_view_menu_items_reach_the_window() {
        assert!(
            super::MENU_ITEMS.contains(&"show-copy") && super::MENU_ITEMS.contains(&"show-queue")
        );
    }

    #[test]
    fn the_file_menu_offers_only_what_applies() {
        use super::menu_state;
        assert_eq!(menu_state(true, false, false), [true, true, false, false]);
        assert_eq!(menu_state(true, true, false), [true, true, true, false]);
        assert_eq!(menu_state(false, false, true), [false, false, false, true]);
        assert_eq!(
            menu_state(false, true, false),
            [false, false, false, false],
            "Start only on New copy"
        );
    }

    fn capability() -> serde_json::Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// The UI's close handler ends by destroying the window; without this permission the
    /// window can't be closed at all.
    #[test]
    fn the_window_may_close_itself() {
        let permissions = capability()["permissions"].clone();
        assert!(
            permissions
                .as_array()
                .unwrap()
                .contains(&"core:window:allow-destroy".into()),
            "{permissions}"
        );
    }

    /// Without a signature over the whole bundle, macOS calls a downloaded app "damaged"
    /// and offers no Open Anyway (#36). Ad-hoc ("-") until it is signed with a Developer ID.
    #[test]
    fn the_bundle_is_signed_as_a_whole() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
        let conf: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(conf["bundle"]["macOS"]["signingIdentity"], "-");
    }

    /// The UI only opens checksum files, so that's all it may open.
    #[test]
    fn only_checksum_files_can_be_opened() {
        let permissions = capability()["permissions"].clone();
        let open = permissions
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["identifier"] == "opener:allow-open-path")
            .unwrap();
        let pattern = glob::Pattern::new(open["allow"][0]["path"].as_str().unwrap()).unwrap();
        // How Tauri matches path scopes on Unix.
        let options = glob::MatchOptions {
            require_literal_separator: true,
            require_literal_leading_dot: true,
            ..Default::default()
        };
        let allowed = |p: &str| pattern.matches_path_with(Path::new(p), options);
        assert!(allowed(
            "/Volumes/RAID/Day01/secopy_2026-09-27_140302.xxh64"
        ));
        assert!(allowed(
            "/Users/me/Movies/CARD/secopy_2026-09-27_140302.xxh64"
        ));
        assert!(!allowed("/Users/me/Documents/notes.txt"));
        assert!(!allowed("/Applications/Calculator.app"));
    }

    #[test]
    fn quitting_during_a_copy_asks_first() {
        assert_eq!(quit_action(true), Quit::AskFirst);
        assert_eq!(quit_action(false), Quit::Now);
    }

    /// The UI's bindings must match the Rust commands. Set `SECOPY_UPDATE_BINDINGS=1` to
    /// rewrite them after changing a command or a DTO.
    #[test]
    fn the_saved_window_size_is_kept_within_the_minimum_and_the_screen() {
        use super::window_size;
        use crate::store::WindowSize;
        let saved = |width, height| WindowSize { width, height };
        assert_eq!(
            window_size(saved(1200.0, 900.0), Some((1440.0, 900.0))),
            (1200.0, 900.0)
        );
        assert_eq!(window_size(saved(300.0, 200.0), None), (720.0, 560.0));
        assert_eq!(
            window_size(saved(3000.0, 2000.0), Some((1440.0, 900.0))),
            (1440.0, 900.0)
        );
    }

    #[test]
    fn the_app_identifier_is_latecommits() {
        let conf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
        let conf: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(conf).unwrap()).unwrap();
        assert_eq!(conf["identifier"], "com.latecommits.secopy");
    }

    #[test]
    fn ui_bindings_are_up_to_date() {
        let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/lib/bindings.ts");
        if std::env::var("SECOPY_UPDATE_BINDINGS").is_ok_and(|v| v == "1") {
            super::export_bindings(&committed).unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("bindings.ts");
        super::export_bindings(&fresh).unwrap();
        assert_eq!(
            std::fs::read_to_string(&committed).unwrap_or_default(),
            std::fs::read_to_string(&fresh).unwrap(),
            "ui/src/lib/bindings.ts is out of date: run SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app"
        );
    }
}
