//! The data the UI sees (RFD §5.2–§5.4). Plain, serializable summaries: paths are strings
//! (names that aren't UTF-8 are shown lossily), counts are `u32`. Bytes and milliseconds
//! are `u64` exported as a TypeScript `number`, marked field by field: JavaScript numbers
//! are exact up to 2^53, which is 9 PB or 285,000 years.

use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::message::Message;
use crate::store::{CopyPreset, MirrorPreset, Settings};

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
    /// The selected copy preset's id (FR-38).
    pub preset_id: Option<String>,
    /// This run's choices differ from the preset's: offer Update / Save as….
    pub preset_changed: bool,
    /// Why there is no source, e.g. "CARD_A isn't connected."
    pub pick_problem: Option<Message>,
    /// A newer scan replaced this one while it ran (FR-3); the UI keeps its current view.
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    /// What the user picked: the directory, or "3 files".
    pub label: Message,
    pub is_folder: bool,
    /// Copy only what's inside the folder (FR-4b) instead of the folder itself.
    pub contents_only: bool,
    /// The picked folder, to scan again when "folder itself / only what's inside" changes.
    pub folder: Option<String>,
    /// The failed files of the last job ("Retry"): nothing to choose but the
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
    /// System files skipped (`.DS_Store`, `Thumbs.db`…); hidden files are copied (FR-12).
    pub skipped_system: u32,
    pub skipped_symlinks: u32,
    /// Things that couldn't be read while scanning, first 20.
    pub problems: Vec<Message>,
    pub problem_count: u32,
}

/// A lowercase extension without the dot; `None` = files without one (FR-9).
pub type ExtensionKey = Option<String>;

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionView {
    pub key: ExtensionKey,
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
    pub blocker: Option<Message>,
    #[specta(type = specta_typescript::Number)]
    pub free_bytes: u64,
    /// The destination's file system as a code: `apfs`, `hfs`, `exfat`, `fat32`, `ntfs`,
    /// `smb`, `nfs`, or `other` (then `fs_name` says which).
    pub fs_kind: String,
    pub fs_name: Option<String>,
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
    pub reason: Message,
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
    pub blocker: Option<Message>,
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
    /// The job's smaller files, as one steady row; `None` when it has none.
    pub small_files: Option<SmallFilesView>,
    /// Files a mirror is archiving or deleting, while it does (`JobPhase::Removing`).
    pub removing: u32,
    /// Whether they are archived (or deleted).
    pub archiving: bool,
    /// A cancelled job removing the files it copied (Cancel's "Also remove…"), while it does
    /// (`JobPhase::Removing`).
    pub undoing: bool,
    /// Set once, when the job has stopped for good.
    pub fatal: Option<Message>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobPhase {
    #[default]
    Copying,
    Verifying,
    /// A mirror archiving or deleting what's gone from its origin (plan 7).
    Removing,
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
    /// Of the job's small files, those finished (only ever goes up).
    pub done: u32,
    pub total: u32,
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
    pub reason: Option<Message>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RowStatus {
    Copied,
    Verified,
    Skipped,
    Failed,
    /// Stopped by Cancel.
    Cancelled,
    /// A check read it as it was copied (plan 8).
    Intact,
    /// A check read something else than was copied.
    Changed,
    /// A check didn't find it.
    Missing,
}

/// The summary after a job (RFD §5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SummaryView {
    pub outcome: JobOutcome,
    /// Why the job stopped, for `JobOutcome::Stopped`.
    pub stopped_because: Option<Message>,
    pub verify: bool,
    pub files: u32,
    pub copied: u32,
    pub verified: u32,
    pub skipped_identical: u32,
    pub skipped_different: u32,
    pub failed: u32,
    /// Items the scan couldn't read, so they weren't copied (#58).
    pub unread: u32,
    /// The destination reported an error while the copy was made durable (#58).
    pub durability_error: Option<Message>,
    /// Empty directories that couldn't be created (#58); listed with the failures.
    pub dir_errors: u32,
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
    pub checksum_error: Option<Message>,
    /// The checksum file is off in Settings (RFD §5.5).
    pub checksum_off: bool,
    /// The text report saved in the app's data folder (FR-35).
    pub report_file: Option<String>,
    /// Why the report couldn't be saved there (each place it was written).
    pub report_errors: Vec<Message>,
    /// A mirror's own figures (plan 7); `None` for a copy.
    pub mirror: Option<MirrorSummaryView>,
    /// What Cancel's "Also remove the files already copied" did (#54).
    pub undone: Option<UndoneView>,
    /// A check's own figures (plan 8); `None` for a copy or a mirror.
    pub check: Option<CheckSummaryView>,
}

/// A directory about to be checked (plan 8): what its checksum files list.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckView {
    pub directory: String,
    pub checksum_files: u32,
    /// Files the checksum files list.
    pub files: u32,
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    pub not_checked: u32,
    /// Problems in the checksum files: "file:line: why".
    pub problems: Vec<Message>,
}

/// What a check found (plan 8).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckSummaryView {
    pub intact: u32,
    pub changed: u32,
    pub missing: u32,
    /// Couldn't be read.
    pub failed: u32,
    /// Files no checksum file lists.
    pub not_checked: u32,
    pub checksum_files: u32,
    /// Problems in the checksum files: "file:line: why", the first 1,000.
    pub problems: Vec<Message>,
    /// Problems past the ones listed; `None` when every one is (#69).
    #[specta(optional)]
    pub more_problems: Option<u32>,
}

impl CheckSummaryView {
    /// Every problem in the checksum files, listed or not.
    pub fn problem_count(&self) -> u32 {
        count(self.problems.len()).saturating_add(self.more_problems.unwrap_or(0))
    }
}

