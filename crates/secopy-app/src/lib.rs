//! The Secopy desktop app (RFD §5, milestone M2): a Tauri shell around `secopy-core`.

pub mod dto;
pub mod jobs;
pub mod session;

/// Starts the app. Blocks until the last window closes.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("failed to start Secopy");
}
