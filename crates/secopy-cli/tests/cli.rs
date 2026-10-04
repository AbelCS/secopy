use std::fs;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_secopy-cli"))
}

#[test]
fn copies_a_folder_with_verify_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(src.join("clips")).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("clips/A001.mov"), b"movie").unwrap();

    let out = cli()
        .arg(&src)
        .arg("--to")
        .arg(&dest)
        .arg("--verify")
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read(dest.join("CARD/clips/A001.mov")).unwrap(),
        b"movie"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("1 files ok, 0 skipped, 0 failed"),
        "{stdout}"
    );
    assert!(stdout.contains("checksum file:"), "{stdout}");
}

#[test]
fn contents_flag_skips_the_folder_itself() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("a.wav"), b"a").unwrap();

    let out = cli()
        .arg(&src)
        .arg("--to")
        .arg(&dest)
        .arg("--contents")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(dest.join("a.wav").is_file());
}

#[test]
fn mixing_a_folder_and_files_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.wav"), b"a").unwrap();
    fs::create_dir_all(dir.path().join("folder")).unwrap();

    let out = cli()
        .arg(dir.path().join("a.wav"))
        .arg(dir.path().join("folder"))
        .arg("--to")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn a_failed_file_gives_exit_code_one() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let locked = dir.path().join("a.wav");
    fs::write(&locked, b"new").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&locked).is_ok() {
        return; // running as root: permissions are not enforced
    }
    let out = cli().arg(&locked).arg("--to").arg(&dest).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn a_missing_destination_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.wav"), b"a").unwrap();
    let out = cli()
        .arg(dir.path().join("a.wav"))
        .arg("--to")
        .arg(dir.path().join("nope"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

fn copy_one_file(dir: &std::path::Path, extra: &[&str]) -> std::process::Output {
    let dest = dir.join("dest");
    fs::create_dir_all(&dest).unwrap();
    cli()
        .arg(dir.join("a.wav"))
        .arg("--to")
        .arg(&dest)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn conflicts_keep_both_by_default() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.wav"), b"new").unwrap();
    fs::create_dir_all(dir.path().join("dest")).unwrap();
    fs::write(dir.path().join("dest/a.wav"), b"old one").unwrap();
    let out = copy_one_file(dir.path(), &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read(dir.path().join("dest/a.wav")).unwrap(), b"old one");
    assert_eq!(fs::read(dir.path().join("dest/a (1).wav")).unwrap(), b"new");
}

#[test]
fn on_conflict_skip_and_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.wav"), b"new").unwrap();
    fs::create_dir_all(dir.path().join("dest")).unwrap();
    fs::write(dir.path().join("dest/a.wav"), b"old one").unwrap();

    let out = copy_one_file(dir.path(), &["--on-conflict", "skip"]);
    assert!(out.status.success());
    assert_eq!(fs::read(dir.path().join("dest/a.wav")).unwrap(), b"old one");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("0 files ok, 1 skipped"), "{stdout}");

    let out = copy_one_file(dir.path(), &["--on-conflict", "overwrite", "--verify"]);
    assert!(out.status.success());
    assert_eq!(fs::read(dir.path().join("dest/a.wav")).unwrap(), b"new");
}

#[test]
fn a_second_run_skips_what_the_first_copied() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.wav"), b"new").unwrap();
    assert!(copy_one_file(dir.path(), &[]).status.success());
    let out = copy_one_file(dir.path(), &[]);
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("1 files already at the destination will be skipped"),
        "{stderr}"
    );
}

#[test]
fn report_flag_writes_text_and_json() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.wav"), b"new").unwrap();
    let reports = dir.path().join("reports");
    fs::create_dir_all(&reports).unwrap();
    let out = copy_one_file(dir.path(), &["--report", reports.to_str().unwrap()]);
    assert!(out.status.success());
    let names: Vec<String> = fs::read_dir(&reports)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.iter().any(|n| n.ends_with("_report.txt")));
    assert!(names.iter().any(|n| n.ends_with("_report.json")));
}

