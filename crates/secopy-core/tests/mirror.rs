use std::fs;
use std::path::Path;
use std::time::Duration;

use secopy_core::job::{JobControl, JobOptions, run_job};
use secopy_core::mirror::{self, Change, Deleted, MirrorOptions};

fn write(root: &Path, files: &[(&str, &[u8])]) {
    for (rel, data) in files {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }
}

/// Same modification time on both sides, as a previous mirror run leaves them.
fn same_time(a: &Path, b: &Path) {
    let t = fs::metadata(a).unwrap().modified().unwrap();
    fs::File::options()
        .write(true)
        .open(b)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn opts() -> MirrorOptions {
    MirrorOptions {
        deleted: Deleted::Archive { days: 30 },
        deep_check: false,
    }
}

fn pair() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let (o, d) = (dir.path().join("origin"), dir.path().join("backup"));
    fs::create_dir_all(&o).unwrap();
    fs::create_dir_all(&d).unwrap();
    (dir, o, d)
}

#[test]
fn new_changed_unchanged_and_deleted() {
    let (_dir, o, d) = pair();
    write(
        &o,
        &[
            ("a.mov", b"same"),
            ("b.mov", b"changed!"),
            ("new/c.mov", b"new"),
        ],
    );
    write(
        &d,
        &[
            ("a.mov", b"same"),
            ("b.mov", b"old"),
            ("gone/d.mov", b"d"),
            ("gone/e.mov", b"e"),
        ],
    );
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let kinds: Vec<(String, Change)> = p
        .changes
        .iter()
        .map(|(i, c)| (p.copy.files[*i].entry.rel.display().to_string(), *c))
        .collect();
    assert!(kinds.contains(&("b.mov".into(), Change::Changed)));
    assert!(kinds.contains(&("new/c.mov".into(), Change::New)));
    assert_eq!(kinds.len(), 2, "a.mov is unchanged");
    let mut removals: Vec<String> = p.removals.iter().map(|r| r.display().to_string()).collect();
    removals.sort();
    assert_eq!(removals, ["gone/d.mov", "gone/e.mov"]);
    assert_eq!(p.remove_dirs, [std::path::PathBuf::from("gone")]);
    assert_eq!(p.guard, None, "2 of 4 is not more than half");
}

#[test]
fn dates_within_two_seconds_are_the_same() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"x")]);
    write(&d, &[("a.mov", b"x")]);
    let t = fs::metadata(o.join("a.mov")).unwrap().modified().unwrap();
    fs::File::options()
        .write(true)
        .open(d.join("a.mov"))
        .unwrap()
        .set_modified(t + Duration::from_secs(1))
        .unwrap();
    assert!(mirror::plan(&o, &d, &opts()).unwrap().changes.is_empty());
    fs::File::options()
        .write(true)
        .open(d.join("a.mov"))
        .unwrap()
        .set_modified(t + Duration::from_secs(5))
        .unwrap();
    assert_eq!(mirror::plan(&o, &d, &opts()).unwrap().changes.len(), 1);
}

#[test]
fn the_deep_check_finds_contents_that_differ() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"abcd")]);
    write(&d, &[("a.mov", b"abce")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    assert!(
        mirror::plan(&o, &d, &opts()).unwrap().changes.is_empty(),
        "size and date match"
    );
    let deep = MirrorOptions {
        deep_check: true,
        ..opts()
    };
    let p = mirror::plan(&o, &d, &deep).unwrap();
    assert_eq!(p.changes, [(0, Change::ContentsDiffer)]);
}

/// Review focus 2.
#[test]
fn system_files_and_the_archive_are_never_removed() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    write(
        &d,
        &[
            (".DS_Store", b"x"),
            ("._a.mov", b"x"),
            (".secopy-archive/2026-01-01 10.00.00/old.mov", b"x"),
            (".a.mov.secopy-partial", b"x"),
        ],
    );
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    assert!(p.removals.is_empty(), "{:?}", p.removals);
    assert!(p.remove_dirs.is_empty());
}

