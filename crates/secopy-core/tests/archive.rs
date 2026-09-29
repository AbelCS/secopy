//! A mirror's archive (#101): what it holds, and deleting it when the user asks.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use secopy_core::mirror::{self, ARCHIVE_DIR};

fn archived(dest: &std::path::Path, run: &str, files: &[(&str, usize)]) {
    for (rel, size) in files {
        let p = dest.join(ARCHIVE_DIR).join(run).join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![b'x'; *size]).unwrap();
    }
}

#[test]
fn an_archive_says_how_many_files_and_how_big() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        mirror::archive_summary(dir.path()).unwrap(),
        None,
        "no archive"
    );
    archived(
        dir.path(),
        "2026-09-01 10.00.00",
        &[("a.mov", 10), ("b/c.mov", 5)],
    );
    archived(dir.path(), "2026-09-02 10.00.00", &[("d.mov", 1)]);
    let s = mirror::archive_summary(dir.path()).unwrap().unwrap();
    assert_eq!((s.files, s.bytes), (3, 16));
    fs::remove_dir_all(dir.path().join(ARCHIVE_DIR)).unwrap();
    fs::create_dir(dir.path().join(ARCHIVE_DIR)).unwrap();
    assert_eq!(
        mirror::archive_summary(dir.path()).unwrap(),
        None,
        "an empty archive is none"
    );
}

#[test]
fn deleting_the_archive_removes_it_all() {
    let dir = tempfile::tempdir().unwrap();
    archived(
        dir.path(),
        "2026-09-01 10.00.00",
        &[("a.mov", 10), ("b/c.mov", 5)],
    );
    fs::write(dir.path().join("kept.mov"), "x").unwrap();
    let done = mirror::delete_archive(dir.path());
    assert_eq!((done.removed, done.failed.len()), (2, 0));
    assert!(!dir.path().join(ARCHIVE_DIR).exists());
    assert!(dir.path().join("kept.mov").exists(), "only the archive");
}

#[test]
fn an_archive_that_is_a_link_is_never_followed() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    fs::write(elsewhere.path().join("precious.mov"), "x").unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), dir.path().join(ARCHIVE_DIR)).unwrap();
    assert_eq!(mirror::archive_summary(dir.path()).unwrap(), None);
    let done = mirror::delete_archive(dir.path());
    assert_eq!(done.removed, 0);
    assert!(elsewhere.path().join("precious.mov").exists());
}

#[test]
fn files_that_cant_be_deleted_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    archived(
        dir.path(),
        "2026-09-01 10.00.00",
        &[("locked/a.mov", 3), ("b.mov", 1)],
    );
    let locked = dir
        .path()
        .join(ARCHIVE_DIR)
        .join("2026-09-01 10.00.00/locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    let done = mirror::delete_archive(dir.path());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(done.removed, 1);
    assert_eq!(done.failed.len(), 1);
    assert!(
        done.failed[0].0.ends_with("locked/a.mov"),
        "{:?}",
        done.failed
    );
    assert!(locked.join("a.mov").exists());
}

#[test]
fn a_destination_that_isnt_there_is_an_error_not_an_empty_archive() {
    let dir = tempfile::tempdir().unwrap();
    assert!(mirror::archive_summary(&dir.path().join("gone")).is_err());
}