#[test]
fn mirror_makes_the_destination_match() {
    let dir = tempfile::tempdir().unwrap();
    let (o, d) = (dir.path().join("o"), dir.path().join("d"));
    std::fs::create_dir_all(&o).unwrap();
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(o.join("a.mov"), b"a").unwrap();
    std::fs::write(d.join("x.mov"), b"x").unwrap();
    // The guard: removing the destination's only file looks wrong, so nothing runs.
    let refused = cli()
        .args(["--mirror", "--to"])
        .arg(&d)
        .arg(&o)
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(2));
    assert!(d.join("x.mov").exists());
    std::fs::write(o.join("keep.mov"), b"k").unwrap();
    std::fs::write(d.join("keep.mov"), b"k").unwrap();
    let dry = cli()
        .args(["--mirror", "--dry-run", "--to"])
        .arg(&d)
        .arg(&o)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&dry.stdout);
    assert!(
        text.contains("+ new: 1") && text.contains("- removed: 1 (archived)"),
        "{text}"
    );
    assert!(d.join("x.mov").exists(), "a dry run changes nothing");
    let run = cli()
        .args(["--mirror", "--to"])
        .arg(&d)
        .arg(&o)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(d.join("a.mov").exists() && !d.join("x.mov").exists());
}

/// #58: a directory the scan couldn't read wasn't copied: exit 1, and say so.
#[test]
fn an_unreadable_directory_is_not_a_success() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(src.join("locked")).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("A001.mov"), b"movie").unwrap();
    fs::set_permissions(src.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let out = cli()
        .arg(&src)
        .arg("--to")
        .arg(&dest)
        .arg("--verify")
        .output()
        .unwrap();
    fs::set_permissions(src.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("1 item couldn't be read (not copied)"),
        "{stdout}"
    );
}

#[test]
fn check_says_intact_then_changed() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("A001.mov"), b"movie").unwrap();
    assert!(
        cli()
            .arg(&src)
            .arg("--to")
            .arg(&dest)
            .arg("--verify")
            .status()
            .unwrap()
            .success()
    );
    let out = cli().arg("--check").arg(&dest).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("intact: 1"));
    fs::write(dest.join("CARD/A001.mov"), b"movif").unwrap();
    let out = cli().arg("--check").arg(&dest).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("CHANGED CARD/A001.mov"));
}

/// Code review (#192): `--check --report DIR` writes the verification's report, as a copy
/// or a mirror does; copy-only options with --check are a usage error, never ignored.
#[test]
fn check_writes_its_report_and_refuses_copy_options() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest, reports) = (
        dir.path().join("CARD"),
        dir.path().join("dest"),
        dir.path().join("reports"),
    );
    for d in [&src, &dest, &reports] {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(src.join("A001.mov"), b"movie").unwrap();
    assert!(
        cli()
            .arg(&src)
            .arg("--to")
            .arg(&dest)
            .status()
            .unwrap()
            .success()
    );
    let out = cli()
        .arg("--check")
        .arg(&dest)
        .arg("--report")
        .arg(&reports)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let names: Vec<String> = fs::read_dir(&reports)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let json = names
        .iter()
        .find(|n| n.ends_with("_report.json"))
        .expect("a report");
    let text = fs::read_to_string(reports.join(json)).unwrap();
    assert!(text.contains("\"mode\": \"check\""), "{text}");
    for flag in ["--verify", "--contents", "--no-checksum", "--mhl"] {
        let out = cli().arg("--check").arg(&dest).arg(flag).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{flag} is refused with --check");
    }
    let out = cli()
        .args(["--check"])
        .arg(&dest)
        .args(["--ext", "mov"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "--ext is refused with --check");
}

/// Code review (#192): 0 days isn't "remove everything archived" (the engine reads it as no
/// limit, and the app refuses it): refused here too.
#[test]
fn archive_days_must_be_at_least_one() {
    let dir = tempfile::tempdir().unwrap();
    let (o, d) = (dir.path().join("o"), dir.path().join("d"));
    fs::create_dir_all(&o).unwrap();
    fs::create_dir_all(&d).unwrap();
    let out = cli()
        .arg(&o)
        .arg("--to")
        .arg(&d)
        .args(["--mirror", "--archive-days", "0"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

/// Code review (#192): the summary's size is what was written, not what was planned.
#[test]
fn the_summary_counts_the_bytes_written() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("CARD"), dir.path().join("dest"));
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("a.mov"), vec![1u8; 2000]).unwrap();
    fs::write(src.join("b.mov"), vec![2u8; 3000]).unwrap();
    fs::set_permissions(src.join("b.mov"), fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(src.join("b.mov")).is_ok() {
        return; // running as root: permissions are not enforced
    }
    let out = cli().arg(&src).arg("--to").arg(&dest).output().unwrap();
    fs::set_permissions(src.join("b.mov"), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("2.0 KB written in"), "{stdout}");
}

/// Final review: nothing to verify is not a success.
#[test]
fn check_with_nothing_to_verify_fails() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.mov"), b"a").unwrap();
    let out = cli().arg("--check").arg(dir.path()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("nothing to verify"));
}

/// #101: with --delete, archived files from earlier runs still go once older than
/// --archive-days, as in the app.
#[test]
fn a_mirror_with_delete_still_removes_expired_archives() {
    let dir = tempfile::tempdir().unwrap();
    let (o, d) = (dir.path().join("o"), dir.path().join("d"));
    fs::create_dir_all(&o).unwrap();
    fs::create_dir_all(&d).unwrap();
    fs::write(o.join("a.mov"), b"a").unwrap();
    let old =
        secopy_core::mirror::archive_dir(&d, chrono::Local::now() - chrono::Duration::days(40));
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("gone.mov"), b"g").unwrap();
    let run = cli()
        .args(["--mirror", "--delete", "--to"])
        .arg(&d)
        .arg(&o)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(!old.exists(), "older than the default 30 days");
}

