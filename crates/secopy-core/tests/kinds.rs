//! Problems and errors carry their kind (#84): the app turns kinds into words in the user's
//! language, while reports and the CLI keep the English text, unchanged.

use std::fs;
use std::io;
use std::path::PathBuf;

use secopy_core::check::{self, ProblemKind};
use secopy_core::mirror::{
    self, Deleted, Guard, MirrorOptions, NotRemoved, PlanError, RemovalError,
};
use secopy_core::scan::{self, DriveRoot, ScanOptions, ScanProblemKind};
use secopy_core::source::{DirMode, Source};

fn opts() -> MirrorOptions {
    MirrorOptions {
        deleted: Deleted::Archive,
        deep_check: false,
        ignore: secopy_core::ignore::Patterns::defaults(),
    }
}

#[test]
fn a_scan_problem_says_what_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("gone.mov");
    let scanned = scan::scan(
        &Source::Files(vec![dir.path().to_path_buf(), missing]),
        &ScanOptions::default(),
    )
    .unwrap();
    assert_eq!(scanned.problems[0].kind, ScanProblemKind::NotAFile);
    assert_eq!(scanned.problems[0].message, "not a regular file");
    let ScanProblemKind::Io(io) = &scanned.problems[1].kind else {
        panic!()
    };
    assert_eq!(io.kind, io::ErrorKind::NotFound);
    assert_eq!(scanned.problems[1].message, io.message);
}

#[test]
fn a_drive_root_is_told_apart() {
    let e = scan::scan(
        &Source::Directory {
            path: PathBuf::from("/"),
            mode: DirMode::FolderItself,
        },
        &ScanOptions::default(),
    )
    .unwrap_err();
    assert!(
        e.get_ref()
            .and_then(|r| r.downcast_ref::<DriveRoot>())
            .is_some()
    );
    assert_eq!(
        e.to_string(),
        "a drive root has no directory name; copy only its contents instead"
    );
}

#[test]
fn a_check_problem_says_what_it_is() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.xxh64"),
        "not a line\nzz00000000000000  a.mov\n0000000000000001  ../out.mov\n",
    )
    .unwrap();
    let plan = check::plan(dir.path(), &secopy_core::ignore::Patterns::defaults()).unwrap();
    let kinds: Vec<_> = plan.problems.iter().map(|p| p.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            ProblemKind::NotALine,
            ProblemKind::NotAChecksum {
                hex: "zz00000000000000".into()
            },
            ProblemKind::Outside {
                path: PathBuf::from("../out.mov")
            },
        ]
    );
    let reasons: Vec<_> = plan.problems.iter().map(|p| p.reason.as_str()).collect();
    assert_eq!(
        reasons,
        vec![
            "not a \"<checksum>  <path>\" line",
            "\"zz00000000000000\" isn't an xxHash64 checksum",
            "../out.mov points outside the checked directory",
        ]
    );
}

#[test]
fn a_mirror_that_cant_be_planned_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("origin");
    let e = mirror::plan(&gone, dir.path(), &opts()).unwrap_err();
    assert_eq!(e, PlanError::OriginMissing(gone.clone()));
    assert_eq!(
        e.to_string(),
        format!("The origin isn't there: {}", gone.display())
    );
    let e = mirror::plan(dir.path(), dir.path(), &opts()).unwrap_err();
    assert_eq!(e, PlanError::Same);
    assert_eq!(
        e.to_string(),
        "The origin and the destination are the same directory."
    );
    let inner = dir.path().join("in");
    fs::create_dir(&inner).unwrap();
    assert_eq!(
        mirror::plan(dir.path(), &inner, &opts()).unwrap_err(),
        PlanError::DestinationInOrigin
    );
    assert_eq!(
        mirror::plan(&inner, dir.path(), &opts()).unwrap_err(),
        PlanError::OriginInDestination
    );
}

#[test]
fn a_guard_says_what_looks_wrong() {
    let dir = tempfile::tempdir().unwrap();
    let (o, d) = (dir.path().join("o"), dir.path().join("d"));
    fs::create_dir_all(&o).unwrap();
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("a.mov"), "a").unwrap();
    let plan = mirror::plan(&o, &d, &opts()).unwrap();
    assert_eq!(plan.guard, Some(Guard::EmptyOrigin));
    assert_eq!(
        plan.guard.unwrap().to_string(),
        "The origin has no files: every file in the destination would be removed."
    );
    assert_eq!(
        Guard::TooMany {
            removals: 3,
            files: 4
        }
        .to_string(),
        "3 of the destination's 4 files would be removed."
    );
}

#[test]
fn why_nothing_was_removed_keeps_its_words() {
    let left = "Files deleted in the origin were left in the destination:";
    assert_eq!(
        NotRemoved::Cancelled.to_string(),
        format!("{left} the mirror was cancelled.")
    );
    assert_eq!(
        NotRemoved::Stopped.to_string(),
        format!("{left} the mirror stopped.")
    );
    assert_eq!(
        NotRemoved::Failed(1).to_string(),
        format!("{left} 1 file failed.")
    );
    assert_eq!(
        NotRemoved::Failed(2).to_string(),
        format!("{left} 2 files failed.")
    );
    assert_eq!(
        NotRemoved::NotClean.to_string(),
        format!("{left} the copy didn't end cleanly.")
    );
    assert_eq!(
        RemovalError::ChangedAfterPreview.to_string(),
        "It changed after the preview, so it was kept."
    );
}
