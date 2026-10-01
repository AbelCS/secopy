//! ASC MHL (#154): preparing a copy's history and recording it.
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use chrono::TimeZone;
use common::write_files;
use secopy_core::filter::ExtensionFilter;
use secopy_core::hash::{hash_bytes, to_hex};
use secopy_core::ignore::Patterns;
use secopy_core::job::{Event, JobControl, JobOptions, JobReport, Progress, run_job, undo};
use secopy_core::mhl::MhlJob;
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
    planned_with(f, filter, policy, Patterns::defaults())
}

/// `planned` with an ignore list for the scan and the history.
fn planned_with(
    f: &Fixture,
    filter: &ExtensionFilter,
    policy: DiffersPolicy,
    ignore: Patterns,
) -> (Plan, MhlInputs) {
    let source = Source::Directory {
        path: f.src.clone(),
        mode: DirMode::FolderItself,
    };
    let scanned = scan(
        &source,
        &ScanOptions {
            ignore: ignore.clone(),
        },
    )
    .unwrap();
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
        ignore,
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

// Recording at the end of the job (Task 6).

fn run(plan: &Plan, mhl: Option<MhlPlan>) -> (JobReport, Vec<Progress>) {
    let opts = JobOptions {
        mhl: mhl.map(|plan| MhlJob {
            plan,
            tool_version: "9.9.9".into(),
        }),
        progress_interval: std::time::Duration::from_millis(1),
        ..JobOptions::default()
    };
    let events = std::sync::Mutex::new(Vec::new());
    let report = run_job(plan, &opts, &JobControl::new(), &|e| {
        if let Event::Progress(p) = e {
            events.lock().unwrap().push(p)
        }
    });
    (report, events.into_inner().unwrap())
}

/// Copies `f` with ASC MHL on.
fn copy(f: &Fixture) -> JobReport {
    let (plan, result) = prepared(f);
    run(&plan, Some(result.unwrap())).0
}

fn manifests(scope: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(scope.join("ascmhl"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".mhl"))
        .collect();
    names.sort();
    names
}

fn latest_manifest(scope: &Path) -> String {
    let name = manifests(scope).pop().unwrap();
    fs::read_to_string(scope.join("ascmhl").join(name)).unwrap()
}

#[test]
fn a_copy_writes_a_new_history_that_lists_every_file() {
    let f = fixture(&[("a.mov", b"aaa"), ("sub/b.mov", b"bb")]);
    let report = copy(&f);
    assert!(report.is_success(), "{report:?}");
    assert_eq!(report.mhl_written.len(), 1);
    let h = read(&f.dest.join("A")).unwrap().unwrap();
    assert_eq!(h.first_xxh64["a.mov"], hash_bytes(b"aaa"));
    assert_eq!(h.first_xxh64["sub/b.mov"], hash_bytes(b"bb"));
    let text = latest_manifest(&f.dest.join("A"));
    assert!(text.contains(r#"<tool version="9.9.9">Secopy</tool>"#));
    assert!(text.contains("<process>transfer</process>"));
}

#[test]
fn files_already_there_are_read_and_recorded() {
    let f = fixture(&[("a.mov", b"aaa")]);
    write_files(&f.dest, &[("A/old.mov", b"old!")]);
    let (plan, result) = prepared(&f);
    let (report, progress) = run(&plan, Some(result.unwrap()));
    assert!(report.is_success(), "{report:?}");
    let h = read(&f.dest.join("A")).unwrap().unwrap();
    assert_eq!(h.first_xxh64["old.mov"], hash_bytes(b"old!"));
    assert!(
        progress
            .iter()
            .any(|p| p.recording.as_ref().is_some_and(|r| r.total == 4))
    );
}

#[test]
fn a_source_history_is_continued_with_verified_files() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"aaa")]);
    let report = copy(&f);
    assert!(report.is_success(), "{report:?}");
    assert_eq!(manifests(&f.dest.join("A")).len(), 2);
    let text = latest_manifest(&f.dest.join("A"));
    assert!(text.contains(&format!(
        r#"<xxh64 action="verified">{}</xxh64>"#,
        to_hex(hash_bytes(b"aaa"))
    )));
    // The source's history is untouched.
    assert_eq!(manifests(&f.src).len(), 1);
}

