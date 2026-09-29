//! The main window's state (RFD §5.2): source → selection → destination check → plan.
//! Every change recomputes what depends on it and returns one [`SessionView`].

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use secopy_core::filter::ExtensionFilter;
use secopy_core::fsinfo::{self, FsKind};
use secopy_core::plan::{DiffersPolicy, Plan};
use secopy_core::preflight::{Blocker, ConflictKind, Preflight, preflight};
use secopy_core::scan::{self, Scan, ScanOptions, Selection};
use secopy_core::source::{DirMode, Source};

use crate::dto::{
    ConflictPolicy, DestinationView, ExtensionKey, ExtensionView, FileProblemView, PlanView,
    SessionView, SourceView, count, show,
};
use crate::message::Message;
use crate::msg;
use crate::say;
use crate::store::CopyPreset;

/// Where macOS mounts drives.
const VOLUMES: &str = "/Volumes";

/// Per-file problems sent to the UI; the rest are only counted.
const PROBLEMS_SHOWN: usize = 100;
/// Scan problems sent to the UI.
const SCAN_PROBLEMS_SHOWN: usize = 20;

pub struct Session {
    /// Increases with every change to FROM; only the newest scan's result is kept (FR-3).
    generation: u64,
    /// The change the shown source and this run's choices belong to; behind `generation`
    /// while a scan is pending.
    source_generation: u64,
    /// The file types the pending scan starts with.
    next_filter: ExtensionFilter,
    /// What the user picked: a drop or Choose…. The source is this plus the
    /// preset's folder (spec B3).
    picked: Option<Vec<PathBuf>>,
    preset: Option<CopyPreset>,
    /// "Include the folder" for this run (FR-4).
    include_folder: bool,
    /// Why the pick has no source.
    pick_problem: Option<Message>,
    source: Option<Picked>,
    filter: ExtensionFilter,
    selection: Option<Selection>,
    dest: Option<PathBuf>,
    policy: ConflictPolicy,
    checked: Option<Result<Preflight, Blocker>>,
    plan: Option<Plan>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            generation: 0,
            source_generation: 0,
            next_filter: ExtensionFilter::All,
            picked: None,
            preset: None,
            include_folder: true,
            pick_problem: None,
            source: None,
            filter: ExtensionFilter::All,
            selection: None,
            dest: None,
            policy: ConflictPolicy::default(),
            checked: None,
            plan: None,
        }
    }
}

/// A change to FROM; each one rescans.
pub enum Change {
    /// A new pick: a drop or Choose….
    Pick(Vec<PathBuf>),
    /// The "Include the folder" checkbox; keeps this run's file types.
    IncludeFolder(bool),
    /// A copy preset selected, or `None`.
    CopyPreset(Option<CopyPreset>),
}

/// A scan to run without the session locked, then hand to [`Session::finish_scan`].
pub struct PendingScan {
    ticket: u64,
    pub source: Source,
    filter: ExtensionFilter,
}

/// Everything a job needs from the main window.
pub struct Ready {
    pub source: Source,
    pub plan: Plan,
    /// The source as shown, for the report (English).
    pub label: String,
    /// The source as the UI shows it.
    pub shown: Message,
    /// A mirror's preset name; `None` for a copy.
    pub mirror: Option<String>,
    /// Where the files land, for Reveal in Finder.
    pub copy_root: PathBuf,
}

struct Picked {
    /// In English, for the report.
    label: String,
    /// As the UI shows it.
    shown: Message,
    is_retry: bool,
    source: Source,
    scan: Scan,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// One folder → a folder source; otherwise only files (RFD Q6).
    /// Starts afresh, as after a panic while the session was held. The generation keeps
    /// counting up, so a scan begun before stays stale.
    pub fn restart(&mut self) {
        let generation = self.generation + 1;
        *self = Session {
            generation,
            source_generation: generation,
            ..Session::new()
        };
    }

    pub fn source_for(paths: &[PathBuf], contents_only: bool) -> Result<Source, Message> {
        match paths {
            [] => Err(msg!("copy.pick.nothing")),
            [only] if only.is_dir() => Ok(Source::Directory {
                path: only.clone(),
                mode: if contents_only {
                    DirMode::ContentsOnly
                } else {
                    DirMode::FolderItself
                },
            }),
            _ if paths.iter().any(|p| p.is_dir()) => Err(msg!("copy.pick.mixed")),
            _ => Ok(Source::Files(paths.to_vec())),
        }
    }

