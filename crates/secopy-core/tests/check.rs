use std::fs;
use std::path::{Path, PathBuf};

use secopy_core::check;
use secopy_core::hash::Hash;
use secopy_core::ignore::Patterns;

#[test]
fn lines_are_parsed_and_bad_ones_named() {
    let text = "00000000000000000123456789abcdef  a/b.mov\nnot a line\n00000000000000000123456789ABCDEF  c.mov\n0000000000000000+123456789abcdef  d.mov\n";
    let (entries, bad) = check::parse(text);
    assert_eq!(
        entries,
        [
            (
                PathBuf::from("a/b.mov"),
                Hash::from_u128(0x0123_4567_89ab_cdef)
            ),
            (
                PathBuf::from("c.mov"),
                Hash::from_u128(0x0123_4567_89ab_cdef)
            ),
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
    let entries: Vec<(PathBuf, Hash)> = names
        .iter()
        .cloned()
        .zip([1, 2, 3].map(Hash::from_u128))
        .collect();
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
    let (entries, bad) = check::parse("00000000000000000000000000000001  a\r\n\r\n");
    assert_eq!(entries, [(PathBuf::from("a"), Hash::from_u128(1))]);
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
        entries.push((PathBuf::from(rel), secopy_core::hash::Hash::of(data)));
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
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
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

/// #154: a copy's ASC MHL history isn't media the checksum file forgot.
#[test]
fn an_ascmhl_folder_isnt_counted_as_not_checked() {
    let (_dir, root) = copy_of(&[("a.mov", b"a")]);
    fs::create_dir_all(root.join("ascmhl")).unwrap();
    fs::write(
        root.join("ascmhl/0001_Day01_2026-10-01_081500Z.mhl"),
        b"<x/>",
    )
    .unwrap();
    fs::write(root.join("ascmhl/ascmhl_chain.xml"), b"<x/>").unwrap();
    fs::create_dir_all(root.join("A001/ascmhl")).unwrap();
    fs::write(root.join("A001/ascmhl/ascmhl_chain.xml"), b"<x/>").unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    assert!(p.not_checked.is_empty(), "{:?}", p.not_checked);
}

#[test]
fn the_newest_checksum_file_wins() {
    let (_dir, root) = copy_of(&[("a.mov", b"old")]);
    fs::write(root.join("a.mov"), b"new").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(
        root.join("later.xxh128"),
        format!("{}  a.mov\n", secopy_core::hash::Hash::of(b"new")),
    )
    .unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.files[0].expected, secopy_core::hash::Hash::of(b"new"));
    assert_eq!(p.files[0].from, PathBuf::from("later.xxh128"));
}

#[test]
fn checksum_files_in_subdirectories_are_found() {
    let dir = tempfile::tempdir().unwrap();
    let (_a, day1) = copy_of(&[("x.mov", b"x")]);
    let drive = dir.path().join("drive");
    fs::create_dir_all(&drive).unwrap();
    fs::rename(&day1, drive.join("Day01")).unwrap();
    let p = check::plan(&drive, &Patterns::defaults()).unwrap();
    assert_eq!(p.files[0].rel, PathBuf::from("Day01/x.mov"));
}

/// Review focus 1.
#[test]
fn a_path_outside_the_directory_is_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.xxh128"),
        "00000000000000000000000000000001  ../secret\n00000000000000000000000000000002  /etc/hosts\n",
    )
    .unwrap();
    let p = check::plan(dir.path(), &secopy_core::ignore::Patterns::defaults()).unwrap();
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
        format!("{}  f.mov\ngarbage\n", secopy_core::hash::Hash::of(b"f")),
    )
    .unwrap();
    let p = check::plan(dir.path(), &secopy_core::ignore::Patterns::defaults()).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.problems.len(), 1);
    assert_eq!(p.problems[0].line, Some(2));
    assert!(p.not_checked.is_empty());
}

fn quick() -> check::CheckOptions {
    check::CheckOptions {
        keep_awake: false,
        ..check::CheckOptions::default()
    }
}

