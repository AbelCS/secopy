mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::write_files;
use secopy_core::filter::ExtensionFilter;
use secopy_core::preflight::{Blocker, ConflictKind, ProblemKind, preflight};
use secopy_core::scan::{ScanOptions, Selection, scan};
use secopy_core::source::{DirMode, Source};

fn card(root: &Path) -> (Source, Selection) {
    let src = root.join("CARD");
    write_files(&src, &[("A001.mov", b"clip-a"), ("B002.mov", b"clip-b")]);
    let source = Source::Directory {
        path: src,
        mode: DirMode::FolderItself,
    };
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    (source, sel)
}

fn set_mtime(path: &Path, t: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn id_of(sel: &Selection, name: &str) -> usize {
    sel.files
        .iter()
        .position(|f| f.rel.ends_with(name))
        .unwrap()
}

#[test]
fn a_missing_destination_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let err = preflight(&source, &sel, &dir.path().join("nope")).unwrap_err();
    assert_eq!(err, Blocker::DestMissing);
}

#[test]
fn a_destination_inside_the_source_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let inside = dir.path().join("CARD/backup");
    fs::create_dir_all(&inside).unwrap();
    assert_eq!(
        preflight(&source, &sel, &inside).unwrap_err(),
        Blocker::DestInsideSource
    );
    assert_eq!(
        preflight(&source, &sel, &dir.path().join("CARD")).unwrap_err(),
        Blocker::DestInsideSource
    );
}

#[test]
fn a_clean_destination_has_nothing_to_report() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let pf = preflight(&source, &sel, &dest).unwrap();
    assert!(pf.file_problems.is_empty());
    assert!(pf.conflicts.is_empty());
    assert!(pf.stale_partials.is_empty());
    assert_eq!(pf.source_roots.len(), 1);
    assert!(pf.fs.free_bytes > 0);
}

#[test]
fn existing_files_are_identical_or_different() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    write_files(
        &dest,
        &[
            ("CARD/A001.mov", b"clip-a"),
            ("CARD/B002.mov", b"another clip"),
        ],
    );
    let (source, _) = card(dir.path());
    let t = SystemTime::now() - Duration::from_secs(3600);
    for name in ["A001.mov", "B002.mov"] {
        set_mtime(&dir.path().join("CARD").join(name), t);
        set_mtime(&dest.join("CARD").join(name), t);
    }
    // Scan after setting the source mtimes.
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &dest).unwrap();
    let kind = |name| {
        pf.conflicts
            .iter()
            .find(|c| c.id == id_of(&sel, name))
            .map(|c| c.kind.clone())
    };
    assert_eq!(kind("A001.mov"), Some(ConflictKind::Identical));
    assert!(matches!(
        kind("B002.mov"),
        Some(ConflictKind::Differs { size: 12, .. })
    ));
}

#[test]
fn a_folder_where_a_file_goes_is_in_the_way() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let dest = dir.path().join("dest");
    fs::create_dir_all(dest.join("CARD/A001.mov")).unwrap();
    let pf = preflight(&source, &sel, &dest).unwrap();
    assert_eq!(pf.file_problems.len(), 1);
    assert_eq!(pf.file_problems[0].id, id_of(&sel, "A001.mov"));
    assert!(matches!(
        pf.file_problems[0].kind,
        ProblemKind::InTheWay { .. }
    ));
}

#[test]
fn leftover_partial_files_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    let (source, sel) = card(dir.path());
    let dest = dir.path().join("dest");
    write_files(&dest, &[("CARD/.A001.mov.secopy-partial", b"half")]);
    let pf = preflight(&source, &sel, &dest).unwrap();
    assert_eq!(
        pf.stale_partials,
        vec![dest.join("CARD").join(".A001.mov.secopy-partial")]
    );
}

#[test]
fn loose_files_report_each_parent_folder_once() {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[("x/a.wav", b"a"), ("x/b.wav", b"b"), ("y/c.wav", b"c")],
    );
    let source = Source::Files(
        ["x/a.wav", "x/b.wav", "y/c.wav"]
            .map(|p| dir.path().join(p))
            .to_vec(),
    );
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let pf = preflight(&source, &sel, &dest).unwrap();
    let roots: Vec<PathBuf> = pf.source_roots.into_iter().map(|r| r.path).collect();
    assert_eq!(roots, vec![dir.path().join("x"), dir.path().join("y")]);
}

#[test]
fn a_source_file_is_never_a_target() {
    // Contents of SSD/X into SSD: X/a.mov would land on the source's own a.mov (#112).
    let dir = tempfile::tempdir().unwrap();
    let ssd = dir.path().join("SSD");
    let x = ssd.join("X");
    write_files(&x, &[("a.mov", b"only copy"), ("X/a.mov", b"another")]);
    let source = Source::Directory {
        path: x.clone(),
        mode: DirMode::ContentsOnly,
    };
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &ssd).unwrap();
    let id = sel
        .files
        .iter()
        .position(|f| f.rel == Path::new("X/a.mov"))
        .unwrap();
    assert!(
        pf.file_problems
            .iter()
            .any(|p| p.id == id && p.kind == ProblemKind::InSource),
        "{:?}",
        pf.file_problems
    );
    assert!(pf.conflicts.iter().all(|c| c.id != id));

    // A file picked from the destination itself.
    let picked = ssd.join("b.mov");
    fs::write(&picked, b"b").unwrap();
    let source = Source::Files(vec![picked]);
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &ssd).unwrap();
    assert_eq!(pf.file_problems.len(), 1);
    assert_eq!(pf.file_problems[0].kind, ProblemKind::InSource);
    assert!(pf.conflicts.is_empty());
}