    /// Applies `change` and returns the scan it needs, or the view when there is nothing to
    /// scan: nothing picked yet, or the pick can't be used (the preset's folder is missing,
    /// a folder and files together).
    pub fn begin(&mut self, change: Change) -> Result<PendingScan, Box<SessionView>> {
        let keep_filter = matches!(change, Change::IncludeFolder(_));
        // A toggle during a pending scan keeps that scan's file types, not the old card's.
        let kept = if self.scan_pending() {
            self.next_filter.clone()
        } else {
            self.filter.clone()
        };
        match change {
            Change::Pick(paths) => {
                self.picked = Some(paths);
                self.include_folder = self.preset.as_ref().is_none_or(|p| p.include_folder);
            }
            Change::IncludeFolder(include) => self.include_folder = include,
            Change::CopyPreset(preset) => {
                if let Some(p) = &preset {
                    self.include_folder = p.include_folder;
                    // Choosing a preset loads its source (one without keeps what's picked).
                    if !p.source.is_empty() {
                        self.picked = Some(vec![PathBuf::from(&p.source)]);
                    }
                }
                self.preset = preset;
            }
        }
        self.generation += 1;
        let Some(paths) = self.picked.clone() else {
            self.source_generation = self.generation;
            return Err(Box::new(self.view()));
        };
        match self.resolve(&paths) {
            Ok(source) => {
                let filter = if keep_filter {
                    kept
                } else {
                    self.preset_filter(&source)
                };
                self.next_filter = filter.clone();
                Ok(PendingScan {
                    ticket: self.generation,
                    source,
                    filter,
                })
            }
            Err(problem) => {
                self.no_source(problem);
                self.source_generation = self.generation;
                Err(Box::new(self.view()))
            }
        }
    }

    pub fn finish_scan(
        &mut self,
        pending: PendingScan,
        scanned: Result<Scan, Message>,
    ) -> SessionView {
        if pending.ticket != self.generation {
            return SessionView {
                stale: true,
                ..self.view()
            };
        }
        self.source_generation = pending.ticket;
        match scanned {
            Ok(scan) => {
                self.pick_problem = None;
                self.source = Some(Picked {
                    label: label(&pending.source),
                    shown: shown(&pending.source),
                    is_retry: false,
                    source: pending.source,
                    scan,
                });
                self.filter = pending.filter;
                self.recompute();
            }
            Err(problem) => self.no_source(problem),
        }
        self.view()
    }

    fn no_source(&mut self, problem: Message) {
        self.pick_problem = Some(problem);
        self.source = None;
        self.filter = ExtensionFilter::All;
        self.recompute();
    }

    /// The source for `paths`: one folder, otherwise files. A preset's source that is
    /// gone (its card isn't inserted) says so instead.
    fn resolve(&self, paths: &[PathBuf]) -> Result<Source, Message> {
        if let [one] = paths
            && !one.exists()
        {
            return Err(gone(one));
        }
        Self::source_for(paths, !self.include_folder)
    }

    /// The filter a new pick or preset starts with: the preset's file types for a folder.
    fn preset_filter(&self, source: &Source) -> ExtensionFilter {
        match (&self.preset, source) {
            (
                Some(CopyPreset {
                    extensions: Some(keys),
                    ..
                }),
                Source::Directory { .. },
            ) => ExtensionFilter::Only(keys.iter().cloned().collect()),
            _ => ExtensionFilter::All,
        }
    }

    pub fn preset(&self) -> Option<&CopyPreset> {
        self.preset.as_ref()
    }

    pub fn destination(&self) -> Option<&Path> {
        self.dest.as_deref()
    }

    /// This run's include-folder choice and file types, for Save as….
    pub fn choices(&self) -> Option<(bool, Option<Vec<ExtensionKey>>)> {
        if self.scan_pending() {
            return None;
        }
        let extensions = match &self.filter {
            ExtensionFilter::All => None,
            ExtensionFilter::Only(keys) => Some(keys.iter().cloned().collect()),
        };
        Some((self.include_folder, extensions))
    }

    /// A change to FROM is still being scanned.
    pub fn scan_pending(&self) -> bool {
        self.generation != self.source_generation
    }

    /// Whether this run's choices differ from the selected preset's.
    fn preset_changed(&self) -> bool {
        let (Some(preset), Some(picked)) = (&self.preset, &self.source) else {
            return false;
        };
        if picked.is_retry || !matches!(picked.source, Source::Directory { .. }) {
            return false;
        }
        !self
            .picked_source()
            .is_some_and(|picked| same_dir(Path::new(&picked), Path::new(&preset.source)))
            || preset.include_folder != self.include_folder
            || picked
                .scan
                .ext_stats
                .keys()
                .any(|key| preset.selects(key) != self.filter.matches(key))
    }