fn check_all(root: &Path) -> check::CheckReport {
    let p = check::plan(root, &Patterns::defaults()).unwrap();
    check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {})
}

#[test]
fn an_untouched_copy_is_intact() {
    let (_dir, root) = copy_of(&[("a.mov", b"a"), ("b/c.mov", b"cc")]);
    let r = check_all(&root);
    assert!(r.is_intact(), "{:?}", r.job.outcomes);
    assert_eq!(r.counts().intact, 2);
}

#[test]
fn changed_missing_and_unreadable_files_are_named() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, root) = copy_of(&[("a.mov", b"aaaa"), ("gone.mov", b"g"), ("locked.mov", b"l")]);
    fs::write(root.join("a.mov"), b"aaab").unwrap(); // same size, flipped byte
    fs::remove_file(root.join("gone.mov")).unwrap();
    fs::set_permissions(root.join("locked.mov"), fs::Permissions::from_mode(0o000)).unwrap();
    let r = check_all(&root);
    fs::set_permissions(root.join("locked.mov"), fs::Permissions::from_mode(0o644)).unwrap();
    let c = r.counts();
    assert_eq!((c.intact, c.changed, c.missing, c.failed), (0, 1, 1, 1));
    assert!(!r.is_intact());
}

/// #69 V5: a listed name that is now a link or a directory is a failure that says so, not
/// "missing", and never intact. Links aren't followed.
#[test]
fn a_listed_link_or_directory_is_a_failure_not_missing() {
    use secopy_core::error::FileError;
    use secopy_core::job::FileStatus;
    let (_dir, root) = copy_of(&[("link.mov", b"target"), ("dir.mov", b"d")]);
    fs::write(root.join("target.mov"), b"target").unwrap();
    fs::remove_file(root.join("link.mov")).unwrap();
    std::os::unix::fs::symlink(root.join("target.mov"), root.join("link.mov")).unwrap();
    fs::remove_file(root.join("dir.mov")).unwrap();
    fs::create_dir(root.join("dir.mov")).unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    let link = p.files.iter().find(|f| f.rel == Path::new("link.mov"));
    assert_eq!(link.unwrap().size, 0, "a link's target isn't counted");
    let r = check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {});
    let status = |name: &str| {
        let o = r.job.outcomes.iter().find(|o| o.rel == Path::new(name));
        o.unwrap().status.clone()
    };
    assert_eq!(status("link.mov"), FileStatus::Failed(FileError::IsLink));
    assert_eq!(
        status("dir.mov"),
        FileStatus::Failed(FileError::IsDirectory)
    );
    assert_eq!(
        FileError::IsLink.to_string(),
        "is a link, not checked (links aren't followed)"
    );
    assert_eq!(
        FileError::IsDirectory.to_string(),
        "is a directory, not a file"
    );
    let c = r.counts();
    assert_eq!((c.intact, c.changed, c.missing, c.failed), (0, 0, 0, 2));
    assert!(!r.is_intact());
}

/// #69 V1: small and large files go to different lanes; every one is still checked.
#[test]
fn small_and_large_files_are_all_checked() {
    let large = vec![7u8; (4 << 20) + 1];
    let (_dir, root) = copy_of(&[("a.mov", b"a"), ("big.mov", &large), ("c.mov", b"c")]);
    fs::write(root.join("c.mov"), b"x").unwrap();
    let r = check_all(&root);
    let c = r.counts();
    assert_eq!((c.intact, c.changed), (2, 1), "{:?}", r.job.outcomes);
    assert_eq!(r.job.not_started, 0);
}

/// Review focus 2.
#[test]
fn a_file_that_changed_size_is_changed() {
    let (_dir, root) = copy_of(&[("a.mov", b"short")]);
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    fs::write(root.join("a.mov"), b"longer than before").unwrap();
    let r = check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {});
    assert_eq!(r.counts().changed, 1);
}

