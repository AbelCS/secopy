mod common;

use std::cell::Cell;
use std::fs;

use common::pattern;
use secopy_core::control::JobControl;
use secopy_core::copy::{Commit, CopyConfig, copy_to_partial, partial_path};
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
        &JobControl::new(),
    )
    .unwrap();
    assert_eq!(pc.hash, hash_bytes(b"hello secopy"));
    assert_eq!(pc.bytes, 12);
    assert!(!dst.exists(), "final name only appears on commit");

    let partial = pc.partial.clone();
    pc.commit(&dst, Commit::NoReplace).unwrap();
    assert_eq!(fs::read(&dst).unwrap(), b"hello secopy");
    assert!(!partial.exists());
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
        &JobControl::new(),
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
        &JobControl::new(),
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

    let err = copy_to_partial(&src, &dst, &small_buffers(), &|_| {}, &cancelled()).unwrap_err();
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
        &JobControl::new(),
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
        &JobControl::new(),
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
        let pc = copy_to_partial(&src, &dst, &cfg, &|_| {}, &JobControl::new()).unwrap();
        assert_eq!(pc.hash, hash_bytes(&data), "size {size}");
        assert_eq!(fs::read(&pc.partial).unwrap(), data, "size {size}");
    }
}

#[test]
fn a_live_writers_partial_file_is_never_touched() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"mine").unwrap();
    // The first writer creates and locks its partial file, then waits at the pause.
    let paused = JobControl::new();
    paused.pause();
    std::thread::scope(|s| {
        let first =
            s.spawn(|| copy_to_partial(&src, &dst, &CopyConfig::default(), &|_| {}, &paused));
        while !partial_path(&dst).exists() {
            std::thread::yield_now();
        }
        let err = copy_to_partial(
            &src,
            &dst,
            &CopyConfig::default(),
            &|_| {},
            &JobControl::new(),
        )
        .unwrap_err();
        assert_eq!(err, FileError::PartialInUse);
        assert!(
            partial_path(&dst).exists(),
            "the live partial file is left alone"
        );
        paused.resume();
        let pc = first.join().unwrap().unwrap();
        pc.commit(&dst, Commit::NoReplace).unwrap();
    });
    assert_eq!(fs::read(&dst).unwrap(), b"mine");
}

#[test]
fn a_stale_partial_file_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"mine").unwrap();
    fs::write(partial_path(&dst), b"left by a crashed job").unwrap();
    age(&partial_path(&dst));
    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &JobControl::new(),
    )
    .unwrap();
    assert!(pc.removed_stale);
    assert_eq!(fs::read(&pc.partial).unwrap(), b"mine");
}

#[test]
fn long_names_get_a_short_partial_name() {
    let dir = tempfile::tempdir().unwrap();
    // 250 bytes: the name fits, but `.<name>.secopy-partial` would not.
    let long = format!("{}.mov", "a".repeat(246));
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join(&long));
    fs::write(&src, b"clip").unwrap();
    let partial = partial_path(&dst);
    let name = partial.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with(".secopy-") && name.ends_with(".partial"),
        "{name}"
    );
    assert_eq!(
        partial,
        partial_path(&dir.path().join(long.to_uppercase())),
        "the short name ignores case"
    );
    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &JobControl::new(),
    )
    .unwrap();
    pc.commit(&dst, Commit::NoReplace).unwrap();
    assert_eq!(fs::read(&dst).unwrap(), b"clip");
}

#[test]
fn replace_swaps_in_the_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"new").unwrap();
    fs::write(&dst, b"old").unwrap();
    let mut perms = fs::metadata(&dst).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&dst, perms).unwrap();
    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &JobControl::new(),
    )
    .unwrap();
    assert_eq!(pc.commit(&dst, Commit::Replace), Ok(dst.clone()));
    assert_eq!(fs::read(&dst).unwrap(), b"new");
}

#[test]
fn keep_both_moves_on_to_the_next_free_number() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.bin");
    let original = dir.path().join("clip.mov");
    let planned = dir.path().join("clip (1).mov");
    fs::write(&src, b"new").unwrap();
    // Someone took the planned name, and the next one, after pre-flight.
    fs::write(&planned, b"theirs").unwrap();
    fs::write(dir.path().join("clip (2).mov"), b"theirs too").unwrap();
    let pc = copy_to_partial(
        &src,
        &planned,
        &CopyConfig::default(),
        &|_| {},
        &JobControl::new(),
    )
    .unwrap();
    let got = pc
        .commit(
            &planned,
            Commit::KeepBoth {
                original: &original,
                n: 1,
            },
        )
        .unwrap();
    assert_eq!(got, dir.path().join("clip (3).mov"));
    assert_eq!(fs::read(&got).unwrap(), b"new");
    assert_eq!(fs::read(&planned).unwrap(), b"theirs");
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
        &JobControl::new(),
    )
    .unwrap();
    fs::write(&dst, b"mine").unwrap();

    let partial = pc.partial.clone();
    assert_eq!(
        pc.commit(&dst, Commit::NoReplace),
        Err(FileError::AlreadyExists)
    );
    assert_eq!(fs::read(&dst).unwrap(), b"mine");
    assert!(
        !partial.exists(),
        "a failed commit removes the partial file"
    );
}

/// Makes a file look as if it was written a minute ago.
fn age(path: &std::path::Path) {
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(old)
        .unwrap();
}

fn cancelled() -> JobControl {
    let control = JobControl::new();
    control.cancel();
    control
}

/// FAT stores local time: a partial file left in another time zone, or before a clock
/// change, can look hours in the future. It is still stale.
#[test]
fn a_stale_partial_file_dated_in_the_future_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"mine").unwrap();
    fs::write(partial_path(&dst), b"left by a crashed job").unwrap();
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
    fs::File::options()
        .write(true)
        .open(partial_path(&dst))
        .unwrap()
        .set_modified(future)
        .unwrap();
    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &JobControl::new(),
    )
    .unwrap();
    assert!(pc.removed_stale);
    assert_eq!(fs::read(&pc.partial).unwrap(), b"mine");
}