#[test]
fn a_changed_file_is_failed_and_the_job_isnt_complete() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"not what it was")]);
    let report = copy(&f);
    assert_eq!(report.mhl_failed, [PathBuf::from("a.mov")]);
    assert!(!report.is_success());
    assert!(latest_manifest(&f.dest.join("A")).contains(r#"action="failed""#));
}

#[test]
fn nested_generations_are_referenced_by_the_root() {
    let f = fixture(&[("A001/a.mov", b"a"), ("notes.txt", b"n")]);
    history(&f.src.join("A001"), &[("a.mov", b"a")]);
    let report = copy(&f);
    assert!(report.is_success(), "{report:?}");
    let nested = f.dest.join("A/A001");
    let nested_name = manifests(&nested).pop().unwrap();
    let c4 = secopy_core::mhl::c4::c4(&fs::read(nested.join("ascmhl").join(&nested_name)).unwrap());
    let root = latest_manifest(&f.dest.join("A"));
    assert!(root.contains(&format!("<path>A001/ascmhl/{nested_name}</path>")));
    assert!(root.contains(&c4));
    // Each file is in its closest history only.
    assert!(root.contains("notes.txt") && !root.contains("A001/a.mov"));
    assert!(latest_manifest(&nested).contains(r#"action="verified""#));
}

#[test]
fn a_chain_changed_during_the_job_isnt_overwritten() {
    let f = fixture(&[("a.mov", b"aaa")]);
    write_files(&f.dest, &[("A/old.mov", b"old!")]);
    history(&f.dest.join("A"), &[("old.mov", b"old!")]);
    let (plan, result) = prepared(&f);
    let mhl = result.unwrap();
    // Another tool appends a generation after the plan was made.
    history(&f.dest.join("A"), &[("old.mov", b"old!")]);
    let chain = fs::read(f.dest.join("A/ascmhl/ascmhl_chain.xml")).unwrap();
    let report = run(&plan, Some(mhl)).0;
    assert!(report.mhl_error.is_some());
    assert!(!report.is_success());
    assert_eq!(
        fs::read(f.dest.join("A/ascmhl/ascmhl_chain.xml")).unwrap(),
        chain
    );
    assert_eq!(manifests(&f.dest.join("A")).len(), 2);
}

#[test]
fn a_write_that_fails_leaves_the_history_as_it_was() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.dest.join("A"), &[]);
    let (plan, result) = prepared(&f);
    let folder = f.dest.join("A/ascmhl");
    let chain = fs::read(folder.join("ascmhl_chain.xml")).unwrap();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    let report = run(&plan, Some(result.unwrap())).0;
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(report.mhl_error.is_some());
    assert_eq!(fs::read(folder.join("ascmhl_chain.xml")).unwrap(), chain);
    assert_eq!(manifests(&f.dest.join("A")).len(), 1);
}

#[test]
fn the_setting_off_writes_nothing() {
    let f = fixture(&[("a.mov", b"aaa")]);
    let (plan, _) = planned(&f, &ExtensionFilter::All, DiffersPolicy::KeepBoth);
    let report = run(&plan, None).0;
    assert!(report.is_success());
    assert!(report.mhl_written.is_empty());
    assert!(!f.dest.join("A/ascmhl").exists());
}

// Undo (Task 7).

#[test]
fn undo_reverts_the_mhl() {
    let f = fixture(&[("a.mov", b"aaa")]);
    write_files(&f.dest, &[("A/old.mov", b"old!")]);
    history(&f.dest.join("A"), &[("old.mov", b"old!")]);
    let chain = fs::read(f.dest.join("A/ascmhl/ascmhl_chain.xml")).unwrap();
    let (plan, result) = prepared(&f);
    let report = run(&plan, Some(result.unwrap())).0;
    assert_eq!(manifests(&f.dest.join("A")).len(), 2);
    let undone = undo(&plan, &report, None);
    assert!(undone.failed.is_empty(), "{:?}", undone.failed);
    assert_eq!(
        fs::read(f.dest.join("A/ascmhl/ascmhl_chain.xml")).unwrap(),
        chain
    );
    assert_eq!(manifests(&f.dest.join("A")).len(), 1);
    assert!(!f.dest.join("A/a.mov").exists());
    assert!(f.dest.join("A/old.mov").exists());
}

#[test]
fn undo_of_a_new_history_removes_its_folder() {
    let f = fixture(&[("a.mov", b"aaa")]);
    let (plan, result) = prepared(&f);
    let report = run(&plan, Some(result.unwrap())).0;
    assert!(f.dest.join("A/ascmhl").is_dir());
    undo(&plan, &report, None);
    assert!(!f.dest.join("A").exists());
}

// Final review fixes.

/// Review #4: the checksum file never lists a history Secopy then appends to (its own Verify
/// would call the copy changed).
#[test]
fn the_checksum_file_leaves_out_a_continued_history() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"aaa")]);
    let report = copy(&f);
    assert!(report.is_success(), "{report:?}");
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    assert!(sums.contains("A/a.mov"), "{sums}");
    assert!(!sums.contains("ascmhl"), "{sums}");
}