/// Review focus 3.
#[test]
fn a_check_writes_nothing() {
    let (_dir, root) = copy_of(&[("a.mov", b"a"), ("sub/b.mov", b"b")]);
    let snapshot = |root: &Path| -> Vec<(PathBuf, std::time::SystemTime)> {
        let mut all: Vec<_> = walkdir::WalkDir::new(root)
            .into_iter()
            .map(|e| e.unwrap())
            .map(|e| {
                (
                    e.path().to_path_buf(),
                    e.metadata().unwrap().modified().unwrap(),
                )
            })
            .collect();
        all.sort();
        all
    };
    let before = snapshot(&root);
    check_all(&root);
    assert_eq!(snapshot(&root), before);
}

#[test]
fn problems_and_cancel_are_not_intact() {
    let (_dir, root) = copy_of(&[("a.mov", b"a")]);
    fs::write(root.join("bad.xxh128"), "garbage\n").unwrap();
    assert!(!check_all(&root).is_intact(), "a checksum file problem");
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    let control = secopy_core::job::JobControl::new();
    control.cancel();
    let r = check::run(&p, &quick(), &control, &|_| {});
    assert!(r.job.cancelled && !r.is_intact());
}

#[test]
fn progress_counts_bytes_checked() {
    let (_dir, root) = copy_of(&[("a.mov", &[1u8; 10_000]), ("b.mov", &[2u8; 5_000])]);
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    let last = std::sync::Mutex::new(None);
    check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|e| {
        if let secopy_core::job::Event::Progress(p) = e {
            *last.lock().unwrap() = Some(p);
        }
    });
    let p = last.into_inner().unwrap().expect("a final progress event");
    assert_eq!(
        (p.files_done, p.verified_bytes, p.total_bytes),
        (2, 15_000, 15_000)
    );
}

#[test]
fn the_report_says_what_was_checked() {
    let (_dir, root) = copy_of(&[("a.mov", b"aaaa"), ("b.mov", b"b")]);
    fs::write(root.join("a.mov"), b"aaab").unwrap();
    fs::write(root.join("extra.mov"), b"x").unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    let r = check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {});
    let meta = secopy_core::report::JobMeta {
        app_version: "test".into(),
        source: root.display().to_string(),
        verify: true,
        started: chrono::Local::now(),
        finished: chrono::Local::now(),
    };
    let report = secopy_core::report::Report::for_check(&p, &r, &meta);
    assert_eq!(report.mode, "check");
    assert_eq!(report.result, "1 changed");
    let text = report.to_text();
    assert!(
        text.contains("NOT CHECKED (no checksum)\n  extra.mov"),
        "{text}"
    );
    assert!(text.contains("changed since it was copied"), "{text}");
    assert!(report.to_json().contains("\"not_checked\""));
}

/// Final review: macOS's own directories on a drive (unreadable ones included) aren't
/// problems or "not checked".
#[test]
fn a_whole_drive_skips_the_systems_directories() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, root) = copy_of(&[("a.mov", b"a")]);
    fs::create_dir_all(root.join(".Trashes")).unwrap();
    fs::create_dir_all(root.join(".Spotlight-V100")).unwrap();
    fs::write(root.join(".Spotlight-V100/store.db"), b"x").unwrap();
    fs::set_permissions(root.join(".Trashes"), fs::Permissions::from_mode(0o000)).unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    fs::set_permissions(root.join(".Trashes"), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(p.problems.is_empty(), "{:?}", p.problems);
    assert!(p.not_checked.is_empty(), "{:?}", p.not_checked);
}

/// Final review: a listed path can't leave the directory through a directory link.
#[test]
fn a_path_through_a_link_is_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backup");
    let outside = dir.path().join("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("x.mov"), b"x").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    fs::write(
        root.join("a.xxh128"),
        format!("{}  link/x.mov\n", secopy_core::hash::Hash::of(b"x")),
    )
    .unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    assert!(p.files.is_empty(), "{:?}", p.files);
    assert_eq!(p.problems.len(), 1);
    assert!(p.problems[0].reason.contains("outside"), "{:?}", p.problems);
}

