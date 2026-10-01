//! ASC MHL (#154): preparing a copy's history and recording it.
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use chrono::TimeZone;
use common::write_files;
use secopy_core::filter::ExtensionFilter;
use secopy_core::hash::hash_bytes;
use secopy_core::mhl::prepare::{MhlBlocker, MhlInputs, MhlPlan, prepare};
use secopy_core::mhl::read::{Damage, read};
use secopy_core::mhl::write::append;
use secopy_core::mhl::{Action, Generation, Record};
use secopy_core::plan::{Action as PlanAction, DiffersPolicy, Plan};
use secopy_core::preflight::preflight;
use secopy_core::scan::{ScanOptions, scan};
use secopy_core::source::{DirMode, Source};

struct Fixture {
    _dir: tempfile::TempDir,
    src: PathBuf,
    dest: PathBuf,
}

/// `src/A` with `files`, and an empty `dest`.
fn fixture(files: &[(&str, &[u8])]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src/A");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    write_files(&src, files);
    Fixture {
        _dir: dir,
        src,
        dest,
    }
}

/// The plan for copying `src` (the folder itself) into `dest`, and its MHL inputs.
fn planned(f: &Fixture, filter: &ExtensionFilter, policy: DiffersPolicy) -> (Plan, MhlInputs) {
    let source = Source::Directory {
        path: f.src.clone(),
        mode: DirMode::FolderItself,
    };
    let scanned = scan(&source, &ScanOptions::default()).unwrap();
    let sel = scanned.select(filter);
    let pf = preflight(&source, &sel, &f.dest).unwrap();
    let plan = Plan::resolve(&sel, &pf, policy);
    let copy_root = match &scanned.root_dir {
        Some(root) => f.dest.join(root),
        None => f.dest.clone(),
    };
    let inputs = MhlInputs {
        copy_root,
        source_dir: Some(f.src.clone()),
    };
    (plan, inputs)
}

fn prepared(f: &Fixture) -> (Plan, Result<MhlPlan, Vec<MhlBlocker>>) {
    let (mut plan, inputs) = planned(f, &ExtensionFilter::All, DiffersPolicy::KeepBoth);
    let result = prepare(&mut plan, &inputs);
    (plan, result)
}

/// Appends a generation listing `files` (with the xxh64 of the given bytes) to the history at
/// `scope`, starting one if there's none.
fn history(scope: &Path, files: &[(&str, &[u8])]) {
    fs::create_dir_all(scope).unwrap();
    let before = read(scope).unwrap();
    let g = Generation {
        created: chrono::Local
            .with_ymd_and_hms(2026, 10, 1, 10, 0, 0)
            .unwrap(),
        hostname: "h".into(),
        tool_version: "other".into(),
        ignore: secopy_core::mhl::ignore::standard_defaults(),
        records: files
            .iter()
            .map(|(rel, bytes)| Record {
                rel: rel.to_string(),
                size: bytes.len() as u64,
                modified: None,
                xxh64: hash_bytes(bytes),
                action: Action::Original,
            })
            .collect(),
        references: Vec::new(),
    };
    let n = before.as_ref().map_or(0, |h| h.entries.len()) as u32;
    let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 0, n).unwrap();
    append(
        scope,
        before.as_ref().map(|h| h.chain_bytes.as_slice()),
        before.as_ref().map_or(&[][..], |h| &h.entries),
        &g,
        at,
    )
    .unwrap();
}

fn blockers(r: Result<MhlPlan, Vec<MhlBlocker>>) -> Vec<MhlBlocker> {
    r.expect_err("blocked")
}

#[test]
fn a_plain_copy_is_a_new_history_with_nothing_to_read() {
    let f = fixture(&[("a.mov", b"aaa"), ("sub/b.mov", b"bb")]);
    let p = prepared(&f).1.unwrap();
    assert_eq!(p.scopes.len(), 1);
    assert_eq!(p.scopes[0].scope, f.dest.join("A"));
    assert_eq!(p.generation(), 1);
    assert!(p.to_read.is_empty());
}

#[test]
fn files_already_there_are_read() {
    let f = fixture(&[("a.mov", b"aaa")]);
    write_files(&f.dest, &[("A/old.mov", b"old!")]);
    let p = prepared(&f).1.unwrap();
    assert_eq!(p.to_read, [f.dest.join("A/old.mov")]);
    assert_eq!(p.to_read_bytes, 4);
}

#[test]
fn ignored_files_already_there_arent_read() {
    let f = fixture(&[("a.mov", b"aaa")]);
    write_files(
        &f.dest,
        &[
            ("A/.DS_Store", b"x"),
            ("A/secopy_2026-10-01_101500.xxh64", b"x"),
        ],
    );
    assert!(prepared(&f).1.unwrap().to_read.is_empty());
}

