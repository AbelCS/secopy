mod common;

use std::cell::Cell;
use std::fs;

use common::pattern;
use secopy_core::control::JobControl;
use secopy_core::error::FileError;
use secopy_core::hash::hash_bytes;
use secopy_core::verify::{CacheBypass, hash_from_device};

#[test]
fn hashes_the_file_in_chunks_and_reports_progress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    // Not a multiple of 4096, so the last unbuffered read is short.
    let data = pattern(3 * 4096 + 123);
    fs::write(&path, &data).unwrap();
    let last = Cell::new(0);

    let (hash, _) = hash_from_device(&path, 4096, &|b| last.set(b), &JobControl::new()).unwrap();
    assert_eq!(hash, hash_bytes(&data));
    assert_eq!(last.get(), data.len() as u64);
}

#[test]
fn cache_bypass_is_active_on_local_disks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    fs::write(&path, b"data").unwrap();
    let (_, bypass) = hash_from_device(&path, 4096, &|_| {}, &JobControl::new()).unwrap();
    assert_eq!(bypass, CacheBypass::Active);
}

#[test]
fn empty_file_hashes_to_the_empty_hash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    fs::write(&path, b"").unwrap();
    let (hash, _) = hash_from_device(&path, 4096, &|_| {}, &JobControl::new()).unwrap();
    assert_eq!(hash, 0xef46_db37_51d8_e999);
}

#[test]
fn missing_file_is_a_read_back_error() {
    let dir = tempfile::tempdir().unwrap();
    let err =
        hash_from_device(&dir.path().join("nope"), 4096, &|_| {}, &JobControl::new()).unwrap_err();
    assert!(matches!(err, FileError::ReadBack(_)));
}

#[test]
fn cancel_stops_verification() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    fs::write(&path, pattern(10_000)).unwrap();
    let err = hash_from_device(&path, 4096, &|_| {}, &cancelled()).unwrap_err();
    assert_eq!(err, FileError::Cancelled);
}

fn cancelled() -> JobControl {
    let control = JobControl::new();
    control.cancel();
    control
}