/// Review focus 3.
#[test]
fn a_name_the_origin_resolves_is_the_same_file() {
    let (_dir, o, d) = pair();
    write(&o, &[("img.jpg", b"x")]);
    write(&d, &[("IMG.jpg", b"x")]);
    same_time(&o.join("img.jpg"), &d.join("IMG.jpg"));
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let origin_is_case_insensitive = o.join("IMG.jpg").exists();
    if origin_is_case_insensitive {
        assert!(p.removals.is_empty(), "the same file, not deleted");
        assert!(p.changes.is_empty(), "and not copied again");
        assert_eq!(p.renames, [("IMG.jpg".into(), "img.jpg".into())]);
    }
}

/// Review focus 4.
#[test]
fn the_guard_trips_on_an_empty_origin_or_half_the_destination() {
    let (_dir, o, d) = pair();
    write(&d, &[("a.mov", b"a"), ("b.mov", b"b"), ("c.mov", b"c")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    assert_eq!(
        p.guard.map(|g| g.to_string()).as_deref(),
        Some("The origin has no files: every file in the destination would be removed.")
    );
    write(&o, &[("a.mov", b"a")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    assert_eq!(
        p.guard.map(|g| g.to_string()).as_deref(),
        Some("2 of the destination's 3 files would be removed.")
    );
    assert!(
        mirror::plan(&o.join("nope"), &d, &opts())
            .unwrap_err()
            .to_string()
            .contains("isn't there")
    );
}

fn run(
    p: &mirror::MirrorPlan,
    archive: Option<&Path>,
) -> (
    secopy_core::job::JobReport,
    Result<mirror::Finished, mirror::NotRemoved>,
) {
    let opts = JobOptions {
        write_checksum_file: false,
        keep_awake: false,
        archive_replaced: archive.map(Path::to_path_buf),
        ..JobOptions::default()
    };
    let report = run_job(&p.copy, &opts, &JobControl::new(), &|_| {});
    let removed = mirror::finish(p, &report, archive);
    (report, removed)
}

#[test]
fn a_run_makes_the_destination_match_and_keeps_what_it_removed() {
    let (dir, o, d) = pair();
    write(&o, &[("b.mov", b"changed!"), ("new/c.mov", b"new")]);
    write(&d, &[("b.mov", b"old"), ("gone/d.mov", b"d")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let archive = dir.path().join("backup/.secopy-archive/run");
    let (report, removed) = run(&p, Some(&archive));
    assert!(report.is_success());
    assert_eq!(removed.unwrap().removals.len(), 1);
    assert_eq!(fs::read(d.join("b.mov")).unwrap(), b"changed!");
    assert!(d.join("new/c.mov").exists() && !d.join("gone").exists());
    assert_eq!(fs::read(archive.join("gone/d.mov")).unwrap(), b"d");
    assert_eq!(
        fs::read(archive.join("b.mov")).unwrap(),
        b"old",
        "the replaced version too"
    );
    let again = mirror::plan(&o, &d, &opts()).unwrap();
    assert!(
        again.changes.is_empty() && again.removals.is_empty(),
        "in sync"
    );
}

#[test]
fn delete_mode_removes_for_good() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    write(&d, &[("a.mov", b"a"), ("x.mov", b"x"), ("y.mov", b"y")]);
    let p = mirror::plan(
        &o,
        &d,
        &MirrorOptions {
            deleted: Deleted::Delete,
            ..opts()
        },
    )
    .unwrap();
    let (_, removed) = run(&p, None);
    assert_eq!(removed.unwrap().removals.len(), 2);
    assert!(!d.join("x.mov").exists() && !d.join(".secopy-archive").exists());
}

/// Review focus 1.
#[test]
fn nothing_is_removed_after_a_failure() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    write(&d, &[("gone.mov", b"g")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    fs::remove_file(o.join("a.mov")).unwrap(); // vanished between plan and run
    let (report, removed) = run(&p, None);
    assert!(!report.is_success());
    assert_eq!(
        removed.unwrap_err().to_string(),
        "Files deleted in the origin were left in the destination: 1 file failed."
    );
    assert!(d.join("gone.mov").exists());
}

/// Review focus 5.
#[test]
fn a_file_back_in_the_origin_is_not_removed() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    write(&d, &[("a.mov", b"a"), ("back.mov", b"b")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    let p = mirror::plan(
        &o,
        &d,
        &MirrorOptions {
            deleted: Deleted::Delete,
            ..opts()
        },
    )
    .unwrap();
    write(&o, &[("back.mov", b"b")]);
    let (_, removed) = run(&p, None);
    assert!(removed.unwrap().removals.is_empty());
    assert!(d.join("back.mov").exists());
}

#[test]
fn archives_older_than_the_limit_are_cleaned_up() {
    let (_dir, _o, d) = pair();
    let now = chrono::Local::now();
    let old = mirror::archive_dir(&d, now - chrono::Duration::days(40));
    let recent = mirror::archive_dir(&d, now - chrono::Duration::days(3));
    for p in [&old, &recent] {
        write(p, &[("x.mov", b"x")]);
    }
    assert_eq!(mirror::clean_archives(&d, 30, now), 1);
    assert!(!old.exists() && recent.exists());
    // 0 days (a preset saved before they were required) never empties the archive (#113).
    assert_eq!(mirror::clean_archives(&d, 0, now), 0);
    assert!(recent.exists());
}

/// Final review 1: a directory the scan couldn't read isn't "deleted in the origin".
#[test]
fn an_origin_directory_that_cant_be_read_removes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a"), ("sub/s.mov", b"s")]);
    write(&d, &[("a.mov", b"a"), ("sub/s.mov", b"s")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    same_time(&o.join("sub/s.mov"), &d.join("sub/s.mov"));
    fs::set_permissions(o.join("sub"), fs::Permissions::from_mode(0o000)).unwrap();
    let p = mirror::plan(&o, &d, &opts());
    fs::set_permissions(o.join("sub"), fs::Permissions::from_mode(0o755)).unwrap();
    let p = p.unwrap();
    assert!(p.removals.is_empty(), "{:?}", p.removals);
    assert!(p.remove_dirs.is_empty(), "{:?}", p.remove_dirs);
    let guard = p
        .guard
        .expect("the preview says why nothing is removed")
        .to_string();
    assert!(guard.contains("couldn't be read"), "{guard}");
    assert!(guard.contains("Nothing is removed"), "{guard}");
}

/// Final review 2: a rename never lands on another file (a case-sensitive destination).
#[test]
fn a_rename_never_replaces_another_file() {
    let (_dir, o, d) = pair();
    write(&o, &[("keep.mov", b"k")]);
    write(
        &d,
        &[("keep.mov", b"k"), ("a.mp4", b"old"), ("A2.mp4", b"new")],
    );
    same_time(&o.join("keep.mov"), &d.join("keep.mov"));
    let mut p = mirror::plan(&o, &d, &opts()).unwrap();
    p.removals.clear();
    p.renames = vec![("a.mp4".into(), "A2.mp4".into())];
    let (_, removed) = run(&p, None);
    assert!(
        removed.unwrap().renamed.is_empty(),
        "not renamed, so not reported"
    );
    assert_eq!(fs::read(d.join("A2.mp4")).unwrap(), b"new");
}

/// Final review 4: what a NAS keeps in a share is not the origin's.
#[test]
fn nas_bookkeeping_in_the_destination_is_never_removed() {
    let (_dir, o, d) = pair();
    write(&o, &[("sub/x.jpg", b"x")]);
    write(&d, &[("sub/x.jpg", b"x")]);
    same_time(&o.join("sub/x.jpg"), &d.join("sub/x.jpg"));
    write(
        &d,
        &[
            ("@eaDir/x.jpg/SYNOPHOTO_THUMB_M.jpg", b"t"),
            ("sub/@eaDir/x.jpg/SYNOPHOTO_THUMB_S.jpg", b"t"),
            ("#recycle/old.jpg", b"r"),
            ("#snapshot/x", b"s"),
            (".@__thumb/x.jpg", b"t"),
            (".AppleDouble/x.jpg", b"a"),
            ("Network Trash Folder/x", b"n"),
        ],
    );
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    assert!(p.removals.is_empty(), "{:?}", p.removals);
    assert!(p.remove_dirs.is_empty(), "{:?}", p.remove_dirs);
    assert_eq!(p.destination_files, 1);
    assert_eq!(p.guard, None);
}

/// #57: the report can say which names were changed to the origin's spelling.
#[test]
fn a_rename_is_reported() {
    let (_dir, o, d) = pair();
    write(&o, &[("img.jpg", b"x")]);
    write(&d, &[("IMG.jpg", b"x")]);
    same_time(&o.join("img.jpg"), &d.join("IMG.jpg"));
    if !o.join("IMG.jpg").exists() {
        return; // a case-sensitive volume: nothing to rename
    }
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let (_, finished) = run(&p, None);
    assert_eq!(
        finished.unwrap().renamed,
        [("IMG.jpg".into(), "img.jpg".into())]
    );
}

/// #57: one inside the other is refused however the path is written.
#[test]
fn the_origin_and_destination_cant_hold_each_other() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a"), ("sub/b.mov", b"b")]);
    let err =
        |origin: &Path, dest: &Path| mirror::plan(origin, dest, &opts()).unwrap_err().to_string();
    assert_eq!(
        err(&o, &o),
        "The origin and the destination are the same directory."
    );
    assert_eq!(
        err(&o, &o.join("sub")),
        "The destination can't be inside the origin."
    );
    fs::create_dir_all(d.join("in")).unwrap();
    write(&d, &[("in/c.mov", b"c")]);
    assert_eq!(
        err(&d.join("in"), &d),
        "The origin can't be inside the destination."
    );
    let shouted = o.parent().unwrap().join("ORIGIN");
    if shouted.exists() {
        // A case-insensitive volume: the same directory spelled otherwise.
        assert_eq!(
            err(&o, &shouted.join("sub")),
            "The destination can't be inside the origin."
        );
    }
}

/// #57: the deep check reports its progress and can be cancelled.
#[test]
fn the_deep_check_reports_progress_and_can_be_cancelled() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a"), ("b.mov", b"b")]);
    write(&d, &[("a.mov", b"a"), ("b.mov", b"b")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    same_time(&o.join("b.mov"), &d.join("b.mov"));
    let deep = MirrorOptions {
        deep_check: true,
        ..opts()
    };
    let seen = std::sync::Mutex::new(Vec::new());
    let control = JobControl::new();
    mirror::plan_watched(&o, &d, &deep, &control, &|done, total| {
        seen.lock().unwrap().push((done, total))
    })
    .unwrap();
    assert_eq!(*seen.lock().unwrap(), [(0, 2), (1, 2), (2, 2)]);
    control.cancel();
    assert_eq!(
        mirror::plan_watched(&o, &d, &deep, &control, &|_, _| {})
            .unwrap_err()
            .to_string(),
        "Cancelled."
    );
}

/// #57 review: a destination reached through a symlink into the origin is refused too.
#[test]
fn a_symlink_into_the_origin_is_refused() {
    let (dir, o, _) = pair();
    write(&o, &[("a.mov", b"a"), ("sub/b.mov", b"b")]);
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(o.join("sub"), &link).unwrap();
    assert_eq!(
        mirror::plan(&o, &link.join("new"), &opts())
            .unwrap_err()
            .to_string(),
        "The destination can't be inside the origin."
    );
}

/// #58: two runs in the same second get their own archive directories, so one never
/// overwrites what the other archived; both are cleaned up in time.
#[test]
fn each_run_gets_its_own_archive_directory() {
    let (_dir, _o, d) = pair();
    let then = chrono::Local::now() - chrono::Duration::days(40);
    let first = mirror::archive_dir(&d, then);
    write(&first, &[("x.mov", b"first")]);
    let second = mirror::archive_dir(&d, then);
    assert_ne!(first, second);
    write(&second, &[("x.mov", b"second")]);
    assert_eq!(fs::read(first.join("x.mov")).unwrap(), b"first");
    assert_eq!(mirror::clean_archives(&d, 30, chrono::Local::now()), 2);
}

/// #58: a `.secopy-archive` that is a link leads outside the destination: cleaning up never
/// follows it, and a mirror that archives refuses to run.
#[test]
fn a_linked_archive_is_never_followed() {
    let (dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    let elsewhere = dir.path().join("elsewhere");
    let old = mirror::archive_dir(
        &elsewhere,
        chrono::Local::now() - chrono::Duration::days(40),
    );
    write(&old, &[("precious.mov", b"p")]);
    std::os::unix::fs::symlink(
        elsewhere.join(mirror::ARCHIVE_DIR),
        d.join(mirror::ARCHIVE_DIR),
    )
    .unwrap();
    assert_eq!(mirror::clean_archives(&d, 30, chrono::Local::now()), 0);
    assert!(old.join("precious.mov").exists());
    let err = mirror::plan(&o, &d, &opts()).unwrap_err().to_string();
    assert!(err.contains(".secopy-archive"), "{err}");
}

/// #58: a destination file that changed after the preview (another app wrote it) isn't the
/// one the preview listed: it is kept, and the run says so.
#[test]
fn a_destination_file_changed_since_the_preview_is_kept() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a"), ("b.mov", b"b")]);
    write(
        &d,
        &[("a.mov", b"a"), ("b.mov", b"b"), ("gone.mov", b"old")],
    );
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    same_time(&o.join("b.mov"), &d.join("b.mov"));
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    assert_eq!(p.removals.len(), 1);
    fs::write(d.join("gone.mov"), b"somebody's new work").unwrap();
    let (_, finished) = run(&p, None);
    let removals = finished.unwrap().removals;
    let why = removals[0].result.as_ref().unwrap_err().to_string();
    assert!(why.contains("changed"), "{why}");
    assert_eq!(
        fs::read(d.join("gone.mov")).unwrap(),
        b"somebody's new work"
    );
}

/// #58: the report (text and JSON alike) has the removals, and a removal that failed means
/// the result isn't "complete".
#[test]
fn the_report_has_the_removals_and_their_failures() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a"), ("b.mov", b"b")]);
    write(&d, &[("a.mov", b"a"), ("b.mov", b"b"), ("gone.mov", b"g")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    same_time(&o.join("b.mov"), &d.join("b.mov"));
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let (report, finished) = run(&p, None);
    let meta = secopy_core::report::JobMeta {
        app_version: "test".into(),
        source: "o".into(),
        verify: true,
        started: chrono::Local::now(),
        finished: chrono::Local::now(),
    };
    let ok = secopy_core::report::Report::new(&p.copy, &report, &meta)
        .with_mirror(mirror::report_part(&finished, false));
    assert_eq!(ok.result, "complete");
    assert!(ok.to_json().contains("\"removed\""), "{}", ok.to_json());
    assert!(
        ok.to_text()
            .contains("Removed from the destination (deleted): 1")
    );
    let mut failed = finished.unwrap();
    failed.removals[0].result = Err(mirror::RemovalError::Io(secopy_core::error::IoFailure {
        kind: std::io::ErrorKind::PermissionDenied,
        message: "Permission denied".into(),
    }));
    let bad = secopy_core::report::Report::new(&p.copy, &report, &meta)
        .with_mirror(mirror::report_part(&Ok(failed), false));
    assert_eq!(bad.result, "1 file couldn't be removed");
    assert!(bad.to_json().contains("Permission denied"));
    assert!(
        bad.to_text()
            .contains("Not removed: 1\n  gone.mov: Permission denied"),
        "{}",
        bad.to_text()
    );
    let renamed = mirror::Finished {
        removals: vec![],
        renamed: vec![("IMG.jpg".into(), "img.jpg".into())],
    };
    let part = mirror::report_part(&Ok(renamed), true);
    let text = secopy_core::report::Report::new(&p.copy, &report, &meta)
        .with_mirror(part)
        .to_text();
    assert!(
        text.contains("Removed from the destination (archived): 0"),
        "{text}"
    );
    assert!(
        text.contains("Renamed to match the origin: 1\n  IMG.jpg → img.jpg\n"),
        "{text}"
    );
}

/// #58 check: removals wait for a job that ended cleanly in every way, not only without
/// failed files (here an empty directory couldn't be created).
#[test]
fn nothing_is_removed_after_any_problem() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    fs::create_dir_all(o.join("EMPTY")).unwrap();
    write(
        &d,
        &[
            ("a.mov", b"a"),
            ("gone.mov", b"g"),
            ("EMPTY", b"a file in the way"),
        ],
    );
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let (report, finished) = run(&p, None);
    assert!(!report.dir_errors.is_empty(), "{report:?}");
    assert!(finished.is_err());
    assert!(d.join("gone.mov").exists());
}

/// #58 check: a file replaced after the preview by another with the same size and time
/// (another app saving through a new file) is still a different file: kept.
#[test]
fn a_replaced_file_with_the_same_size_and_time_is_kept() {
    let (dir, o, d) = pair();
    write(&o, &[("a.mov", b"a"), ("b.mov", b"b")]);
    write(
        &d,
        &[("a.mov", b"a"), ("b.mov", b"b"), ("gone.mov", b"old")],
    );
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    same_time(&o.join("b.mov"), &d.join("b.mov"));
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let new = dir.path().join("new.mov");
    fs::write(&new, b"new").unwrap();
    same_time(&d.join("gone.mov"), &new);
    fs::rename(&new, d.join("gone.mov")).unwrap();
    let (_, finished) = run(&p, None);
    assert!(finished.unwrap().removals[0].result.is_err());
    assert_eq!(fs::read(d.join("gone.mov")).unwrap(), b"new");
}

#[test]
fn a_mirror_keeps_a_checksum_file_a_check_can_use() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a"), ("new/b.mov", b"b")]);
    write(&d, &[("gone.mov", b"g"), ("a.mov", b"old a")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let (report, finished) = run(&p, None);
    let finished = finished.unwrap();
    mirror::write_checksums(&p, &report, &finished).unwrap();
    let text = fs::read_to_string(d.join(secopy_core::check::MIRROR_CHECKSUMS)).unwrap();
    let (entries, bad) = secopy_core::check::parse(&text);
    assert!(bad.is_empty());
    let mut names: Vec<_> = entries
        .iter()
        .map(|(p, _)| p.display().to_string())
        .collect();
    names.sort();
    assert_eq!(names, ["a.mov", "new/b.mov"]);
    let plan = secopy_core::check::plan(&d).unwrap();
    let r = secopy_core::check::run(
        &plan,
        &secopy_core::check::CheckOptions {
            keep_awake: false,
            ..Default::default()
        },
        &JobControl::new(),
        &|_| {},
    );
    assert!(r.is_intact(), "{:?}", r.job.outcomes);
    assert!(!d.join(".secopy-checksums.partial").exists());
}

/// Review focus 5.
#[test]
fn a_failed_mirror_keeps_the_previous_checksums() {
    let (_dir, o, d) = pair();
    write(
        &d,
        &[(".secopy-checksums.xxh64", b"0000000000000001  a.mov\n")],
    );
    write(&o, &[("a.mov", b"a")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    fs::remove_file(o.join("a.mov")).unwrap(); // vanished: the copy phase fails
    let (_, finished) = run(&p, None);
    assert!(finished.is_err());
    // The caller writes checksums only after Ok(finished); the file is as it was.
    assert_eq!(
        fs::read(d.join(".secopy-checksums.xxh64")).unwrap(),
        b"0000000000000001  a.mov\n"
    );
}

#[test]
fn the_checksum_file_is_never_planned_for_removal() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    write(&d, &[("a.mov", b"a"), (".secopy-checksums.xxh64", b"x")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    assert!(p.removals.is_empty(), "{:?}", p.removals);
}

#[test]
fn a_deep_check_records_the_hashes_it_compared() {
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    write(&d, &[("a.mov", b"a")]);
    same_time(&o.join("a.mov"), &d.join("a.mov"));
    let deep = MirrorOptions {
        deep_check: true,
        ..opts()
    };
    let p = mirror::plan(&o, &d, &deep).unwrap();
    assert_eq!(
        p.same.get(Path::new("a.mov")),
        Some(&secopy_core::hash::hash_bytes(b"a"))
    );
}

fn check_of(d: &Path) -> secopy_core::check::CheckReport {
    let plan = secopy_core::check::plan(d).unwrap();
    secopy_core::check::run(
        &plan,
        &secopy_core::check::CheckOptions {
            keep_awake: false,
            ..Default::default()
        },
        &JobControl::new(),
        &|_| {},
    )
}

/// Final review: a file renamed (letter case) and changed in the origin keeps its new hash,
/// and a file gone from both sides leaves the checksum file.
#[test]
fn the_checksum_file_follows_renames_and_drops_gone_files() {
    let (_dir, o, d) = pair();
    write(&o, &[("A.MOV", b"new contents")]);
    write(&d, &[("a.mov", b"old")]);
    let old = secopy_core::hash::hash_bytes(b"old");
    fs::write(
        d.join(".secopy-checksums.xxh64"),
        format!("{old:016x}  a.mov\n{old:016x}  b.mov\n"),
    )
    .unwrap();
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let (report, finished) = run(&p, None);
    mirror::write_checksums(&p, &report, &finished.unwrap()).unwrap();
    let r = check_of(&d);
    assert!(r.is_intact(), "{:?} {:?}", r.job.outcomes, r.problems);
}

/// Final review: a link planted where the temporary file goes is never written through.
#[test]
fn the_temporary_file_is_never_written_through_a_link() {
    let (dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    let victim = dir.path().join("victim.txt");
    fs::write(&victim, b"keep me").unwrap();
    std::os::unix::fs::symlink(&victim, d.join(".secopy-checksums.partial")).unwrap();
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let (report, finished) = run(&p, None);
    mirror::write_checksums(&p, &report, &finished.unwrap()).unwrap();
    assert_eq!(fs::read(&victim).unwrap(), b"keep me");
    assert!(check_of(&d).is_intact());
}

/// Final review: a previous checksum file that can't be read is kept, not replaced.
#[test]
fn an_unreadable_checksum_file_is_kept() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, o, d) = pair();
    write(&o, &[("a.mov", b"a")]);
    let sums = d.join(".secopy-checksums.xxh64");
    fs::write(&sums, b"0000000000000001  old.mov\n").unwrap();
    fs::set_permissions(&sums, fs::Permissions::from_mode(0o000)).unwrap();
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    let (report, finished) = run(&p, None);
    let result = mirror::write_checksums(&p, &report, &finished.unwrap());
    fs::set_permissions(&sums, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(&sums).unwrap(), b"0000000000000001  old.mov\n");
}

/// QA review (#114): a folder moved to another disk, left as a link. Links aren't followed, so
/// what's under it can't be told apart from deleted: with that disk unplugged, its backup stays.
#[test]
fn files_under_an_origin_link_are_never_removed() {
    let (dir, o, d) = pair();
    let other = dir.path().join("Other");
    write(&o, &[("a.mov", b"a")]);
    write(&other, &[("x.mov", b"x")]);
    std::os::unix::fs::symlink(&other, o.join("Old")).unwrap();
    write(
        &d,
        &[("a.mov", b"a"), ("Old/x.mov", b"x"), ("gone.mov", b"g")],
    );
    let delete = MirrorOptions {
        deleted: Deleted::Delete,
        ..opts()
    };
    let online = mirror::plan(&o, &d, &delete).unwrap();
    assert_eq!(online.removals, vec![std::path::PathBuf::from("gone.mov")]);
    fs::remove_dir_all(&other).unwrap(); // the other disk is unplugged
    let offline = mirror::plan(&o, &d, &delete).unwrap();
    assert_eq!(offline.removals, vec![std::path::PathBuf::from("gone.mov")]);
    let report = run_job(
        &offline.copy,
        &JobOptions::default(),
        &JobControl::new(),
        &|_| {},
    );
    mirror::finish(&offline, &report, None).unwrap();
    assert!(d.join("Old/x.mov").exists());
    assert!(!d.join("gone.mov").exists());
}
