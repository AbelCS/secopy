use std::fs;
use std::path::{Path, PathBuf};

use secopy_core::check;

#[test]
fn lines_are_parsed_and_bad_ones_named() {
    let text =
        "0123456789abcdef  a/b.mov\nnot a line\n0123456789ABCDEF  c.mov\n+123456789abcdef  d.mov\n";
    let (entries, bad) = check::parse(text);
    assert_eq!(
        entries,
        [
            (PathBuf::from("a/b.mov"), 0x0123_4567_89ab_cdef),
            (PathBuf::from("c.mov"), 0x0123_4567_89ab_cdef),
        ]
    );
    assert_eq!(bad.iter().map(|(n, _)| *n).collect::<Vec<_>>(), [2, 4]);
}

/// Review focus 4.
#[test]
fn escaped_names_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let names = [
        PathBuf::from("back\\slash.mov"),
        PathBuf::from("new\nline.mov"),
        PathBuf::from("plain.mov"),
    ];
    let entries: Vec<(PathBuf, u64)> = names.iter().cloned().zip([1, 2, 3]).collect();
    let path =
        secopy_core::checksum_file::write(dir.path(), &entries, chrono::Local::now()).unwrap();
    let (read, bad) = check::parse(&fs::read_to_string(path).unwrap());
    assert!(bad.is_empty(), "{bad:?}");
    let mut read = read;
    read.sort();
    let mut want = entries;
    want.sort();
    assert_eq!(read, want);
}

#[test]
fn crlf_and_empty_lines_are_fine() {
    let (entries, bad) = check::parse("0000000000000001  a\r\n\r\n");
    assert_eq!(entries, [(PathBuf::from("a"), 1)]);
    assert!(bad.is_empty());
    let _ = Path::new("");
}

fn copy_of(files: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Day01");
    let mut entries = Vec::new();
    for (rel, data) in files {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, data).unwrap();
        entries.push((PathBuf::from(rel), secopy_core::hash::hash_bytes(data)));
    }
    secopy_core::checksum_file::write(&root, &entries, chrono::Local::now()).unwrap();
    (dir, root)
}

#[test]
fn a_plan_lists_files_and_what_nothing_lists() {
    let (_dir, root) = copy_of(&[("CLIP/A001.mov", b"a"), ("B.mov", b"bb")]);
    fs::write(root.join("extra.mov"), b"x").unwrap();
    fs::write(root.join(".DS_Store"), b"x").unwrap();
    fs::create_dir_all(root.join(".secopy-archive/2026-01-01 10.00.00")).unwrap();
    fs::write(
        root.join(".secopy-archive/2026-01-01 10.00.00/old.mov"),
        b"x",
    )
    .unwrap();
    fs::write(root.join("secopy_x_report.txt"), b"r").unwrap();
    let p = check::plan(&root).unwrap();
    assert_eq!(p.checksum_files.len(), 1);
    let mut listed: Vec<_> = p.files.iter().map(|f| f.rel.clone()).collect();
    listed.sort();
    assert_eq!(
        listed,
        [PathBuf::from("B.mov"), PathBuf::from("CLIP/A001.mov")]
    );
    assert_eq!(p.not_checked, [PathBuf::from("extra.mov")]);
    assert_eq!(p.total_bytes, 3);
    assert!(p.problems.is_empty(), "{:?}", p.problems);
}

#[test]
fn the_newest_checksum_file_wins() {
    let (_dir, root) = copy_of(&[("a.mov", b"old")]);
    fs::write(root.join("a.mov"), b"new").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(
        root.join("later.xxh64"),
        format!("{:016x}  a.mov\n", secopy_core::hash::hash_bytes(b"new")),
    )
    .unwrap();
    let p = check::plan(&root).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.files[0].expected, secopy_core::hash::hash_bytes(b"new"));
    assert_eq!(p.files[0].from, PathBuf::from("later.xxh64"));
}

#[test]
fn checksum_files_in_subdirectories_are_found() {
    let dir = tempfile::tempdir().unwrap();
    let (_a, day1) = copy_of(&[("x.mov", b"x")]);
    let drive = dir.path().join("drive");
    fs::create_dir_all(&drive).unwrap();
    fs::rename(&day1, drive.join("Day01")).unwrap();
    let p = check::plan(&drive).unwrap();
    assert_eq!(p.files[0].rel, PathBuf::from("Day01/x.mov"));
}

/// Review focus 1.
#[test]
fn a_path_outside_the_directory_is_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.xxh64"),
        "0000000000000001  ../secret\n0000000000000002  /etc/hosts\n",
    )
    .unwrap();
    let p = check::plan(dir.path()).unwrap();
    assert!(p.files.is_empty());
    assert_eq!(p.problems.len(), 2, "{:?}", p.problems);
    assert!(p.problems.iter().all(|x| x.reason.contains("outside")));
}

#[test]
fn bad_lines_and_mirror_checksums() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("f.mov"), b"f").unwrap();
    fs::write(
        dir.path().join(check::MIRROR_CHECKSUMS),
        format!(
            "{:016x}  f.mov\ngarbage\n",
            secopy_core::hash::hash_bytes(b"f")
        ),
    )
    .unwrap();
    let p = check::plan(dir.path()).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.problems.len(), 1);
    assert_eq!(p.problems[0].line, Some(2));
    assert!(p.not_checked.is_empty());
}
