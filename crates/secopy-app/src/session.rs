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

/// Per-file problems sent to the UI; the rest are only counted.
const PROBLEMS_SHOWN: usize = 100;
/// Scan problems sent to the UI.
const SCAN_PROBLEMS_SHOWN: usize = 20;

#[derive(Default)]
pub struct Session {
    /// Increases with every scan; only the newest scan's result is kept (FR-3).
    generation: u64,
    source: Option<Picked>,
    filter: ExtensionFilter,
    selection: Option<Selection>,
    dest: Option<PathBuf>,
    policy: ConflictPolicy,
    checked: Option<Result<Preflight, Blocker>>,
    plan: Option<Plan>,
}

/// Everything a job needs from the main window.
pub struct Ready {
    pub source: Source,
    pub plan: Plan,
    /// The source as shown, for the report.
    pub label: String,
    /// Where the files land, for Reveal in Finder.
    pub copy_root: PathBuf,
}

struct Picked {
    label: String,
    is_retry: bool,
    source: Source,
    scan: Scan,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// One folder → a folder source; otherwise only files (RFD Q6).
    pub fn source_for(paths: &[PathBuf], contents_only: bool) -> Result<Source, String> {
        match paths {
            [] => Err("nothing was picked".into()),
            [only] if only.is_dir() => Ok(Source::Directory {
                path: only.clone(),
                mode: if contents_only {
                    DirMode::ContentsOnly
                } else {
                    DirMode::FolderItself
                },
            }),
            _ if paths.iter().any(|p| p.is_dir()) => {
                Err("Pick one folder, or only files — not both.".into())
            }
            _ => Ok(Source::Files(paths.to_vec())),
        }
    }

    /// Starts a scan and returns its ticket. Scanning runs without the session locked;
    /// [`finish_scan`](Self::finish_scan) then keeps the result only if no newer scan began.
    pub fn begin_scan(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    pub fn finish_scan(&mut self, ticket: u64, source: Source, scan: Scan) -> SessionView {
        if ticket != self.generation {
            return SessionView {
                stale: true,
                ..self.view()
            };
        }
        self.source = Some(Picked {
            label: label(&source),
            is_retry: false,
            source,
            scan,
        });
        self.filter = ExtensionFilter::All;
        self.recompute();
        self.view()
    }

    /// For "Retry failed": the failed files of the last job, checked again (RFD §5.4).
    pub fn install_retry(&mut self, source: Source, selection: Selection) -> SessionView {
        self.generation += 1;
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
            is_retry: true,
            source,
            scan,
        });
        self.filter = ExtensionFilter::All;
        self.selection = Some(selection);
        self.recheck();
        self.view()
    }

    pub fn clear_source(&mut self) -> SessionView {
        self.generation += 1;
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
        }
    }

    fn source_view(&self, picked: &Picked) -> SourceView {
        let scan = &picked.scan;
        let mut extensions: Vec<ExtensionView> = scan
            .ext_stats
            .iter()
            .map(|(key, stat)| ExtensionView {
                key: key.clone(),
                label: key
                    .as_ref()
                    .map_or("(no extension)".to_string(), |k| format!(".{k}")),
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
            label: picked.label.clone(),
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
                ExtensionFilter::Only(keys) => Some(keys.iter().cloned().collect()),
            },
            skipped_hidden: count(scan.skipped_hidden),
            skipped_symlinks: count(scan.skipped_symlinks.len()),
            problems: scan
                .problems
                .iter()
                .take(SCAN_PROBLEMS_SHOWN)
                .map(|p| format!("{}: {}", show(&p.path), p.message))
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
                view.free_bytes = pf.fs.free_bytes;
                view.fs_kind = fs_label(&pf.fs.kind);
                view.problems = pf
                    .file_problems
                    .iter()
                    .take(PROBLEMS_SHOWN)
                    .map(|p| FileProblemView {
                        path: show(&sel.files[p.id].rel),
                        reason: p.kind.to_error().to_string(),
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
            Some(Err(blocker)) => view.blocker = Some(sentence(blocker.to_string())),
            // No source yet: show what the destination is, or why it can't be used.
            None => match fsinfo::fs_info(dest) {
                Ok(info) => {
                    view.free_bytes = info.free_bytes;
                    view.fs_kind = fs_label(&info.kind);
                }
                Err(_) if !dest.is_dir() => {
                    view.blocker = Some(sentence(Blocker::DestMissing.to_string()))
                }
                Err(e) => {
                    view.blocker = Some(sentence(Blocker::DestNotWritable(e.into()).to_string()))
                }
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
        blocker: plan.blockers().first().map(|b| sentence(b.to_string())),
    }
}

fn label(source: &Source) -> String {
    match source {
        Source::Directory { path, .. } => show(path),
        Source::Files(files) if files.len() == 1 => show(&files[0]),
        Source::Files(files) => format!("{} files", files.len()),
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

fn fs_label(kind: &FsKind) -> String {
    match kind {
        FsKind::Apfs => "APFS".into(),
        FsKind::HfsPlus => "Mac OS Extended".into(),
        FsKind::ExFat => "exFAT".into(),
        FsKind::Fat => "FAT32".into(),
        FsKind::Ntfs => "NTFS".into(),
        FsKind::Smb => "network (SMB)".into(),
        FsKind::Nfs => "network (NFS)".into(),
        other => format!("{other:?}"),
    }
}

/// Engine messages start in lower case; the UI shows them as sentences.
fn sentence(text: String) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

/// Scans `source` (the slow part of picking a source); called without the session lock.
pub fn scan_source(source: &Source) -> Result<Scan, String> {
    scan::scan(source, &ScanOptions::default()).map_err(|e| sentence(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn pick(session: &mut Session, paths: &[PathBuf], contents_only: bool) -> SessionView {
        let ticket = session.begin_scan();
        let source = Session::source_for(paths, contents_only).unwrap();
        let scan = scan_source(&source).unwrap();
        session.finish_scan(ticket, source, scan)
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
        let labels: Vec<_> = src.extensions.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, [".mov", ".xml"]);
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
        let err =
            Session::source_for(&[f.card.clone(), f.card.join("A001.mov")], false).unwrap_err();
        assert!(err.contains("not both"), "{err}");
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
            blocker.starts_with("The destination is the source"),
            "{blocker}"
        );
        assert!(view.plan.is_none());
        assert!(s.ready().is_none());
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
            missing.blocker.as_deref(),
            Some("The destination is not an existing folder")
        );
    }

    #[test]
    fn a_scan_replaced_by_a_newer_one_is_dropped() {
        let f = fixture();
        let mut s = Session::new();
        let old = s.begin_scan();
        let newer = s.begin_scan();
        let source = Session::source_for(std::slice::from_ref(&f.card), false).unwrap();
        let view = s.finish_scan(old, source.clone(), scan_source(&source).unwrap());
        assert!(view.stale && view.source.is_none());
        let view = s.finish_scan(newer, source.clone(), scan_source(&source).unwrap());
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
