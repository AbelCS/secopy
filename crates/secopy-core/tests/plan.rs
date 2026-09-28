use std::fs;
use std::path::{Path, PathBuf};

use secopy_core::error::FileError;
use secopy_core::fsinfo::{FsInfo, FsKind, NameLimit};
use secopy_core::names::numbered;
use secopy_core::plan::{Action, DiffersPolicy, Plan, space_margin};
use secopy_core::preflight::{
    Blocker, Conflict, ConflictKind, FileProblem, Preflight, ProblemKind,
};
use secopy_core::scan::{ScanEntry, Selection};

fn entry(rel: &str, size: u64) -> ScanEntry {
    ScanEntry {
        source: PathBuf::from("/src").join(rel),
        rel: PathBuf::from(rel),
        size,
        ext: None,
        mtime: None,
    }
}

fn fs_with(free_bytes: u64) -> FsInfo {
    FsInfo {
        kind: FsKind::Apfs,
        case_sensitive: false,
        free_bytes,
        max_file_size: None,
        name_limit: NameLimit::Utf16Units(255),
        device: 1,
    }
}

/// 0: new.mov (no conflict), 1: same.mov (identical), 2: clip.mov (differs),
/// 3: bad.mov (pre-flight problem)
fn setup(dest: &Path) -> (Selection, Preflight) {
    let sel = Selection {
        files: vec![
            entry("new.mov", 100),
            entry("same.mov", 200),
            entry("clip.mov", 300),
            entry("bad.mov", 400),
        ],
        dirs: vec![],
        total_bytes: 1000,
        unread: vec![],
    };
    let pf = Preflight {
        dest: dest.to_path_buf(),
        fs: fs_with(u64::MAX),
        file_problems: vec![FileProblem {
            id: 3,
            kind: ProblemKind::NameClash,
        }],
        conflicts: vec![
            Conflict {
                id: 1,
                kind: ConflictKind::Identical,
            },
            Conflict {
                id: 2,
                kind: ConflictKind::Differs {
                    size: 1,
                    mtime: None,
                },
            },
        ],
        stale_partials: vec![],
        checksum_omissions: vec![],
        source_roots: vec![],
    };
    (sel, pf)
}

fn actions(plan: &Plan) -> Vec<Action> {
    plan.files.iter().map(|f| f.action.clone()).collect()
}

#[test]
fn each_policy_only_changes_files_that_differ() {
    let dir = tempfile::tempdir().unwrap();
    let (sel, pf) = setup(dir.path());
    let fail = Action::Fail(FileError::NameClash);
    let cases = [
        (
            DiffersPolicy::KeepBoth,
            Action::KeepBoth {
                rel: PathBuf::from("clip (1).mov"),
                n: 1,
            },
        ),
        (DiffersPolicy::Overwrite, Action::Overwrite),
        (DiffersPolicy::Skip, Action::SkipDiffers),
    ];
    for (policy, differs) in cases {
        let plan = Plan::resolve(&sel, &pf, policy);
        assert_eq!(
            actions(&plan),
            vec![Action::Copy, Action::SkipIdentical, differs, fail.clone()],
            "{policy:?}"
        );
    }
}

#[test]
fn keep_both_is_the_default() {
    assert_eq!(DiffersPolicy::default(), DiffersPolicy::KeepBoth);
}

#[test]
fn keep_both_skips_names_taken_on_disk_and_in_the_plan() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("clip (1).mov"), b"x").unwrap();
    let (mut sel, mut pf) = setup(dir.path());
    // A second file named "clip (2).mov" is already part of this copy.
    sel.files.push(entry("clip (2).mov", 10));
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(
        plan.files[2].action,
        Action::KeepBoth {
            rel: PathBuf::from("clip (3).mov"),
            n: 3,
        }
    );
    assert_eq!(plan.files[2].final_rel(), Path::new("clip (3).mov"));
    // Case-insensitive destinations treat "CLIP (3).MOV" as taken too.
    pf.fs.case_sensitive = false;
    sel.files.push(entry("CLIP (3).MOV", 10));
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(
        plan.files[2].action,
        Action::KeepBoth {
            rel: PathBuf::from("clip (4).mov"),
            n: 4,
        }
    );
}

#[test]
fn numbered_names_keep_the_extension_and_folder() {
    assert_eq!(
        numbered(Path::new("clips/A001.mov"), 1),
        Path::new("clips/A001 (1).mov")
    );
    assert_eq!(numbered(Path::new("README"), 2), Path::new("README (2)"));
    assert_eq!(
        numbered(Path::new("backup.tar.gz"), 1),
        Path::new("backup.tar (1).gz")
    );
}

#[test]
fn free_space_counts_only_written_files_plus_a_margin() {
    let dir = tempfile::tempdir().unwrap();
    let (sel, mut pf) = setup(dir.path());
    // Keep both writes new.mov and clip.mov: 400 bytes.
    let written = 400;
    let needed = written + space_margin(written);
    pf.fs.free_bytes = needed;
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(plan.bytes_to_write(), written);
    assert!(plan.blockers().is_empty());

    pf.fs.free_bytes = needed - 1;
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(
        plan.blockers(),
        vec![Blocker::NotEnoughSpace {
            needed,
            free: needed - 1
        }]
    );
    // Skipping the file that differs needs less.
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::Skip);
    assert!(plan.blockers().is_empty());
}

#[test]
fn the_margin_is_one_percent_but_at_least_64_mib() {
    assert_eq!(space_margin(0), 64 << 20);
    assert_eq!(space_margin(100 << 30), (100 << 30) / 100);
}

#[test]
fn names_the_checksum_file_cant_hold_are_marked() {
    let dir = tempfile::tempdir().unwrap();
    let (sel, mut pf) = setup(dir.path());
    pf.checksum_omissions = vec![0];
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    assert!(!plan.files[0].in_checksum_file);
    assert!(plan.files[1].in_checksum_file);
}