    /// The selected preset with this run's choices (Update). File types the preset
    /// lists that aren't on this card are kept.
    pub fn updated_preset(&self) -> Option<CopyPreset> {
        if self.scan_pending() {
            return None;
        }
        let preset = self.preset.as_ref()?;
        let picked = self.source.as_ref()?;
        let present: Vec<&ExtensionKey> = picked.scan.ext_stats.keys().collect();
        let extensions =
            if preset.extensions.is_none() && present.iter().all(|k| self.filter.matches(k)) {
                None
            } else {
                let mut keys: BTreeSet<ExtensionKey> = preset
                    .extensions
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|k| !present.contains(&k))
                    .collect();
                keys.extend(
                    present
                        .into_iter()
                        .filter(|k| self.filter.matches(k))
                        .cloned(),
                );
                Some(keys.into_iter().collect())
            };
        Some(CopyPreset {
            source: self
                .picked_source()
                .unwrap_or_else(|| preset.source.clone()),
            include_folder: self.include_folder,
            extensions,
            ..preset.clone()
        })
    }

    /// The selected preset was saved (Update, or edited in Copy presets).
    pub fn preset_saved(&mut self, preset: CopyPreset) -> SessionView {
        self.preset = Some(preset);
        self.view()
    }

    /// This setup as a queued job (FR-39); `None` unless Start would start it. A
    /// retry can't be queued: it copies a failed job's files, not a source.
    pub fn copy_job(&self, verify: bool) -> Option<crate::queue::CopyJob> {
        self.ready()?;
        let picked = self.source.as_ref()?;
        if picked.is_retry {
            return None;
        }
        Some(crate::queue::CopyJob {
            sources: self.picked.clone()?,
            include_folder: self.include_folder,
            extensions: match &self.filter {
                ExtensionFilter::All => None,
                ExtensionFilter::Only(keys) => Some(keys.iter().cloned().collect()),
            },
            destination: self.dest.clone()?,
            conflicts: self.policy,
            verify,
        })
    }

    /// The picked directory, which a preset saves as its source; `None` for files.
    pub fn picked_source(&self) -> Option<String> {
        match self.picked.as_deref() {
            Some([one]) if one.is_dir() => Some(show(one)),
            _ => None,
        }
    }

    /// For "Retry": the failed files of the last job, checked again (RFD §5.4).
    pub fn install_retry(&mut self, source: Source, selection: Selection) -> SessionView {
        self.generation += 1;
        self.source_generation = self.generation;
        // The same folder the first job created, as the scan would have named it.
        let root_dir = match &source {
            Source::Directory {
                path,
                mode: DirMode::FolderItself,
            } => path.file_name().map(PathBuf::from),
            _ => None,
        };
        let scan = Scan {
            files: selection.files.clone(),
            root_dir,
            ..Scan::default()
        };
        self.source = Some(Picked {
            label: format!("Retry: {} failed files", selection.files.len()),
            shown: msg!("copy.picked.retry", count = selection.files.len()),
            is_retry: true,
            source,
            scan,
        });
        self.filter = ExtensionFilter::All;
        self.pick_problem = None;
        self.selection = Some(selection);
        self.recheck();
        self.view()
    }

    pub fn clear_source(&mut self) -> SessionView {
        self.generation += 1;
        self.source_generation = self.generation;
        self.picked = None;
        self.pick_problem = None;
        self.source = None;
        self.filter = ExtensionFilter::All;
        self.recompute();
        self.view()
    }

    /// `None` selects every extension, which also recreates empty folders (FR-10).
    pub fn set_filter(&mut self, selected: Option<Vec<ExtensionKey>>) -> SessionView {
        self.filter = match selected {
            None => ExtensionFilter::All,
            Some(keys) => ExtensionFilter::Only(keys.into_iter().collect::<BTreeSet<_>>()),
        };
        self.recompute();
        self.view()
    }

    pub fn set_destination(&mut self, dest: Option<PathBuf>) -> SessionView {
        self.dest = dest;
        self.recheck();
        self.view()
    }

    pub fn set_policy(&mut self, policy: ConflictPolicy) -> SessionView {
        self.policy = policy;
        self.replan();
        self.view()
    }

    /// What a job starts with; `None` while anything blocks Start.
    pub fn ready(&self) -> Option<Ready> {
        let picked = self.source.as_ref()?;
        let plan = self.plan.as_ref()?;
        (plan.blockers().is_empty() && !plan.files.is_empty()).then(|| Ready {
            source: picked.source.clone(),
            plan: plan.clone(),
            label: picked.label.clone(),
            shown: picked.shown.clone(),
            mirror: None,
            copy_root: self.copy_root(&plan.dest),
        })
    }

    fn recompute(&mut self) {
        self.selection = self.source.as_ref().map(|p| p.scan.select(&self.filter));
        self.recheck();
    }

    fn recheck(&mut self) {
        self.checked = match (&self.source, &self.selection, &self.dest) {
            (Some(picked), Some(sel), Some(dest)) => Some(preflight(&picked.source, sel, dest)),
            _ => None,
        };
        self.replan();
    }

    fn replan(&mut self) {
        self.plan = match (&self.selection, &self.checked) {
            (Some(sel), Some(Ok(pf))) => Some(Plan::resolve(sel, pf, policy(self.policy))),
            _ => None,
        };
    }

    pub fn view(&self) -> SessionView {
        let selection = self.selection.as_ref();
        SessionView {
            source: self.source.as_ref().map(|p| self.source_view(p)),
            selected_files: selection.map_or(0, |s| count(s.files.len())),
            selected_bytes: selection.map_or(0, |s| s.total_bytes),
            destination: self.dest.as_ref().map(|d| self.destination_view(d)),
            conflicts: self.policy,
            plan: self.plan.as_ref().map(plan_view),
            stale: false,
            preset_id: self.preset.as_ref().map(|p| p.id.clone()),
            preset_changed: self.preset_changed(),
            pick_problem: self.pick_problem.clone(),
        }
    }

    fn source_view(&self, picked: &Picked) -> SourceView {
        let scan = &picked.scan;
        let mut extensions: Vec<ExtensionView> = scan
            .ext_stats
            .iter()
            .map(|(key, stat)| ExtensionView {
                key: key.clone(),
                files: count(stat.files),
                bytes: stat.bytes,
            })
            .collect();
        extensions.sort_by_key(|e| std::cmp::Reverse(e.bytes));
        let (folder, contents_only) = match &picked.source {
            Source::Directory { path, mode } => (Some(show(path)), *mode == DirMode::ContentsOnly),
            Source::Files(_) => (None, false),
        };
        SourceView {
            label: picked.shown.clone(),
            is_folder: folder.is_some(),
            contents_only,
            folder,
            is_retry: picked.is_retry,
            root_dir: scan.root_dir.as_deref().map(show),
            files: count(scan.files.len()),
            bytes: scan.files.iter().map(|f| f.size).sum(),
            extensions,
            selected_extensions: match &self.filter {
                ExtensionFilter::All => None,
                ExtensionFilter::Only(keys) => Some(
                    scan.ext_stats
                        .keys()
                        .filter(|k| keys.contains(*k))
                        .cloned()
                        .collect(),
                ),
            },
            skipped_system: count(scan.skipped_system),
            skipped_symlinks: count(scan.skipped_symlinks.len()),
            problems: scan
                .problems
                .iter()
                .take(SCAN_PROBLEMS_SHOWN)
                .map(say::scan_problem)
                .collect(),
            problem_count: count(scan.problems.len()),
        }
    }

    fn destination_view(&self, dest: &Path) -> DestinationView {
        let copy_root = self.copy_root(dest);
        let mut view = DestinationView {
            path: show(dest),
            copy_root: show(&copy_root),
            blocker: None,
            free_bytes: 0,
            fs_kind: String::new(),
            fs_name: None,
            existing_items: existing_items(&copy_root),
            problems: Vec::new(),
            problem_count: 0,
            identical: 0,
            differs: 0,
            stale_partials: 0,
        };
        match &self.checked {
            Some(Ok(pf)) => {
                let sel = self
                    .selection
                    .as_ref()
                    .expect("checked implies a selection");
                view.free_bytes = pf.fs.available_bytes;
                (view.fs_kind, view.fs_name) = fs_code(&pf.fs.kind);
                view.problems = pf
                    .file_problems
                    .iter()
                    .take(PROBLEMS_SHOWN)
                    .map(|p| FileProblemView {
                        path: show(&sel.files[p.id].rel),
                        reason: say::file_error(&p.kind.to_error()),
                    })
                    .collect();
                view.problem_count = count(pf.file_problems.len());
                view.identical = count(
                    pf.conflicts
                        .iter()
                        .filter(|c| c.kind == ConflictKind::Identical)
                        .count(),
                );
                view.differs = count(pf.conflicts.len()) - view.identical;
                view.stale_partials = count(pf.stale_partials.len());
            }
            Some(Err(blocker)) => view.blocker = Some(say::blocker(blocker)),
            // No source yet: show what the destination is, or why it can't be used.
            None => match fsinfo::fs_info(dest) {
                Ok(info) => {
                    view.free_bytes = info.available_bytes;
                    (view.fs_kind, view.fs_name) = fs_code(&info.kind);
                }
                Err(_) if !dest.is_dir() => {
                    view.blocker = Some(say::blocker(&Blocker::DestMissing))
                }
                Err(e) => view.blocker = Some(say::blocker(&Blocker::DestNotWritable(e.into()))),
            },
        }
        view
    }

    /// Where the files land: `DEST/<folder name>` for "copy the folder itself" (FR-4a),
    /// else `DEST`.
    fn copy_root(&self, dest: &Path) -> PathBuf {
        match self.source.as_ref().and_then(|p| p.scan.root_dir.as_ref()) {
            Some(root) => dest.join(root),
            None => dest.to_path_buf(),
        }
    }
}