#[test]
fn a_destination_history_continues_and_its_files_arent_read() {
    let f = fixture(&[("a.mov", b"aaa")]);
    write_files(&f.dest, &[("A/old.mov", b"old!")]);
    history(&f.dest.join("A"), &[("old.mov", b"old!")]);
    let p = prepared(&f).1.unwrap();
    assert_eq!(p.generation(), 2);
    assert!(p.to_read.is_empty());
    assert!(p.scopes[0].dest.is_some());
}

#[test]
fn a_source_history_comes_along() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"aaa")]);
    let p = prepared(&f).1.unwrap();
    assert!(p.scopes[0].source.is_some());
    assert_eq!(p.generation(), 2);
}

#[test]
fn the_same_history_twice_appends() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"aaa")]);
    // A first copy brought it (same chain and manifest), then added a generation.
    let dest = f.dest.join("A");
    fs::create_dir_all(dest.join("ascmhl")).unwrap();
    for e in fs::read_dir(f.src.join("ascmhl")).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), dest.join("ascmhl").join(e.file_name())).unwrap();
    }
    write_files(&dest, &[("a.mov", b"aaa")]);
    history(&dest, &[("a.mov", b"aaa")]);
    let (plan, result) = prepared(&f);
    let p = result.unwrap();
    assert_eq!(p.generation(), 3);
    for file in plan
        .files
        .iter()
        .filter(|p| p.entry.rel.components().any(|c| c.as_os_str() == "ascmhl"))
    {
        assert_eq!(
            file.action,
            PlanAction::SkipIdentical,
            "{:?}",
            file.entry.rel
        );
    }
}

#[test]
fn two_different_histories_block() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"aaa")]);
    history(&f.dest.join("A"), &[("other.mov", b"o")]);
    write_files(&f.dest, &[("A/other.mov", b"o")]);
    assert!(
        blockers(prepared(&f).1).contains(&MhlBlocker::TwoHistories {
            scope: f.dest.join("A")
        })
    );
}

#[test]
fn overwriting_a_recorded_file_blocks() {
    let f = fixture(&[("a.mov", b"new contents")]);
    write_files(&f.dest, &[("A/a.mov", b"old")]);
    history(&f.dest.join("A"), &[("a.mov", b"old")]);
    let (mut plan, inputs) = planned(&f, &ExtensionFilter::All, DiffersPolicy::Overwrite);
    assert!(
        blockers(prepare(&mut plan, &inputs)).contains(&MhlBlocker::OverwritesRecorded {
            path: f.dest.join("A/a.mov")
        })
    );
}

#[test]
fn a_damaged_history_blocks() {
    let f = fixture(&[("a.mov", b"aaa")]);
    fs::create_dir_all(f.dest.join("A/ascmhl")).unwrap();
    assert!(blockers(prepared(&f).1).contains(&MhlBlocker::Damaged {
        scope: f.dest.join("A"),
        damage: Damage::NoChain
    }));
}

#[test]
fn leaving_out_recorded_files_blocks() {
    let f = fixture(&[("a.mov", b"aaa"), ("b.wav", b"bbb")]);
    history(&f.src, &[("a.mov", b"aaa"), ("b.wav", b"bbb")]);
    let movs = ExtensionFilter::Only(
        ["mov", "mhl", "xml"]
            .into_iter()
            .map(|e| Some(e.to_string()))
            .collect(),
    );
    let (mut plan, inputs) = planned(&f, &movs, DiffersPolicy::KeepBoth);
    assert!(
        blockers(prepare(&mut plan, &inputs)).contains(&MhlBlocker::LeavesOut {
            scope: f.dest.join("A")
        })
    );
}

#[test]
fn leaving_out_the_history_itself_blocks() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"aaa")]);
    let movs = ExtensionFilter::Only([Some("mov".to_string())].into_iter().collect());
    let (mut plan, inputs) = planned(&f, &movs, DiffersPolicy::KeepBoth);
    assert!(
        blockers(prepare(&mut plan, &inputs)).contains(&MhlBlocker::LeavesOut {
            scope: f.dest.join("A")
        })
    );
}

#[test]
fn a_name_xml_cant_hold_blocks() {
    let f = fixture(&[("bad\u{1}.mov", b"x")]);
    assert!(blockers(prepared(&f).1).contains(&MhlBlocker::Unlistable {
        path: f.dest.join("A/bad\u{1}.mov")
    }));
}

#[test]
fn nested_histories_come_deepest_first() {
    let f = fixture(&[
        ("A001/a.mov", b"a"),
        ("A002/b.mov", b"b"),
        ("notes.txt", b"n"),
    ]);
    history(&f.src.join("A001"), &[("a.mov", b"a")]);
    history(&f.src.join("A002"), &[("b.mov", b"b")]);
    let p = prepared(&f).1.unwrap();
    let scopes: Vec<_> = p.scopes.iter().map(|s| s.scope.clone()).collect();
    assert_eq!(scopes.len(), 3);
    assert_eq!(scopes[2], f.dest.join("A"));
    assert!(scopes[..2].contains(&f.dest.join("A/A001")));
    assert!(scopes[..2].contains(&f.dest.join("A/A002")));
}