/// QA review (#116): nothing to copy isn't a success (a script would take it for one), and
/// nothing is created.
#[test]
fn nothing_to_copy_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("CARD"), dir.path().join("dest"));
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(src.join("a.mov"), b"a").unwrap();
    let out = cli()
        .arg(&src)
        .args(["--ext", "mvo", "--to"])
        .arg(&dest)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nothing to copy"));
    assert!(!dest.join("CARD").exists());
}

/// QA review (#116): --report works for a mirror too.
#[test]
fn a_mirror_writes_the_report_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let (o, d, r) = (
        dir.path().join("o"),
        dir.path().join("d"),
        dir.path().join("r"),
    );
    for p in [&o, &d, &r] {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(o.join("a.mov"), b"a").unwrap();
    let out = cli()
        .args(["--mirror", "--report"])
        .arg(&r)
        .arg("--to")
        .arg(&d)
        .arg(&o)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let reports: Vec<_> = std::fs::read_dir(&r)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with("_report.txt"))
        .collect();
    assert_eq!(reports.len(), 1);
}

/// Review of #116: a source of empty folders is still copied (FR-6); only a file-type filter
/// that matches nothing is "nothing to copy".
#[test]
fn a_source_of_empty_folders_is_copied() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("CARD"), dir.path().join("dest"));
    std::fs::create_dir_all(src.join("DCIM/100CANON")).unwrap();
    std::fs::create_dir_all(&dest).unwrap();
    let out = cli()
        .arg(&src)
        .args(["--contents", "--to"])
        .arg(&dest)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dest.join("DCIM/100CANON").is_dir());
}

/// Review of #116: a mirror whose checksum file can't be written doesn't report complete.
#[test]
fn a_mirror_report_says_when_its_checksum_file_failed() {
    let dir = tempfile::tempdir().unwrap();
    let (o, d, r) = (
        dir.path().join("o"),
        dir.path().join("d"),
        dir.path().join("r"),
    );
    for p in [&o, &d, &r] {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(o.join("a.mov"), b"a").unwrap();
    std::fs::create_dir_all(d.join(".secopy-checksums.xxh128")).unwrap(); // in the way
    let out = cli()
        .args(["--mirror", "--report"])
        .arg(&r)
        .arg("--to")
        .arg(&d)
        .arg(&o)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report = std::fs::read_dir(&r)
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().ends_with("_report.txt"))
        .unwrap();
    let text = std::fs::read_to_string(report.path()).unwrap();
    assert!(!text.contains("Result:       complete"), "{text}");
}

