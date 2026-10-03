//! FR-30: `cd DEST && xxhsum -c <file>` must pass. Needs `xxhsum` on PATH; CI sets
//! SECOPY_REQUIRE_XXHSUM=1 so the test cannot silently skip there.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{pattern, write_files};
use secopy_core::checksum_file;
use secopy_core::hash::Hash;

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
    let entries: Vec<(PathBuf, Hash)> = files
        .iter()
        .map(|(p, d)| (Path::new(p).to_path_buf(), Hash::of(d)))
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

/// #178 review focus 1: Copy & Verify with the real 4 MiB buffers, at the sizes where XXH128
/// or the buffers change path: every file verifies, its hash is the one-go hash, and
/// `xxhsum -c` accepts the `.xxh128` checksum file.
#[test]
fn copy_and_verify_hashes_match_xxhsum_at_the_edges() {
    use secopy_core::filter::ExtensionFilter;
    use secopy_core::hash::Hash;
    use secopy_core::job::{FileStatus, JobControl, JobOptions, run_job};
    use secopy_core::plan::{DiffersPolicy, Plan};
    use secopy_core::preflight::preflight;
    use secopy_core::scan::{ScanOptions, scan};
    use secopy_core::source::{DirMode, Source};

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let sizes = [0usize, 240, 241, (4 << 20) - 1, 4 << 20, (4 << 20) + 1];
    let files: Vec<(String, Vec<u8>)> = sizes
        .iter()
        .map(|&n| (format!("f{n}.bin"), pattern(n)))
        .collect();
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(p, d)| (p.as_str(), d.as_slice()))
        .collect();
    write_files(&src, &refs);
    let source = Source::Directory {
        path: src.clone(),
        mode: DirMode::FolderItself,
    };
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &dest).unwrap();
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    let opts = JobOptions {
        verify: true,
        keep_awake: false,
        ..JobOptions::default()
    };
    let report = run_job(&plan, &opts, &JobControl::new(), &|_| {});
    for (name, data) in &files {
        let o = report
            .outcomes
            .iter()
            .find(|o| o.final_rel.ends_with(name))
            .unwrap();
        assert_eq!(o.status, FileStatus::Verified, "{name}");
        assert_eq!(o.hash, Some(Hash::of(data)), "{name}");
    }
    let sums = report.checksum_file.expect("a checksum file");
    assert_eq!(sums.extension().unwrap(), "xxh128");
    if !xxhsum_available() {
        assert!(
            std::env::var("SECOPY_REQUIRE_XXHSUM").as_deref() != Ok("1"),
            "xxhsum is required but not installed"
        );
        return;
    }
    let out = Command::new("xxhsum")
        .arg("-c")
        .arg(sums.file_name().unwrap())
        .current_dir(sums.parent().unwrap())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "xxhsum -c failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
