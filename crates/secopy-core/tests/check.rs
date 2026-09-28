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