fn policy(p: ConflictPolicy) -> DiffersPolicy {
    match p {
        ConflictPolicy::KeepBoth => DiffersPolicy::KeepBoth,
        ConflictPolicy::Overwrite => DiffersPolicy::Overwrite,
        ConflictPolicy::Skip => DiffersPolicy::Skip,
    }
}

fn plan_view(plan: &Plan) -> PlanView {
    PlanView {
        files_to_write: count(plan.files.iter().filter(|f| f.action.writes()).count()),
        bytes_to_write: plan.bytes_to_write(),
        blocker: plan.blockers().first().map(say::blocker),
        purgeable: plan.purgeable_needed().as_ref().map(say::purgeable),
    }
}

/// The source in English, for the report.
fn label(source: &Source) -> String {
    match source {
        Source::Directory { path, .. } => show(path),
        Source::Files(files) if files.len() == 1 => show(&files[0]),
        Source::Files(files) => format!("{} files", files.len()),
    }
}

/// The source as the UI shows it: its path, or "3 files".
pub(crate) fn shown(source: &Source) -> Message {
    match source {
        Source::Directory { path, .. } => Message::raw(show(path)),
        Source::Files(files) if files.len() == 1 => Message::raw(show(&files[0])),
        Source::Files(files) => msg!("copy.picked.files", count = files.len()),
    }
}

