mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use common::{pattern, read_tree, write_files};
use secopy_core::copy::CopyConfig;
use secopy_core::error::FileError;
use secopy_core::filter::ExtensionFilter;
use secopy_core::hash::{hash_bytes, to_hex};
use secopy_core::job::{
    Event, FileStatus, Hooks, JobControl, JobOptions, JobReport, Progress, run_job,
};
use secopy_core::scan::{ScanOptions, Selection, scan};
use secopy_core::source::{DirMode, Source};

struct Fixture {
    _dir: tempfile::TempDir,
    src: PathBuf,
    dest: PathBuf,
}

/// src/CARD with a mix of small and "large" files (large = above the 64-byte test threshold).
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src/CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let big = pattern(1000);
    write_files(
        &src,
        &[
            ("A001.mov", &big),
            ("clips/B002.mov", &pattern(333)),
            ("notes.txt", b"hello"),
            ("empty.bin", b""),
        ],
    );
    Fixture {
        _dir: dir,
        src,
        dest,
    }
}

fn select(src: &Path) -> Selection {
    let source = Source::Directory {
        path: src.to_path_buf(),
        mode: DirMode::FolderItself,
    };
    scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All)
}

/// Small buffers and a low threshold so both lanes and the pipeline are exercised.
fn opts(verify: bool) -> JobOptions {
    JobOptions {
        verify,
        copy: CopyConfig {
            buffer_size: 64,
            buffers: 3,
            uncached_write: false,
        },
        small_file_threshold: 64,
        small_file_lanes: 4,
        large_file_lanes: 2,
        ..JobOptions::default()
    }
}

fn run(sel: &Selection, dest: &Path, opts: &JobOptions) -> (JobReport, Vec<Event>) {
    let events = Mutex::new(Vec::new());
    let report = run_job(sel, dest, opts, &JobControl::new(), &|e| {
        events.lock().unwrap().push(e)
    });
    (report, events.into_inner().unwrap())
}

fn expected_tree(src: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    read_tree(src.parent().unwrap())
}

#[test]
fn copy_mode_copies_everything_and_writes_the_checksum_file() {
    let f = fixture();
    let sel = select(&f.src);
    let (report, _) = run(&sel, &f.dest, &opts(false));

    assert!(report.is_success(), "{report:?}");
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| o.status == FileStatus::Copied)
    );
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
    assert_eq!(report.cache_bypass, None);

    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    let line = format!("{}  CARD/A001.mov", to_hex(hash_bytes(&pattern(1000))));
    assert!(sums.lines().any(|l| l == line), "{sums}");
    assert_eq!(sums.lines().count(), 4);
}

#[test]
fn verify_mode_marks_files_verified() {
    let f = fixture();
    let (report, _) = run(&select(&f.src), &f.dest, &opts(true));
    assert!(report.is_success(), "{report:?}");
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| o.status == FileStatus::Verified)
    );
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
    assert!(report.cache_bypass.is_some());
}

#[test]
fn empty_source_folders_are_recreated() {
    let f = fixture();
    fs::create_dir_all(f.src.join("EMPTY")).unwrap();
    run(&select(&f.src), &f.dest, &opts(false));
    assert!(f.dest.join("CARD/EMPTY").is_dir());
}

fn flip_first_byte(path: &Path) {
    let mut data = fs::read(path).unwrap();
    data[0] ^= 0xFF;
    fs::write(path, data).unwrap();
}

#[test]
fn a_corrupted_copy_is_recopied_once_and_then_verifies() {
    let f = fixture();
    let mut o = opts(true);
    o.hooks = Hooks {
        after_copy: Some(|p, attempt| {
            if attempt == 0 && p.to_string_lossy().contains("A001") {
                flip_first_byte(p);
            }
        }),
    };
    let (report, _) = run(&select(&f.src), &f.dest, &o);
    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
}

#[test]
fn a_copy_that_stays_corrupted_fails_and_leaves_no_file() {
    let f = fixture();
    let mut o = opts(true);
    o.hooks = Hooks {
        after_copy: Some(|p, _| {
            if p.to_string_lossy().contains("A001") {
                flip_first_byte(p);
            }
        }),
    };
    let (report, _) = run(&select(&f.src), &f.dest, &o);

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert!(matches!(
        failed[0].status,
        FileStatus::Failed(FileError::HashMismatch { .. })
    ));
    assert!(!f.dest.join("CARD/A001.mov").exists());
    assert!(
        read_tree(&f.dest)
            .keys()
            .all(|k| !k.contains("secopy-partial"))
    );
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    assert!(
        !sums.contains("A001"),
        "failed files are not listed (FR-28)"
    );
}

#[test]
fn an_existing_destination_file_is_never_overwritten() {
    let f = fixture();
    write_files(&f.dest, &[("CARD/notes.txt", b"mine")]);
    let (report, _) = run(&select(&f.src), &f.dest, &opts(false));

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(
        failed[0].status,
        FileStatus::Failed(FileError::AlreadyExists)
    );
    assert_eq!(fs::read(f.dest.join("CARD/notes.txt")).unwrap(), b"mine");
}

