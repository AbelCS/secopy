//! The Secopy desktop app (RFD §5, milestone M2): a Tauri shell around `secopy-core`.

pub mod commands;
pub mod dto;
pub mod jobs;
mod migrate;
mod picker;
pub mod session;
pub mod store;
pub mod volumes;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, RunEvent, Runtime};

use commands::AppState;

/// The commands and types the UI sees; `ui/src/lib/bindings.ts` is generated from this.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::pick_source,
        commands::scan_source,
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
        .invoke_handler(builder.invoke_handler())
        .menu(menu)
        .on_menu_event(|app, event| {
            if event.id() == QUIT {
                quit(app);
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
            app.manage(AppState::new(data.join("reports")));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start Secopy")
        .run(|app, event| {
            // The app is going away: stop the copy first, so no partial file is left behind.
            // `Exit` also covers quitting from the Dock or at logout, which can't be refused.
            if let RunEvent::ExitRequested { .. } | RunEvent::Exit = event {
                let jobs = &app.state::<AppState>().jobs;
                jobs.cancel();
                jobs.wait();
            }
        });
}

const QUIT: &str = "quit";

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
    let copying = app.state::<AppState>().jobs.is_running();
    match (quit_action(copying), app.get_webview_window("main")) {
        (Quit::AskFirst, Some(window)) => {
            let _ = window.close();
        }
        _ => app.exit(0),
    }
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
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit Secopy", true, Some("CmdOrCtrl+Q"))?,
        ],
    )?;
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
    Menu::with_items(app, &[&app_menu, &edit, &window])
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Quit, quit_action};

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