/// Why `path` can't be used: its drive isn't connected, or it is gone.
pub(crate) fn gone(path: &Path) -> Message {
    if let Ok(rest) = path.strip_prefix(VOLUMES)
        && let Some(drive) = rest.components().next()
        && !Path::new(VOLUMES).join(drive).exists()
    {
        return msg!(
            "errors.source.notConnected",
            drive = drive.as_os_str().to_string_lossy().into_owned(),
        );
    }
    msg!("errors.source.gone", path = path)
}

/// Whether `a` and `b` are the same directory on disk: on a case-insensitive drive `DCIM`
/// and `dcim` are one (#69). Compared as text when either can't be looked at.
fn same_dir(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a), Ok(b)) => (a.dev(), a.ino()) == (b.dev(), b.ino()),
        _ => a == b,
    }
}

/// Visible items in `dir` (names starting with `.` aren't counted); `None` if it isn't a
/// folder.
fn existing_items(dir: &Path) -> Option<u32> {
    let entries = fs::read_dir(dir).ok()?;
    Some(count(
        entries
            .filter_map(Result::ok)
            .filter(|e| !e.file_name().as_encoded_bytes().starts_with(b"."))
            .count(),
    ))
}

/// The file system as a code the UI names (`copy.fs.*`), and the name of one it has none for.
fn fs_code(kind: &FsKind) -> (String, Option<String>) {
    let code = match kind {
        FsKind::Apfs => "apfs",
        FsKind::HfsPlus => "hfs",
        FsKind::ExFat => "exfat",
        FsKind::Fat => "fat32",
        FsKind::Ntfs => "ntfs",
        FsKind::Smb => "smb",
        FsKind::Nfs => "nfs",
        other => return ("other".into(), Some(format!("{other:?}"))),
    };
    (code.into(), None)
}

