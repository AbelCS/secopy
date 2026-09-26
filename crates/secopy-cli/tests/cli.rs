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
    assert!(stdout.contains("1 files ok, 0 failed"), "{stdout}");
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

#[cfg(unix)]
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
