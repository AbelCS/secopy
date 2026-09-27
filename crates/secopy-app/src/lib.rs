//! The Secopy desktop app (RFD §5, milestone M2): a Tauri shell around `secopy-core`.

pub mod commands;
pub mod dto;
pub mod jobs;
pub mod session;

use tauri::{Manager, RunEvent};

use commands::AppState;

/// The commands and types the UI sees; `ui/src/lib/bindings.ts` is generated from this.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
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
        .setup(move |app| {
            builder.mount_events(app);
            let reports = app.path().app_data_dir()?.join("reports");
            app.manage(AppState::new(reports));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start Secopy")
        .run(|app, event| {
            // Quitting during a copy: stop it first, so no partial file is left behind.
            if let RunEvent::ExitRequested { .. } = event {
                let jobs = &app.state::<AppState>().jobs;
                jobs.cancel();
                jobs.wait();
            }
        });
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The UI's bindings must match the Rust commands. Set `SECOPY_UPDATE_BINDINGS=1` to
    /// rewrite them after changing a command or a DTO.
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
