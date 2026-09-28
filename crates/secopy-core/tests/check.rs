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

fn quick() -> check::CheckOptions {
    check::CheckOptions {
        keep_awake: false,
        ..check::CheckOptions::default()
    }
}

fn check_all(root: &Path) -> check::CheckReport {
    let p = check::plan(root).unwrap();
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
    let p = check::plan(&root).unwrap();
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

/// Review focus 2.
#[test]
fn a_file_that_changed_size_is_changed() {
    let (_dir, root) = copy_of(&[("a.mov", b"short")]);
    let p = check::plan(&root).unwrap();
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
    fs::write(root.join("bad.xxh64"), "garbage\n").unwrap();
    assert!(!check_all(&root).is_intact(), "a checksum file problem");
    let p = check::plan(&root).unwrap();
    let control = secopy_core::job::JobControl::new();
    control.cancel();
    let r = check::run(&p, &quick(), &control, &|_| {});
    assert!(r.job.cancelled && !r.is_intact());
}

#[test]
fn progress_counts_bytes_checked() {
    let (_dir, root) = copy_of(&[("a.mov", &[1u8; 10_000]), ("b.mov", &[2u8; 5_000])]);
    let p = check::plan(&root).unwrap();
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
    let p = check::plan(&root).unwrap();
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
    let p = check::plan(&root).unwrap();
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
        root.join("a.xxh64"),
        format!("{:016x}  link/x.mov\n", secopy_core::hash::hash_bytes(b"x")),
    )
    .unwrap();
    let p = check::plan(&root).unwrap();
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
        root.join("x.xxh64"),
        format!("{:016x}  ./a.mov\n", secopy_core::hash::hash_bytes(b"a")),
    )
    .unwrap();
    let p = check::plan(&root).unwrap();
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
    let p = check::plan(&root).unwrap();
    assert_eq!(p.not_checked, [PathBuf::from("sales_report.txt")]);
}
