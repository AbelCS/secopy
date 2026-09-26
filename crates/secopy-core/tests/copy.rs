mod common;

use std::cell::Cell;
use std::fs;
use std::sync::atomic::AtomicBool;

use common::pattern;
use secopy_core::copy::{CopyConfig, commit, copy_to_partial, partial_path};
use secopy_core::error::FileError;
use secopy_core::hash::hash_bytes;

fn small_buffers() -> CopyConfig {
    CopyConfig {
        buffer_size: 7,
        buffers: 2,
        uncached_write: false,
    }
}

#[test]
fn partial_name_is_hidden_and_next_to_the_final_file() {
    let p = partial_path(std::path::Path::new("/d/clips/A001.mov"));
    assert_eq!(p, std::path::Path::new("/d/clips/.A001.mov.secopy-partial"));
}

#[test]
fn small_file_is_copied_hashed_and_committed() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"hello secopy").unwrap();

    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(pc.hash, hash_bytes(b"hello secopy"));
    assert_eq!(pc.bytes, 12);
    assert!(!dst.exists(), "final name only appears on commit");

    commit(&pc.partial, &dst).unwrap();
    assert_eq!(fs::read(&dst).unwrap(), b"hello secopy");
    assert!(!pc.partial.exists());
}

#[test]
fn large_file_goes_through_the_pipeline_in_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    let data = pattern(1000);
    fs::write(&src, &data).unwrap();
    let last = Cell::new(0);

    let pc = copy_to_partial(
        &src,
        &dst,
        &small_buffers(),
        &|b| last.set(b),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(pc.hash, hash_bytes(&data));
    assert_eq!(last.get(), 1000);
    assert_eq!(fs::read(&pc.partial).unwrap(), data);
}

#[test]
fn empty_file_has_the_empty_hash() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"").unwrap();
    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(pc.hash, 0xef46_db37_51d8_e999);
    assert_eq!(pc.bytes, 0);
}

#[test]
fn cancel_removes_the_partial_file() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, pattern(1000)).unwrap();

    let err = copy_to_partial(
        &src,
        &dst,
        &small_buffers(),
        &|_| {},
        &AtomicBool::new(true),
    )
    .unwrap_err();
    assert_eq!(err, FileError::Cancelled);
    assert!(!partial_path(&dst).exists());
}

#[test]
fn missing_source_is_a_read_error_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("b.bin");
    let err = copy_to_partial(
        &dir.path().join("nope"),
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(matches!(err, FileError::ReadSource(_)));
    assert!(!partial_path(&dst).exists());
}

#[test]
fn missing_destination_folder_is_a_write_error() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.bin");
    fs::write(&src, b"x").unwrap();
    let err = copy_to_partial(
        &src,
        &dir.path().join("no/such/b.bin"),
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(matches!(err, FileError::WriteDest(_)));
}

#[test]
fn sizes_around_the_buffer_size_copy_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = small_buffers(); // 7-byte buffers
    for size in [6, 7, 8, 14, 15] {
        let (src, dst) = (
            dir.path().join(format!("s{size}")),
            dir.path().join(format!("d{size}")),
        );
        let data = pattern(size);
        fs::write(&src, &data).unwrap();
        let pc = copy_to_partial(&src, &dst, &cfg, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(pc.hash, hash_bytes(&data), "size {size}");
        assert_eq!(fs::read(&pc.partial).unwrap(), data, "size {size}");
    }
}

#[test]
fn another_writers_partial_file_is_never_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"mine").unwrap();
    fs::write(partial_path(&dst), b"other writer").unwrap();

    let err = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert_eq!(err, FileError::NameClash);
    assert_eq!(fs::read(partial_path(&dst)).unwrap(), b"other writer");
}

#[test]
fn commit_never_replaces_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"new").unwrap();
    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    fs::write(&dst, b"mine").unwrap();

    assert_eq!(commit(&pc.partial, &dst), Err(FileError::AlreadyExists));
    assert_eq!(fs::read(&dst).unwrap(), b"mine");
}