#[test]
fn cancelling_before_start_copies_nothing() {
    let f = fixture();
    let sel = select(&f.src);
    let control = JobControl::new();
    control.cancel();
    let report = run_job(&sel, &f.dest, &opts(true), &control, &|_| {});

    assert!(report.cancelled);
    assert_eq!(report.not_started, sel.files.len() as u64);
    assert!(read_tree(&f.dest).is_empty());
    assert_eq!(report.checksum_file, None);
}

#[test]
fn checksum_file_can_be_turned_off() {
    let f = fixture();
    let mut o = opts(false);
    o.write_checksum_file = false;
    let (report, _) = run(&select(&f.src), &f.dest, &o);
    assert_eq!(report.checksum_file, None);
    assert!(
        fs::read_dir(&f.dest)
            .unwrap()
            .all(|e| { e.unwrap().path().extension().is_none_or(|x| x != "xxh64") })
    );
}

#[test]
fn final_progress_event_is_complete() {
    let f = fixture();
    let sel = select(&f.src);
    let (_, events) = run(&sel, &f.dest, &opts(true));

    let last = events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Progress(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        last,
        Progress {
            total_files: 4,
            total_bytes: sel.total_bytes,
            files_done: 4,
            copied_bytes: sel.total_bytes,
            verified_bytes: sel.total_bytes,
            active: vec![],
        }
    );
    let finished = events
        .iter()
        .filter(|e| matches!(e, Event::FileFinished(_)))
        .count();
    assert_eq!(finished, 4);
}

#[test]
fn many_small_files_are_all_copied() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    for i in 0..300 {
        write_files(&src, &[(&format!("d{}/f{i}.bin", i % 7), &pattern(i))]);
    }
    let (report, _) = run(&select(&src), &dest, &opts(true));
    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&dest.join("src")), read_tree(&src));
}

#[cfg(unix)]
#[test]
fn an_unreadable_file_fails_alone() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let locked = f.src.join("notes.txt");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&locked).is_ok() {
        return; // running as root: permissions are not enforced
    }
    let (report, _) = run(&select(&f.src), &f.dest, &opts(true));

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert!(matches!(
        failed[0].status,
        FileStatus::Failed(FileError::ReadSource(_))
    ));
    assert_eq!(report.outcomes.len(), 4);
}

#[test]
fn files_that_map_to_the_same_name_never_mix() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    write_files(
        dir.path(),
        &[("x/a.txt", &pattern(500)), ("y/A.TXT", &pattern(900))],
    );
    let source = Source::Files(vec![dir.path().join("x/a.txt"), dir.path().join("y/A.TXT")]);
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let (report, _) = run(&sel, &dest, &opts(false));

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].rel, Path::new("A.TXT"));
    assert_eq!(failed[0].status, FileStatus::Failed(FileError::NameClash));
    assert_eq!(fs::read(dest.join("a.txt")).unwrap(), pattern(500));
}

#[test]
fn an_unwritable_destination_fails_every_file_without_hanging() {
    let f = fixture();
    let not_a_dir = f.dest.join("file");
    fs::write(&not_a_dir, b"x").unwrap();
    let (report, _) = run(&select(&f.src), &not_a_dir, &opts(true));

    assert_eq!(report.outcomes.len(), 4);
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| matches!(o.status, FileStatus::Failed(FileError::WriteDest(_))))
    );
    assert_eq!(report.checksum_file, None);
}

#[test]
fn unicode_names_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("Tomas");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let names = [
        "ñandú.wav",
        "日本語 クリップ.mov",
        "cafe\u{301}.txt",
        "🎬 take 1.mov",
    ];
    for n in names {
        write_files(&src, &[(n, n.as_bytes())]);
    }
    let (report, _) = run(&select(&src), &dest, &opts(true));

    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&dest.join("Tomas")), read_tree(&src));
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    for n in names {
        assert!(
            sums.contains(&format!("  Tomas/{n}\n")),
            "{n} missing in:\n{sums}"
        );
    }
}

#[test]
fn cancelling_mid_job_keeps_finished_files_and_removes_partials() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    for i in 0..50 {
        write_files(&src, &[(&format!("f{i:02}.bin"), &pattern(2000))]);
    }
    let mut o = opts(true);
    o.small_file_lanes = 1;
    o.large_file_lanes = 1;
    let control = JobControl::new();
    let report = run_job(&select(&src), &dest, &o, &control, &|e| {
        if let Event::FileFinished(_) = e {
            control.cancel();
        }
    });

    assert!(report.cancelled);
    assert!(report.not_started > 0);
    let tree = read_tree(&dest);
    assert!(
        tree.keys().all(|k| !k.contains("secopy-partial")),
        "{:?}",
        tree.keys()
    );
    let ok: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| o.status == FileStatus::Verified)
        .collect();
    assert!(!ok.is_empty());
    assert_eq!(tree.len(), ok.len(), "only verified files remain");
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    assert_eq!(sums.lines().count(), ok.len());
}