/// The files a cancelled job removed again (#54).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UndoneView {
    /// Files it had created, removed.
    pub removed: u32,
    /// Files it had replaced that couldn't be put back: the new version stays.
    pub not_restored: u32,
    /// Files that couldn't be removed or put back (the report lists them).
    pub failed: u32,
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

/// What the window loads at start (plan 3b-1).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StartView {
    pub session: SessionView,
    pub settings: Settings,
    pub copy_presets: Vec<CopyPreset>,
    /// Copy & Verify (true) or Copy, as last used (FR-36).
    pub verify: bool,
    /// Recent destinations that still exist, most recent first.
    pub recent_destinations: Vec<String>,
    /// Saved files that couldn't be read; shown once.
    pub warnings: Vec<Message>,
    /// The copy preset last used, when its source is there: the window loads it again (FR-36).
    pub last_preset: Option<String>,
}

/// After a copy preset change: the presets and what FROM shows now.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CopyPresetsView {
    pub presets: Vec<CopyPreset>,
    pub session: SessionView,
}

/// A queued job as the Queue screen shows it (plan 6).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueuedJobView {
    /// "copy", "mirror", or "unknown" for a job a newer Secopy wrote.
    pub kind: String,
    pub verify: bool,
    /// The source as shown ("3 files" for several).
    pub source: Message,
    pub destination: String,
    pub last_error: Option<Message>,
    pub supported: bool,
    /// A mirror's preset name; `None` for a copy.
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueView {
    pub jobs: Vec<QueuedJobView>,
    pub on_failure: crate::queue::OnFailure,
}

/// How a queued job ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum QueueResult {
    Complete,
    Failed,
    Cancelled,
    NotRun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueResultView {
    pub job: QueuedJobView,
    pub result: QueueResult,
    /// Why it failed, was cancelled or didn't run.
    pub reason: Option<Message>,
    /// The job's summary, when it ran.
    pub summary: Option<SummaryView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueSummaryView {
    pub results: Vec<QueueResultView>,
    pub complete: u32,
    pub count: u32,
    #[specta(type = specta_typescript::Number)]
    pub millis: u64,
    /// Why the queue couldn't be saved after a job; the run itself went on.
    pub save_error: Option<Message>,
}

/// What a queue run sends to the window.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QueueEvent {
    /// The job is being checked (its source scanned, the destination looked at).
    JobChecking {
        index: u32,
        count: u32,
    },
    /// A queued mirror's deep check: files compared, of how many.
    Compared {
        index: u32,
        done: u32,
        total: u32,
    },
    /// The checks passed and it runs: a job that can't start never gets this.
    JobStarted {
        index: u32,
        count: u32,
        job: QueuedJobView,
    },
    Progress {
        view: ProgressView,
    },
    Done {
        summary: QueueSummaryView,
    },
}

/// What a mirror did besides copying (FR-52).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorSummaryView {
    /// Files new in the origin, copied.
    pub new: u32,
    /// Files changed in the origin, replaced.
    pub updated: u32,
    /// Files gone from the origin, archived or deleted.
    pub removed: u32,
    /// Removed files were archived (or deleted).
    pub archived: bool,
    /// Files that couldn't be archived or deleted, with why.
    pub removal_failures: Vec<FinishedRow>,
    /// Why nothing was removed: the copy phase failed or was cancelled.
    pub nothing_removed: Option<Message>,
    /// Archived files the user asked to delete that couldn't be (#101), and when they go.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub archive_not_deleted: Option<Message>,
}

/// What a mirror's archive holds (#99, #101): shown on the mirror's screen, and asked before
/// switching it to Delete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveView {
    pub files: u32,
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    /// When its oldest run was archived (RFC 3339).
    pub oldest: Option<String>,
    /// The destination is there: when not, nothing is known about its archive.
    pub connected: bool,
    /// A job or the queue runs: the archive isn't deleted now.
    pub busy: bool,
}

/// What "Delete them now" did (#101).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveDeletedView {
    pub removed: u32,
    /// Archived files that couldn't be deleted, and when they go instead.
    pub not_deleted: Option<Message>,
}

/// How far a preview's deep check is: files compared, of how many (#57).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComparedView {
    pub done: u32,
    pub total: u32,
}

/// A mirror's preview (FR-47): what a run would do, before anything is touched.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorPreviewView {
    pub preset_id: String,
    pub name: String,
    pub origin: String,
    pub destination: String,
    pub new_files: u32,
    #[specta(type = specta_typescript::Number)]
    pub new_bytes: u64,
    pub changed_files: u32,
    #[specta(type = specta_typescript::Number)]
    pub changed_bytes: u64,
    pub removed_files: u32,
    /// Days removed files are archived for; `None` when they are deleted.
    pub archive_days: Option<u32>,
    /// Files that will fail (a name the destination can't take, something in the way).
    pub failing: u32,
    pub unchanged: u32,
    /// Why the run looks wrong (FR-50): the preview's Start asks first.
    pub guard: Option<Message>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PreviewKind {
    New,
    Changed,
    Removed,
}

/// One file in a mirror's preview.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRow {
    pub path: String,
    #[specta(type = specta_typescript::Number)]
    pub size: u64,
    pub kind: PreviewKind,
    pub reason: Message,
}

/// What goes in an export (#77).
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportWhat {
    pub settings: bool,
    pub copy_presets: bool,
    pub mirror_presets: bool,
}

/// After Import: what to say, and everything the window shows, as saved (#77).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportDone {
    pub message: Message,
    /// Part of it couldn't be saved; `message` says what.
    pub failed: bool,
    pub settings: Settings,
    pub copy_presets: Vec<CopyPreset>,
    pub mirror_presets: Vec<MirrorPreset>,
}
