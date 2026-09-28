use std::fs;
use std::path::Path;
use std::time::Duration;

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
