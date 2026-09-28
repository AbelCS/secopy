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
        p.guard.as_deref(),
        Some("The origin has no files: every file in the destination would be removed.")
    );
    write(&o, &[("a.mov", b"a")]);
    let p = mirror::plan(&o, &d, &opts()).unwrap();
    assert_eq!(
        p.guard.as_deref(),
        Some("2 of the destination's 3 files would be removed.")
    );
    assert!(
        mirror::plan(&o.join("nope"), &d, &opts())
            .unwrap_err()
            .contains("isn't there")
    );
}

fn run(
    p: &mirror::MirrorPlan,
    archive: Option<&Path>,
) -> (
    secopy_core::job::JobReport,
    Result<mirror::Finished, String>,
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
    assert_eq!(removed.unwrap_err(), "Nothing was removed: 1 file failed.");
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
}

/// Final review 1: a directory the scan couldn't read isn't "deleted in the origin".
#[cfg(unix)]
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
    let guard = p.guard.expect("the preview says why nothing is removed");
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
    let err = |origin: &Path, dest: &Path| mirror::plan(origin, dest, &opts()).unwrap_err();
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
        mirror::plan_watched(&o, &d, &deep, &control, &|_, _| {}).unwrap_err(),
        "Cancelled."
    );
}

/// #57 review: a destination reached through a symlink into the origin is refused too.
#[cfg(unix)]
#[test]
fn a_symlink_into_the_origin_is_refused() {
    let (dir, o, _) = pair();
    write(&o, &[("a.mov", b"a"), ("sub/b.mov", b"b")]);
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(o.join("sub"), &link).unwrap();
    assert_eq!(
        mirror::plan(&o, &link.join("new"), &opts()).unwrap_err(),
        "The destination can't be inside the origin."
    );
}
