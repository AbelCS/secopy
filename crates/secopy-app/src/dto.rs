//! The data the UI sees (RFD §5.2–§5.4). Plain, serializable summaries: paths are strings
//! (names that aren't UTF-8 are shown lossily), counts are `u32`. Bytes and milliseconds
//! are `u64` exported as a TypeScript `number`, marked field by field: JavaScript numbers
//! are exact up to 2^53, which is 9 PB or 285,000 years.

use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Everything the main window shows. Every session command returns the whole view, so the
/// UI never has to combine partial answers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub source: Option<SourceView>,
    /// Files and bytes the extension filter keeps.
    pub selected_files: u32,
    #[specta(type = specta_typescript::Number)]
    pub selected_bytes: u64,
    pub destination: Option<DestinationView>,
    pub conflicts: ConflictPolicy,
    pub plan: Option<PlanView>,
    /// The selected source profile's id (FR-38).
    pub profile_id: Option<String>,
    /// This run's choices differ from the profile's: offer Update profile / Save as new….
    pub profile_changed: bool,
    /// Why there is no source, e.g. "CARD_A has no PRIVATE/M4ROOT/CLIP".
    pub pick_problem: Option<String>,
    /// What Save as new… suggests for the profile's folder.
    pub suggested_folder: String,
    /// A newer scan replaced this one while it ran (FR-3); the UI keeps its current view.
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    /// What the user picked: the folder, or "3 files".
    pub label: String,
    pub is_folder: bool,
    /// Copy only what's inside the folder (FR-4b) instead of the folder itself.
    pub contents_only: bool,
    /// The picked folder, to scan again when "folder itself / only what's inside" changes.
    pub folder: Option<String>,
    /// The failed files of the last job ("Retry failed"): nothing to choose but the
    /// destination.
    pub is_retry: bool,
    /// The folder created for "copy the folder itself", e.g. "CLIP".
    pub root_dir: Option<String>,
    pub files: u32,
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    /// Sorted by bytes, largest first (FR-8).
    pub extensions: Vec<ExtensionView>,
    /// `None` = every extension; otherwise the selected keys (FR-8).
    pub selected_extensions: Option<Vec<ExtensionKey>>,
    pub skipped_hidden: u32,
    pub skipped_symlinks: u32,
    /// Things that couldn't be read while scanning, first 20.
    pub problems: Vec<String>,
    pub problem_count: u32,
}

/// A lowercase extension without the dot; `None` = files without one (FR-9).
pub type ExtensionKey = Option<String>;

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionView {
    pub key: ExtensionKey,
    /// ".mov", or "(no extension)".
    pub label: String,
    pub files: u32,
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DestinationView {
    pub path: String,
    /// Where the files will land ("Files will go to", FR-4).
    pub copy_root: String,
    /// Stops the job (FR-16); Start stays disabled.
    pub blocker: Option<String>,
    #[specta(type = specta_typescript::Number)]
    pub free_bytes: u64,
    pub fs_kind: String,
    /// Items already in the copy root, hidden ones not counted; `None` if it doesn't exist.
    /// More than zero shows the non-empty warning.
    pub existing_items: Option<u32>,
    /// Files that will fail, first 100 (FR-16).
    pub problems: Vec<FileProblemView>,
    pub problem_count: u32,
    /// Same size and date at the destination: skipped, not checked (FR-17).
    pub identical: u32,
    /// Different files with the same name (FR-17).
    pub differs: u32,
    /// Partial files left by an interrupted copy, to be replaced (FR-18).
    pub stale_partials: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileProblemView {
    pub path: String,
    pub reason: String,
}

/// What to do with files that exist but differ (FR-17).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    #[default]
    KeepBoth,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub files_to_write: u32,
    #[specta(type = specta_typescript::Number)]
    pub bytes_to_write: u64,
    /// Not enough free space (FR-16).
    pub blocker: Option<String>,
}

/// Sent twice a second while a job runs (RFD §5.3, NFR-5).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    pub phase: JobPhase,
    #[specta(type = specta_typescript::Number)]
    pub elapsed_ms: u64,
    pub paused: bool,
    pub verify: bool,
    pub total_files: u32,
    /// Bytes the job writes; skipped files are not included.
    #[specta(type = specta_typescript::Number)]
    pub total_bytes: u64,
    #[specta(type = specta_typescript::Number)]
    pub copied_bytes: u64,
    #[specta(type = specta_typescript::Number)]
    pub verified_bytes: u64,
    pub files_done: u32,
    pub files_skipped: u32,
    pub files_failed: u32,
    /// Files of 8 MiB or more in progress.
    pub active: Vec<ActiveFileView>,
    /// Smaller files in progress, summed into one row.
    pub small_files: Option<SmallFilesView>,
    /// Set once, when the job has stopped for good.
    pub fatal: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobPhase {
    #[default]
    Copying,
    Verifying,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFileView {
    pub id: u32,
    pub name: String,
    pub path: String,
    pub verifying: bool,
    #[specta(type = specta_typescript::Number)]
    pub size: u64,
    #[specta(type = specta_typescript::Number)]
    pub bytes_done: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SmallFilesView {
    pub count: u32,
    #[specta(type = specta_typescript::Number)]
    pub size: u64,
    #[specta(type = specta_typescript::Number)]
    pub bytes_done: u64,
}

/// One row of the finished list (RFD §5.3).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FinishedRow {
    pub id: u32,
    pub path: String,
    /// Differs from `path` when the copy was kept under a new name.
    pub final_path: String,
    #[specta(type = specta_typescript::Number)]
    pub size: u64,
    #[specta(type = specta_typescript::Number)]
    pub millis: u64,
    pub hash: Option<String>,
    pub status: RowStatus,
    /// Why it failed or was skipped.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RowStatus {
    Copied,
    Verified,
    Skipped,
    Failed,
}

/// The summary after a job (RFD §5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SummaryView {
    pub outcome: JobOutcome,
    /// Why the job stopped, for `JobOutcome::Stopped`.
    pub stopped_because: Option<String>,
    pub verify: bool,
    pub files: u32,
    pub copied: u32,
    pub verified: u32,
    pub skipped_identical: u32,
    pub skipped_different: u32,
    pub failed: u32,
    pub not_started: u32,
    #[specta(type = specta_typescript::Number)]
    pub bytes_written: u64,
    #[specta(type = specta_typescript::Number)]
    pub millis: u64,
    /// Failed files with their reasons, first 1,000.
    pub failures: Vec<FinishedRow>,
    /// Rows in the finished list: every file the job got to.
    pub finished: u32,
    pub copy_root: String,
    pub checksum_file: Option<String>,
    pub checksum_error: Option<String>,
    /// The checksum file is off in Settings (RFD §5.5).
    pub checksum_off: bool,
    /// The text report saved in the app's data folder (FR-35).
    pub report_file: Option<String>,
    /// Why the report couldn't be saved there.
    pub report_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobOutcome {
    Complete,
    Failures,
    Cancelled,
    Stopped,
}

/// Paths shown to people: lossy for names that aren't UTF-8.
pub fn show(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Counts shown in the UI; clamped rather than wrapped past `u32::MAX`.
pub fn count(n: impl TryInto<u32>) -> u32 {
    n.try_into().unwrap_or(u32::MAX)
}