/// Scans `source` (the slow part of picking a source); called without the session lock.
pub fn scan_source(source: &Source) -> Result<Scan, Message> {
    scan::scan(source, &ScanOptions::default()).map_err(|e| say::scan_error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::En;

    fn write(root: &Path, files: &[(&str, &[u8])]) {
        for (rel, data) in files {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, data).unwrap();
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        card: PathBuf,
        dest: PathBuf,
    }

    /// CARD/ with two .mov (7 bytes) and one .xml (3 bytes); an empty dest/.
    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        write(
            &card,
            &[
                ("A001.mov", b"movie-a"),
                ("B002.mov", b"movie-b"),
                ("A001.xml", b"xml"),
            ],
        );
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        Fixture {
            _dir: dir,
            card,
            dest,
        }
    }

    use crate::store::CopyPreset;

    /// Applies `change` and runs its scan, like the commands do.
    fn apply(session: &mut Session, change: Change) -> SessionView {
        match session.begin(change) {
            Ok(pending) => {
                let scanned = scan_source(&pending.source);
                session.finish_scan(pending, scanned)
            }
            Err(view) => *view,
        }
    }

    fn pick(session: &mut Session, paths: &[PathBuf], contents_only: bool) -> SessionView {
        apply(session, Change::Pick(paths.to_vec()));
        if contents_only {
            apply(session, Change::IncludeFolder(false))
        } else {
            session.view()
        }
    }

    /// A preset loading `source` (empty: whatever is picked).
    fn preset(source: &Path, extensions: Option<&[&str]>) -> CopyPreset {
        CopyPreset {
            id: "fx3".into(),
            name: "Sony FX3".into(),
            source: show(source),
            include_folder: true,
            extensions: extensions.map(|e| e.iter().map(|x| Some(x.to_string())).collect()),
        }
    }

    /// A card with the fixture's files in PRIVATE/M4ROOT/CLIP; returns the clip folder.
    fn card(f: &Fixture) -> PathBuf {
        let root = f.card.parent().unwrap().join("SONY_CARD");
        write(
            &root.join("PRIVATE/M4ROOT/CLIP"),
            &[
                ("C001.mp4", b"clip-1"),
                ("C002.mp4", b"clip-2"),
                ("C001M01.xml", b"x"),
            ],
        );
        root.join("PRIVATE/M4ROOT/CLIP")
    }

    #[test]
    fn choosing_a_preset_loads_its_source_and_settings() {
        let f = fixture();
        let clip = card(&f);
        let mut s = Session::new();
        let view = apply(&mut s, Change::CopyPreset(Some(preset(&clip, None))));
        let src = view.source.unwrap();
        assert_eq!(src.label, show(&clip));
        assert_eq!(src.root_dir.as_deref(), Some("CLIP"));
        assert_eq!(src.files, 3);
        assert_eq!(view.preset_id.as_deref(), Some("fx3"));
        assert!(!view.preset_changed && view.pick_problem.is_none());
    }

    #[test]
    fn a_preset_whose_source_is_gone_says_so_and_has_no_source() {
        let f = fixture();
        let mut s = Session::new();
        let gone = f.card.parent().unwrap().join("gone");
        let view = apply(&mut s, Change::CopyPreset(Some(preset(&gone, None))));
        assert!(view.source.is_none());
        assert_eq!(
            view.pick_problem.en(),
            Some(format!("{} isn't there any more.", show(&gone)))
        );
        let card = Path::new("/Volumes/SECOPY_NO_SUCH_CARD/DCIM");
        let view = apply(&mut s, Change::CopyPreset(Some(preset(card, None))));
        assert_eq!(
            view.pick_problem.en().as_deref(),
            Some("SECOPY_NO_SUCH_CARD isn't connected.")
        );
        s.set_destination(Some(f.dest.clone()));
        assert!(s.ready().is_none());
    }

    #[test]
    fn the_preset_file_types_become_the_filter() {
        let f = fixture();
        let clip = card(&f);
        let mut s = Session::new();
        let view = apply(
            &mut s,
            Change::CopyPreset(Some(preset(&clip, Some(&["mp4", "wav"])))),
        );
        assert_eq!(view.selected_files, 2, "the two .mp4 files");
        assert_eq!(
            view.source.unwrap().selected_extensions,
            Some(vec![Some("mp4".into())]),
            "only the types on this card are shown as selected"
        );
        assert!(!view.preset_changed);
    }

    #[test]
    fn changed_for_this_run_follows_the_source_include_and_file_types() {
        let f = fixture();
        let clip = card(&f);
        let mut s = Session::new();
        apply(
            &mut s,
            Change::CopyPreset(Some(preset(&clip, Some(&["mp4"])))),
        );
        assert!(
            s.set_filter(Some(vec![Some("mp4".into()), Some("xml".into())]))
                .preset_changed
        );
        assert!(!s.set_filter(Some(vec![Some("mp4".into())])).preset_changed);
        assert!(apply(&mut s, Change::IncludeFolder(false)).preset_changed);
        apply(&mut s, Change::IncludeFolder(true));
        let view = apply(&mut s, Change::Pick(vec![f.card.clone()]));
        assert!(view.preset_changed, "another source");
        assert_eq!(s.updated_preset().unwrap().source, show(&f.card));
    }

    /// #69: the preset's directory picked in other letter case is the same directory on a
    /// case-insensitive drive, not "Changed for this run".
    #[test]
    fn the_same_directory_in_other_letter_case_isnt_a_change() {
        let f = fixture();
        let clip = card(&f);
        let mut s = Session::new();
        apply(&mut s, Change::CopyPreset(Some(preset(&clip, None))));
        let other_case = clip.with_file_name("clip");
        if !other_case.is_dir() {
            return; // a case-sensitive drive: that is another directory
        }
        let view = apply(&mut s, Change::Pick(vec![other_case]));
        assert!(view.source.is_some());
        assert!(!view.preset_changed);
    }

    #[test]
    fn a_preset_without_a_source_gets_one_with_update() {
        let f = fixture();
        let mut s = Session::new();
        let view = apply(
            &mut s,
            Change::CopyPreset(Some(preset(Path::new(""), None))),
        );
        assert!(view.source.is_none() && view.pick_problem.is_none());
        let view = apply(&mut s, Change::Pick(vec![f.card.clone()]));
        assert!(view.preset_changed);
        let updated = s.updated_preset().unwrap();
        assert_eq!(updated.source, show(&f.card));
        assert!(!s.preset_saved(updated).preset_changed);
    }

    #[test]
    fn the_include_checkbox_keeps_this_runs_file_types() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        s.set_filter(Some(vec![Some("xml".into())]));
        let view = apply(&mut s, Change::IncludeFolder(false));
        assert!(view.source.unwrap().contents_only);
        assert_eq!(view.selected_files, 1, "still only the .xml");
    }

    #[test]
    fn update_keeps_file_types_not_on_this_card() {
        let f = fixture();
        let clip = card(&f);
        let mut s = Session::new();
        apply(
            &mut s,
            Change::CopyPreset(Some(preset(&clip, Some(&["mp4", "wav"])))),
        );
        s.set_filter(Some(vec![Some("xml".into())]));
        let updated = s.updated_preset().unwrap();
        assert_eq!(
            updated.extensions,
            Some(vec![Some("wav".into()), Some("xml".into())]),
            ".wav isn't on this card, so it stays; .mp4 was turned off, .xml on"
        );
        let view = s.preset_saved(updated);
        assert!(!view.preset_changed);
    }

    #[test]
    fn an_all_types_preset_stays_all_when_nothing_is_turned_off() {
        let f = fixture();
        let clip = card(&f);
        let mut s = Session::new();
        apply(&mut s, Change::CopyPreset(Some(preset(&clip, None))));
        apply(&mut s, Change::IncludeFolder(false));
        let updated = s.updated_preset().unwrap();
        assert_eq!((updated.include_folder, updated.extensions), (false, None));
        s.set_filter(Some(vec![Some("mp4".into())]));
        assert_eq!(
            s.updated_preset().unwrap().extensions,
            Some(vec![Some("mp4".into())])
        );
    }

    #[test]
    fn a_files_pick_ignores_the_preset() {
        let f = fixture();
        let mut s = Session::new();
        apply(
            &mut s,
            Change::CopyPreset(Some(preset(Path::new(""), Some(&["mp4"])))),
        );
        let view = apply(
            &mut s,
            Change::Pick(vec![f.card.join("A001.mov"), f.card.join("A001.xml")]),
        );
        let src = view.source.unwrap();
        assert!(!src.is_folder);
        assert_eq!(view.selected_files, 2);
        assert!(!view.preset_changed);
    }

    #[test]
    fn update_and_save_as_wait_for_a_pending_scan() {
        let f = fixture();
        let clip = card(&f);
        let mut s = Session::new();
        apply(
            &mut s,
            Change::CopyPreset(Some(preset(&clip, Some(&["mp4"])))),
        );
        s.set_filter(Some(vec![Some("xml".into())]));
        assert!(s.updated_preset().is_some() && s.choices().is_some());
        let mut photos = preset(&f.card, Some(&["jpg"]));
        photos.id = "photos".into();
        let pending = s.begin(Change::CopyPreset(Some(photos))).ok();
        assert!(s.scan_pending());
        assert_eq!(
            s.updated_preset(),
            None,
            "the choices belong to the old preset's scan"
        );
        assert_eq!(s.choices(), None);
        drop(pending);
    }

    /// #69 review: a scan started before the session was started afresh stays stale, even
    /// though a newer one was begun since.
    #[test]
    fn a_scan_from_before_a_restart_is_stale() {
        let f = fixture();
        let mut s = Session::new();
        let old = s.begin(Change::Pick(vec![f.card.clone()])).ok().unwrap();
        s.restart();
        let new = s.begin(Change::Pick(vec![card(&f)])).ok().unwrap();
        let old_scan = scan_source(&old.source);
        assert!(s.finish_scan(old, old_scan).stale);
        let new_scan = scan_source(&new.source);
        assert!(!s.finish_scan(new, new_scan).stale);
    }

    #[test]
    fn a_new_picks_file_types_survive_an_include_toggle_during_its_scan() {
        let f = fixture();
        let clip = card(&f);
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        s.set_filter(Some(vec![Some("xml".into())]));
        let first = s.begin(Change::Pick(vec![clip])).ok().unwrap();
        let second = s.begin(Change::IncludeFolder(false)).ok().unwrap();
        let first_scan = scan_source(&first.source);
        assert!(s.finish_scan(first, first_scan).stale);
        let second_scan = scan_source(&second.source);
        let view = s.finish_scan(second, second_scan);
        assert_eq!(
            view.selected_files, 3,
            "a new pick starts with every file type, not the old card's"
        );
        assert!(!s.scan_pending());
    }

    #[test]
    fn save_as_new_saves_the_picked_directory() {
        let f = fixture();
        let mut s = Session::new();
        assert_eq!(s.picked_source(), None);
        pick(&mut s, std::slice::from_ref(&f.card), false);
        assert_eq!(s.picked_source(), Some(show(&f.card)));
        apply(&mut s, Change::Pick(vec![f.card.join("A001.mov")]));
        assert_eq!(s.picked_source(), None, "files have no source to save");
    }

    #[test]
    fn a_folder_shows_counts_and_extensions_largest_first() {
        let f = fixture();
        let mut s = Session::new();
        let view = pick(&mut s, std::slice::from_ref(&f.card), false);
        let src = view.source.unwrap();
        assert!(src.is_folder && !src.contents_only && !src.is_retry);
        assert_eq!(src.root_dir.as_deref(), Some("CARD"));
        assert_eq!(src.folder, Some(show(&f.card)));
        assert_eq!((src.files, src.bytes), (3, 17));
        let keys: Vec<_> = src.extensions.iter().map(|e| e.key.as_deref()).collect();
        assert_eq!(keys, [Some("mov"), Some("xml")]);
        assert_eq!((view.selected_files, view.selected_bytes), (3, 17));
        assert_eq!(src.selected_extensions, None);
    }

    #[test]
    fn the_filter_narrows_the_selection() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let view = s.set_filter(Some(vec![Some("mov".into())]));
        assert_eq!((view.selected_files, view.selected_bytes), (2, 14));
        assert_eq!(
            view.source.unwrap().selected_extensions,
            Some(vec![Some("mov".into())])
        );
    }

    #[test]
    fn a_folder_and_files_together_are_refused() {
        let f = fixture();
        let mut s = Session::new();
        let view = apply(
            &mut s,
            Change::Pick(vec![f.card.clone(), f.card.join("A001.mov")]),
        );
        assert!(view.source.is_none());
        assert_eq!(
            view.pick_problem.en().as_deref(),
            Some("Pick one directory, or only files — not both.")
        );
    }

    #[test]
    fn copy_root_follows_the_folder_or_contents_choice() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let view = s.set_destination(Some(f.dest.clone()));
        assert_eq!(
            view.destination.unwrap().copy_root,
            show(&f.dest.join("CARD"))
        );
        let view = pick(&mut s, std::slice::from_ref(&f.card), true);
        assert_eq!(view.destination.unwrap().copy_root, show(&f.dest));
    }

    #[test]
    fn a_non_empty_copy_root_is_counted_without_hidden_files() {
        let f = fixture();
        write(
            &f.dest,
            &[
                ("CARD/old.mov", b"x"),
                ("CARD/.DS_Store", b"x"),
                ("CARD/sub/y.mov", b"y"),
            ],
        );
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let dest = s.set_destination(Some(f.dest.clone())).destination.unwrap();
        assert_eq!(dest.existing_items, Some(2), "old.mov and sub/");
        assert!(dest.blocker.is_none());
    }

    #[test]
    fn a_missing_copy_root_has_no_existing_items() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let dest = s.set_destination(Some(f.dest.clone())).destination.unwrap();
        assert_eq!(dest.existing_items, None);
    }

    #[test]
    fn conflicts_are_counted_and_the_policy_changes_the_plan() {
        let f = fixture();
        write(&f.dest, &[("CARD/A001.mov", b"other content")]);
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let view = s.set_destination(Some(f.dest.clone()));
        let dest = view.destination.unwrap();
        assert_eq!((dest.identical, dest.differs), (0, 1));
        assert_eq!(view.conflicts, ConflictPolicy::KeepBoth);
        assert_eq!(view.plan.unwrap().files_to_write, 3);
        let view = s.set_policy(ConflictPolicy::Skip);
        assert_eq!(view.plan.unwrap().files_to_write, 2);
        assert!(s.ready().is_some());
    }

    #[test]
    fn a_destination_inside_the_source_blocks_start() {
        let f = fixture();
        let inside = f.card.join("backup");
        fs::create_dir_all(&inside).unwrap();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        let view = s.set_destination(Some(inside));
        let blocker = view.destination.unwrap().blocker.unwrap();
        assert!(
            blocker == "The destination is the source directory or inside it",
            "{blocker}"
        );
        assert!(view.plan.is_none());
        assert!(s.ready().is_none());
    }

    #[test]
    fn a_copy_that_needs_purgeable_space_says_so() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        s.set_destination(Some(f.dest.clone()));
        let mut plan = s.plan.clone().expect("a plan");
        let needed = plan.bytes_to_write() + secopy_core::plan::space_margin(plan.bytes_to_write());
        plan.fs.free_bytes = needed - 1;
        plan.fs.available_bytes = needed;
        let view = plan_view(&plan);
        assert!(view.blocker.is_none());
        assert_eq!(
            view.purgeable.map(|m| m.key),
            Some("copy.preflight.purgeable".to_string())
        );
        plan.fs.free_bytes = needed;
        assert!(plan_view(&plan).purgeable.is_none());
    }

    #[test]
    fn a_destination_alone_shows_its_free_space_or_why_it_cant_be_used() {
        let f = fixture();
        let mut s = Session::new();
        let dest = s.set_destination(Some(f.dest.clone())).destination.unwrap();
        assert!(dest.free_bytes > 0 && dest.blocker.is_none());
        let missing = s
            .set_destination(Some(f.dest.join("nope")))
            .destination
            .unwrap();
        assert_eq!(
            missing.blocker.en().as_deref(),
            Some("The destination is not an existing directory")
        );
    }

    #[test]
    fn a_scan_replaced_by_a_newer_one_is_dropped() {
        let f = fixture();
        let mut s = Session::new();
        let old = s.begin(Change::Pick(vec![f.card.clone()])).ok().unwrap();
        let newer = s.begin(Change::Pick(vec![f.card.clone()])).ok().unwrap();
        let old_scan = scan_source(&old.source);
        let view = s.finish_scan(old, old_scan);
        assert!(view.stale && view.source.is_none());
        let newer_scan = scan_source(&newer.source);
        let view = s.finish_scan(newer, newer_scan);
        assert!(!view.stale && view.source.is_some());
    }

    #[test]
    fn a_retry_keeps_the_copy_root_and_says_it_is_a_retry() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        s.set_destination(Some(f.dest.clone()));
        let source = Session::source_for(std::slice::from_ref(&f.card), false).unwrap();
        let failed = scan_source(&source)
            .unwrap()
            .select(&ExtensionFilter::All)
            .subset(&[0]);
        let view = s.install_retry(source, failed);
        let src = view.source.unwrap();
        assert!(src.is_retry);
        assert_eq!(src.files, 1);
        assert_eq!(
            view.destination.unwrap().copy_root,
            show(&f.dest.join("CARD"))
        );
        assert_eq!(s.ready().unwrap().copy_root, f.dest.join("CARD"));
    }

    #[test]
    fn nothing_to_write_is_not_ready() {
        let f = fixture();
        let mut s = Session::new();
        pick(&mut s, std::slice::from_ref(&f.card), false);
        s.set_destination(Some(f.dest.clone()));
        s.set_filter(Some(vec![Some("wav".into())]));
        assert!(s.ready().is_none());
    }
}