/// Review #5: what a generation's ignore list leaves out isn't recorded in it.
#[test]
fn ignored_files_arent_recorded() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"aaa")]);
    copy(&f);
    let text = latest_manifest(&f.dest.join("A"));
    let paths: Vec<&str> = text.lines().filter(|l| l.contains("<path")).collect();
    assert!(paths.iter().any(|l| l.contains(">a.mov<")), "{text}");
    assert!(!paths.iter().any(|l| l.contains("ascmhl")), "{paths:?}");
}

/// Review #6: a file the source's history lists that would land under another name (Keep
/// both) or not at all (Skip) blocks: the history would describe the wrong file.
#[test]
fn a_recorded_file_that_wouldnt_land_at_its_path_blocks() {
    for policy in [DiffersPolicy::KeepBoth, DiffersPolicy::Skip] {
        let f = fixture(&[("a.mov", b"aaa")]);
        history(&f.src, &[("a.mov", b"aaa")]);
        write_files(&f.dest, &[("A/a.mov", b"a different one")]);
        let (mut plan, inputs) = planned(&f, &ExtensionFilter::All, policy);
        assert!(
            blockers(prepare(&mut plan, &inputs)).contains(&MhlBlocker::Conflicts {
                path: f.dest.join("A/a.mov")
            }),
            "{policy:?}"
        );
    }
}

/// Review #7: a history whose manifest didn't arrive with the copy isn't appended to.
#[test]
fn a_source_history_that_didnt_arrive_whole_isnt_appended_to() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"aaa")]);
    let (plan, result) = prepared(&f);
    let mhl = result.unwrap();
    // A manifest gone from the source before it's copied: its copy fails, the chain's doesn't.
    let manifest = fs::read_dir(f.src.join("ascmhl"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "mhl"))
        .unwrap();
    fs::remove_file(&manifest).unwrap();
    let report = run(&plan, Some(mhl)).0;
    assert!(report.mhl_error.is_some());
    assert_eq!(
        fs::read(f.dest.join("A/ascmhl/ascmhl_chain.xml")).unwrap(),
        fs::read(f.src.join("ascmhl/ascmhl_chain.xml")).unwrap()
    );
}

/// Review #8: a nested history a file-type filter leaves out still counts.
#[test]
fn a_nested_history_left_out_by_a_filter_blocks() {
    let f = fixture(&[("A001/a.mov", b"a")]);
    history(&f.src.join("A001"), &[("a.mov", b"a")]);
    let movs = ExtensionFilter::Only([Some("mov".to_string())].into_iter().collect());
    let (mut plan, inputs) = planned(&f, &movs, DiffersPolicy::KeepBoth);
    assert!(
        blockers(prepare(&mut plan, &inputs)).contains(&MhlBlocker::LeavesOut {
            scope: f.dest.join("A/A001")
        })
    );
}

/// Review #8: a nested `ascmhl` without a chain in the source is damage.
#[test]
fn a_nested_source_history_without_a_chain_blocks() {
    let f = fixture(&[("A001/a.mov", b"a")]);
    fs::create_dir_all(f.src.join("A001/ascmhl")).unwrap();
    fs::write(
        f.src.join("A001/ascmhl/0001_A001_2026-10-01_080000Z.mhl"),
        b"x",
    )
    .unwrap();
    assert!(blockers(prepared(&f).1).contains(&MhlBlocker::Damaged {
        scope: f.src.join("A001"),
        damage: Damage::NoChain
    }));
}

