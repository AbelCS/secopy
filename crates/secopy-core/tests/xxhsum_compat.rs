//! FR-30: `cd DEST && xxhsum -c <file>` must pass. Needs `xxhsum` on PATH; CI sets
//! SECOPY_REQUIRE_XXHSUM=1 so the test cannot silently skip there.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{pattern, write_files};
use secopy_core::checksum_file;
use secopy_core::hash::hash_bytes;

fn xxhsum_available() -> bool {
    Command::new("xxhsum").arg("--version").output().is_ok()
}

#[test]
fn xxhsum_accepts_our_checksum_file() {
    if !xxhsum_available() {
        assert!(
            std::env::var("SECOPY_REQUIRE_XXHSUM").as_deref() != Ok("1"),
            "xxhsum is required but not installed"
        );
        eprintln!("skipping: xxhsum not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("A001.mov", pattern(5000)),
        ("clips/B 002 (copy).mov", pattern(10)),
        ("clips/ñandú.wav", b"unicode".to_vec()),
        ("empty.bin", Vec::new()),
    ];
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    write_files(dir.path(), &refs);
    let entries: Vec<(PathBuf, u64)> = files
        .iter()
        .map(|(p, d)| (Path::new(p).to_path_buf(), hash_bytes(d)))
        .collect();

    let sums = checksum_file::write(dir.path(), &entries, chrono::Local::now()).unwrap();
    let out = Command::new("xxhsum")
        .arg("-c")
        .arg(sums.file_name().unwrap())
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "xxhsum -c failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // And it really checks: corrupt one file and xxhsum must fail.
    fs::write(dir.path().join("A001.mov"), b"tampered").unwrap();
    let out = Command::new("xxhsum")
        .arg("-c")
        .arg(sums.file_name().unwrap())
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
}