/// QA review (#136): expired archived files the clean-up couldn't remove are said, and the
/// run isn't a success.
#[test]
fn a_mirror_says_when_expired_archive_files_stay() {
    let dir = tempfile::tempdir().unwrap();
    let (o, d) = (dir.path().join("o"), dir.path().join("d"));
    fs::create_dir_all(&o).unwrap();
    fs::create_dir_all(&d).unwrap();
    fs::write(o.join("a.mov"), b"a").unwrap();
    let old =
        secopy_core::mirror::archive_dir(&d, chrono::Local::now() - chrono::Duration::days(40));
    fs::create_dir_all(&old).unwrap();
    let stuck = old.join("gone.mov");
    fs::write(&stuck, b"g").unwrap();
    let c = std::ffi::CString::new(stuck.to_str().unwrap()).unwrap();
    // SAFETY: a NUL-terminated path; UF_IMMUTABLE makes its removal fail.
    unsafe { libc::chflags(c.as_ptr(), libc::UF_IMMUTABLE as _) };
    let run = cli()
        .args(["--mirror", "--to"])
        .arg(&d)
        .arg(&o)
        .output()
        .unwrap();
    // SAFETY: as above; cleared so the test directory can be removed.
    unsafe { libc::chflags(c.as_ptr(), 0) };
    assert_eq!(run.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("NOT removed"),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// QA review (#137): a mirror's dry run counts files that will fail apart, not as unchanged.
#[test]
fn a_mirror_dry_run_counts_files_that_will_fail() {
    let dir = tempfile::tempdir().unwrap();
    let (o, d) = (dir.path().join("o"), dir.path().join("d"));
    fs::create_dir_all(&o).unwrap();
    fs::create_dir_all(d.join("x.mov")).unwrap(); // a directory where the file goes
    fs::write(o.join("x.mov"), b"x").unwrap();
    let out = cli()
        .args(["--mirror", "--dry-run", "--to"])
        .arg(&d)
        .arg(&o)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("! will fail: 1"), "{text}");
    assert!(text.contains("= unchanged: 0"), "{text}");
}

/// #154: --mhl writes an ASC MHL history in the folder the files go to.
#[test]
fn mhl_writes_a_history() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("A001.mov"), b"movie").unwrap();
    let out = cli()
        .arg(&src)
        .arg("--to")
        .arg(&dest)
        .arg("--mhl")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dest.join("CARD/ascmhl/ascmhl_chain.xml").is_file());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ASC MHL: new history"), "{stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ASC MHL:"), "{stdout}");
}

/// #154: an ASC MHL blocker stops the copy before anything is copied.
#[test]
fn mhl_blockers_stop_before_copying() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(dest.join("CARD/ascmhl")).unwrap();
    fs::write(src.join("A001.mov"), b"movie").unwrap();
    let out = cli()
        .arg(&src)
        .arg("--to")
        .arg(&dest)
        .arg("--mhl")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("damaged"), "{stderr}");
    assert!(!dest.join("CARD/A001.mov").exists());
}

/// #154: ASC MHL is for copies, not mirrors.
#[test]
fn mhl_isnt_for_mirrors() {
    let dir = tempfile::tempdir().unwrap();
    let out = cli()
        .arg(dir.path())
        .arg("--to")
        .arg(dir.path())
        .arg("--mirror")
        .arg("--mhl")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

/// #158: --ignore adds name patterns to the defaults.
#[test]
fn ignore_adds_patterns() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("a.MP4"), b"a").unwrap();
    fs::write(src.join("a.LRF"), b"l").unwrap();
    fs::write(src.join(".DS_Store"), b"d").unwrap();
    let out = cli()
        .arg(&src)
        .args(["--ignore", "*.lrf", "--to"])
        .arg(&dest)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dest.join("CARD/a.MP4").exists());
    assert!(!dest.join("CARD/a.LRF").exists() && !dest.join("CARD/.DS_Store").exists());
}

/// #158: a pattern with / is an error, before anything is copied.
#[test]
fn a_bad_pattern_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let out = cli()
        .arg(dir.path())
        .args(["--ignore", "a/b", "--to"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("a/b"));
}

/// #158: --include-system-files drops the defaults; --ignore still applies.
#[test]
fn include_system_files_drops_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join(".DS_Store"), b"d").unwrap();
    fs::write(src.join("a.LRF"), b"l").unwrap();
    let out = cli()
        .arg(&src)
        .args(["--include-system-files", "--ignore", "*.LRF", "--to"])
        .arg(&dest)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dest.join("CARD/.DS_Store").exists() && !dest.join("CARD/a.LRF").exists());
}