/// Final review: `./a`, as `find . | xargs xxhsum` writes it, is the file `a`.
#[test]
fn dot_slash_paths_are_the_same_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    fs::write(root.join("a.mov"), b"a").unwrap();
    fs::write(
        root.join("x.xxh128"),
        format!("{}  ./a.mov\n", secopy_core::hash::Hash::of(b"a")),
    )
    .unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.files[0].rel, PathBuf::from("a.mov"));
    assert!(p.not_checked.is_empty(), "{:?}", p.not_checked);
}

/// Final review: only Secopy's own reports are left out; a person's `sales_report.txt` is
/// "not checked" like any file.
#[test]
fn only_secopys_reports_are_left_out() {
    let (_dir, root) = copy_of(&[("a.mov", b"a")]);
    fs::write(root.join("sales_report.txt"), b"s").unwrap();
    fs::write(root.join("secopy_2026-09-28_120000_report.txt"), b"r").unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    assert_eq!(p.not_checked, [PathBuf::from("sales_report.txt")]);
}

/// #69 V7: a check's report says Verify, names the directory once, counts bytes read, and
/// claims no write, destination or checksum file.
#[test]
fn a_checks_report_is_a_verify_not_a_copy() {
    let (_dir, root) = copy_of(&[("a.mov", b"aaaa"), ("b.mov", b"bb"), ("gone.mov", b"g")]);
    fs::write(root.join("a.mov"), b"aaab").unwrap();
    fs::remove_file(root.join("gone.mov")).unwrap();
    let p = check::plan(&root, &Patterns::defaults()).unwrap();
    let r = check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {});
    let meta = secopy_core::report::JobMeta {
        app_version: "test".into(),
        source: root.display().to_string(),
        verify: true,
        started: chrono::Local::now(),
        finished: chrono::Local::now(),
    };
    let report = secopy_core::report::Report::for_check(&p, &r, &meta);
    let text = report.to_text();
    assert!(text.contains("Mode:         Verify\n"), "{text}");
    let dir_line = format!("Directory:    {}\n", root.display());
    assert!(text.contains(&dir_line), "{text}");
    assert!(text.contains("  1 intact\n"), "{text}");
    assert!(text.contains("Read:         6 bytes\n"), "{text}");
    for claim in [
        "Copy",
        "Source:",
        "Destination:",
        "Written:",
        "Checksum file",
    ] {
        assert!(!text.contains(claim), "{claim:?} in\n{text}");
    }
    let json: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(json["mode"], "check");
    assert_eq!(json["counts"]["bytes_written"], 0);
    assert_eq!(json["counts"]["bytes_read"], 6);
    assert_eq!(json["checksum_file"], serde_json::Value::Null);
    assert_eq!(json["checksum_off"], false);
}

/// #69 V3: each problem file in a check's report names the checksum file that listed it.
#[test]
fn the_report_names_the_checksum_file_that_listed_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    let (_a, day1) = copy_of(&[("a.mov", b"aaaa"), ("gone.mov", b"g"), ("ok.mov", b"o")]);
    let drive = dir.path().join("drive");
    fs::create_dir_all(&drive).unwrap();
    fs::rename(&day1, drive.join("Day01")).unwrap();
    fs::write(drive.join("Day01/a.mov"), b"aaab").unwrap();
    fs::remove_file(drive.join("Day01/gone.mov")).unwrap();
    let p = check::plan(&drive, &Patterns::defaults()).unwrap();
    let sums = secopy_core::checksum_file::slash_path(&p.checksum_files[0]);
    assert!(sums.starts_with("Day01/secopy_"), "{sums}");
    let r = check::run(&p, &quick(), &secopy_core::job::JobControl::new(), &|_| {});
    let meta = secopy_core::report::JobMeta {
        app_version: "test".into(),
        source: drive.display().to_string(),
        verify: true,
        started: chrono::Local::now(),
        finished: chrono::Local::now(),
    };
    let report = secopy_core::report::Report::for_check(&p, &r, &meta);
    let text = report.to_text();
    let hex = |data: &[u8]| Hash::of(data).to_hex();
    assert!(
        text.contains(&format!(
            "  Day01/a.mov: changed since it was copied (expected {}, found {}) (listed in {sums})",
            hex(b"aaaa"),
            hex(b"aaab"),
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!("  Day01/gone.mov: missing (listed in {sums})")),
        "{text}"
    );
    assert!(!text.contains("Day01/ok.mov: "), "{text}");
    let json: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let file = |name: &str| {
        let files = json["files"].as_array().unwrap();
        files.iter().find(|f| f["path"] == name).unwrap().clone()
    };
    assert_eq!(file("Day01/gone.mov")["status"], "missing");
    assert_eq!(file("Day01/gone.mov")["listed_in"], sums.as_str());
    assert_eq!(file("Day01/a.mov")["listed_in"], sums.as_str());
    assert_eq!(file("Day01/ok.mov")["listed_in"], sums.as_str());
}