/// Review #13: a folder in the destination that can't be read blocks.
#[test]
fn an_unreadable_folder_in_the_destination_blocks() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture(&[("a.mov", b"aaa")]);
    write_files(&f.dest, &[("A/locked/x.mov", b"x")]);
    let locked = f.dest.join("A/locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let result = prepared(&f).1;
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(blockers(result).contains(&MhlBlocker::Unreadable { path: locked }));
}

/// Review #14: a folder name a manifest's name can't carry blocks.
#[test]
fn a_scope_name_a_manifest_cant_carry_blocks() {
    for name in ["bad\u{1}", "back\\slash"] {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join(name);
        let dest = dir.path().join("dest");
        write_files(&src, &[("a.mov", b"a")]);
        fs::create_dir_all(&dest).unwrap();
        let f = Fixture {
            _dir: dir,
            src,
            dest,
        };
        assert!(
            blockers(prepared(&f).1).contains(&MhlBlocker::Unlistable {
                path: f.dest.join(name)
            }),
            "{name:?}"
        );
    }
}

/// Review #16: the saved report doesn't call a copy complete when its ASC MHL failed.
#[test]
fn the_report_says_when_the_mhl_failed() {
    let f = fixture(&[("a.mov", b"aaa")]);
    history(&f.src, &[("a.mov", b"not what it was")]);
    let (plan, result) = prepared(&f);
    let job = run(&plan, Some(result.unwrap())).0;
    let meta = secopy_core::report::JobMeta {
        app_version: "0".into(),
        source: "src".into(),
        verify: true,
        started: chrono::Local::now(),
        finished: chrono::Local::now(),
    };
    let report = secopy_core::report::Report::new(&plan, &job, &meta);
    assert_eq!(report.result, "1 file doesn't match its ASC MHL history");
}

/// #158: the ignore list goes into the history's own.
#[test]
fn the_ignore_list_is_in_the_manifest() {
    let f = fixture(&[("a.mov", b"aaa"), ("a.LRF", b"l")]);
    let lrf = Patterns::new(["*.LRF".to_string(), "[x]".to_string()]).unwrap();
    let (mut plan, inputs) = planned_with(&f, &ExtensionFilter::All, DiffersPolicy::KeepBoth, lrf);
    let mhl = prepare(&mut plan, &inputs).unwrap();
    let report = run(&plan, Some(mhl)).0;
    assert!(report.is_success(), "{report:?}");
    let text = latest_manifest(&f.dest.join("A"));
    assert!(text.contains("<pattern>*.[lL][rR][fF]</pattern>"), "{text}");
    assert!(text.contains("<pattern>[[][xX]]</pattern>"), "{text}");
    assert!(!text.contains("a.LRF"), "{text}");
}

/// #158: a source history that lists a file the list ignores can't be carried whole.
#[test]
fn a_source_history_listing_an_ignored_file_blocks() {
    let f = fixture(&[("a.mov", b"aaa"), ("a.LRF", b"l")]);
    history(&f.src, &[("a.mov", b"aaa"), ("a.LRF", b"l")]);
    let lrf = Patterns::new(["*.LRF".to_string()]).unwrap();
    let (mut plan, inputs) = planned_with(&f, &ExtensionFilter::All, DiffersPolicy::KeepBoth, lrf);
    assert!(
        blockers(prepare(&mut plan, &inputs)).contains(&MhlBlocker::LeavesOut {
            scope: f.dest.join("A")
        })
    );
}

/// Review #8: an ignored folder in the destination isn't entered, even when unreadable.
#[test]
fn an_ignored_unreadable_folder_in_the_destination_doesnt_block() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture(&[("a.mov", b"aaa")]);
    write_files(&f.dest, &[("A/cache/x.bin", b"x")]);
    let locked = f.dest.join("A/cache");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let cache = Patterns::new(["CACHE".to_string()]).unwrap();
    let (mut plan, inputs) =
        planned_with(&f, &ExtensionFilter::All, DiffersPolicy::KeepBoth, cache);
    let result = prepare(&mut plan, &inputs);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let mhl = result.expect("not blocked");
    assert!(mhl.to_read.is_empty());
}