/// #158: what the list names isn't "not checked".
#[test]
fn an_ignored_file_isnt_not_checked() {
    let (_dir, root) = copy_of(&[("a.mov", b"a")]);
    fs::write(root.join("extra.LRF"), b"x").unwrap();
    let lrf = Patterns::new(["*.LRF".to_string()]).unwrap();
    assert!(check::plan(&root, &lrf).unwrap().not_checked.is_empty());
    assert_eq!(
        check::plan(&root, &Patterns::defaults())
            .unwrap()
            .not_checked,
        [PathBuf::from("extra.LRF")]
    );
}

/// #158: a file a checksum file lists is checked, whatever the list says.
#[test]
fn a_listed_file_is_checked_whatever_the_list_says() {
    let (_dir, root) = copy_of(&[("a.LRF", b"a")]);
    let lrf = Patterns::new(["*.LRF".to_string()]).unwrap();
    let p = check::plan(&root, &lrf).unwrap();
    assert_eq!(p.files.len(), 1);
}

/// #178: XXH128 only; an old xxh64 checksum file is an ordinary file, never read.
#[test]
fn old_xxh64_files_are_not_read() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.mov"), b"a").unwrap();
    fs::write(dir.path().join("old.xxh64"), format!("{:016x}  a.mov\n", 1)).unwrap();
    let p = check::plan(dir.path(), &Patterns::defaults()).unwrap();
    assert!(p.checksum_files.is_empty(), "{:?}", p.checksum_files);
    // Nothing listed: the app, the queue and the CLI refuse to verify ("No checksum files
    // here"), so nothing is ever called intact.
    assert!(p.files.is_empty(), "{:?}", p.files);
    assert_eq!(
        p.not_checked,
        [PathBuf::from("a.mov"), PathBuf::from("old.xxh64")]
    );
}

/// #178: a 16-digit (xxh64) line in an XXH128 file is a problem, never read as a hash.
#[test]
fn a_16_digit_line_is_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.mov"), b"a").unwrap();
    fs::write(dir.path().join("x.xxh128"), "ef46db3751d8e999  a.mov\n").unwrap();
    let p = check::plan(dir.path(), &Patterns::defaults()).unwrap();
    assert_eq!(p.checksum_files.len(), 1, "the .xxh128 file is read");
    assert!(p.files.is_empty(), "{:?}", p.files);
    assert_eq!(p.problems.len(), 1);
    assert_eq!(p.problems[0].line, Some(1));
}

/// #178 review focus 2: one flipped byte is still a change, never intact.
#[test]
fn a_flipped_byte_still_fails_verify() {
    let dir = tempfile::tempdir().unwrap();
    let data = vec![0x42u8; 300];
    let mut flipped = data.clone();
    flipped[123] ^= 1;
    fs::write(dir.path().join("a.mov"), &flipped).unwrap();
    fs::write(
        dir.path().join("x.xxh128"),
        format!("{}  a.mov\n", secopy_core::hash::Hash::of(&data).to_hex()),
    )
    .unwrap();
    let r = check_all(dir.path());
    let c = r.counts();
    assert_eq!((c.intact, c.changed), (0, 1));
    assert!(!r.is_intact());
}
