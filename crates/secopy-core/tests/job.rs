mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use common::{pattern, read_tree, write_files};
use secopy_core::copy::CopyConfig;
use secopy_core::error::{FatalError, FileError};
use secopy_core::filter::ExtensionFilter;
use secopy_core::hash::{hash_bytes, to_hex};
use secopy_core::job::{
    Event, FileStatus, Hooks, JobControl, JobOptions, JobReport, Progress, SkipReason, run_job,
};
use secopy_core::plan::{Action, DiffersPolicy, Plan};
use secopy_core::preflight::preflight;
use secopy_core::scan::{ScanOptions, scan};
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

/// Scan, pre-flight and resolve with `policy` for files that differ.
fn plan_with(source: &Source, dest: &Path, policy: DiffersPolicy) -> Plan {
    let sel = scan(source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(source, &sel, dest).unwrap();
    Plan::resolve(&sel, &pf, policy)
}

fn plan_of(source: &Source, dest: &Path) -> Plan {
    plan_with(source, dest, DiffersPolicy::KeepBoth)
}

/// Copies the folder `src` itself into `dest`.
fn plan(src: &Path, dest: &Path) -> Plan {
    let source = Source::Directory {
        path: src.to_path_buf(),
        mode: DirMode::FolderItself,
    };
    plan_of(&source, dest)
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

fn run(plan: &Plan, opts: &JobOptions) -> (JobReport, Vec<Event>) {
    let events = Mutex::new(Vec::new());
    let report = run_job(plan, opts, &JobControl::new(), &|e| {
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
    let (report, _) = run(&plan(&f.src, &f.dest), &opts(false));

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
    let (report, _) = run(&plan(&f.src, &f.dest), &opts(true));
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
    run(&plan(&f.src, &f.dest), &opts(false));
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
        before_copy: None,
        after_copy: Some(|p, attempt| {
            if attempt == 0 && p.to_string_lossy().contains("A001") {
                flip_first_byte(p);
            }
        }),
    };
    let (report, _) = run(&plan(&f.src, &f.dest), &o);
    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
}

#[test]
fn a_copy_that_stays_corrupted_fails_and_leaves_no_file() {
    let f = fixture();
    let mut o = opts(true);
    o.hooks = Hooks {
        before_copy: None,
        after_copy: Some(|p, _| {
            if p.to_string_lossy().contains("A001") {
                flip_first_byte(p);
            }
        }),
    };
    let (report, _) = run(&plan(&f.src, &f.dest), &o);

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
fn a_file_that_appears_after_preflight_is_never_overwritten() {
    let f = fixture();
    let plan = plan(&f.src, &f.dest);
    write_files(&f.dest, &[("CARD/notes.txt", b"mine")]);
    let (report, _) = run(&plan, &opts(false));

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
    let plan = plan(&f.src, &f.dest);
    let control = JobControl::new();
    control.cancel();
    let report = run_job(&plan, &opts(true), &control, &|_| {});

    assert!(report.cancelled);
    assert_eq!(report.not_started, plan.files.len() as u64);
    assert_eq!(
        fs::read_dir(&f.dest).unwrap().count(),
        0,
        "not even empty folders are left (FR-10)"
    );
    assert_eq!(report.checksum_file, None);
}

#[test]
fn checksum_file_can_be_turned_off() {
    let f = fixture();
    let mut o = opts(false);
    o.write_checksum_file = false;
    let (report, _) = run(&plan(&f.src, &f.dest), &o);
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
    let plan = plan(&f.src, &f.dest);
    let total = plan.total_bytes();
    let (_, events) = run(&plan, &opts(true));

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
            total_bytes: total,
            files_done: 4,
            files_skipped: 0,
            copied_bytes: total,
            verified_bytes: total,
            active: vec![],
            paused: false,
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
    let (report, _) = run(&plan(&src, &dest), &opts(true));
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
    let (report, _) = run(&plan(&f.src, &f.dest), &opts(true));

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
    let plan = plan_of(&source, &dest);
    let (report, _) = run(&plan, &opts(false));

    assert_eq!(fs::read(dest.join("a.txt")).unwrap(), pattern(500));
    if plan.fs.case_sensitive {
        // Linux: two different files (FR-17a applies to case-insensitive drives only).
        assert!(report.is_success(), "{report:?}");
        assert_eq!(fs::read(dest.join("A.TXT")).unwrap(), pattern(900));
    } else {
        let failed: Vec<_> = report.failed().collect();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].rel, Path::new("A.TXT"));
        assert_eq!(failed[0].status, FileStatus::Failed(FileError::NameClash));
    }
}

#[test]
fn a_destination_that_turns_into_a_file_stops_the_job() {
    let f = fixture();
    let plan = plan(&f.src, &f.dest);
    // The destination turns into a file after pre-flight.
    fs::remove_dir(&f.dest).unwrap();
    fs::write(&f.dest, b"x").unwrap();
    let (report, _) = run(&plan, &opts(true));

    assert_eq!(report.fatal, Some(FatalError::DestinationGone));
    assert!(!report.cancelled, "a fatal error is not a cancel");
    assert!(
        report
            .failed()
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
    let (report, _) = run(&plan(&src, &dest), &opts(true));

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
    let report = run_job(&plan(&src, &dest), &o, &control, &|e| {
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

/// Two jobs copying the same names into one destination (two card readers, one
/// "Day01" folder) must never report a file whose bytes on disk are the other job's.
#[test]
fn concurrent_jobs_into_one_destination_never_report_foreign_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let (card_a, card_b) = (dir.path().join("a/DCIM"), dir.path().join("b/DCIM"));
    for i in 0..200 {
        let name = format!("IMG_{i:04}.JPG");
        write_files(&card_a, &[(&name, &[b'A'; 3000])]);
        write_files(&card_b, &[(&name, &[b'B'; 1000])]);
    }
    let contents = |p: &Path, dest: &Path| {
        let source = Source::Directory {
            path: p.to_path_buf(),
            mode: DirMode::ContentsOnly,
        };
        plan_of(&source, dest)
    };
    for verify in [false, true] {
        let dest = dest.join(if verify { "verify" } else { "copy" });
        fs::create_dir_all(&dest).unwrap();
        // Both plans see an empty destination, so both try every name.
        let (plan_a, plan_b) = (contents(&card_a, &dest), contents(&card_b, &dest));
        let o = opts(verify);
        let (ra, rb) = std::thread::scope(|s| {
            let a = s.spawn(|| run_job(&plan_a, &o, &JobControl::new(), &|_| {}));
            let b = s.spawn(|| run_job(&plan_b, &o, &JobControl::new(), &|_| {}));
            (a.join().unwrap(), b.join().unwrap())
        });
        let mut claimed = std::collections::HashSet::new();
        for o in ra.outcomes.iter().chain(&rb.outcomes) {
            if matches!(o.status, FileStatus::Copied | FileStatus::Verified) {
                let on_disk = fs::read(dest.join(&o.rel)).unwrap();
                assert_eq!(
                    Some(hash_bytes(&on_disk)),
                    o.hash,
                    "{} reported ok but holds other bytes (verify={verify})",
                    o.rel.display()
                );
                assert!(
                    claimed.insert(o.rel.clone()),
                    "{} claimed twice",
                    o.rel.display()
                );
            }
        }
        assert_eq!(claimed.len(), 200, "each name is copied exactly once");
    }
}

/// macOS treats `café` in NFC and NFD as the same name, although the bytes differ.
#[cfg(target_os = "macos")]
#[test]
fn names_equal_after_unicode_normalization_never_mix() {
    let dir = tempfile::tempdir().unwrap();
    let (nfc, nfd) = ("caf\u{e9}.bin", "cafe\u{301}.bin");
    write_files(
        dir.path(),
        &[
            (&format!("x/{nfc}"), &[b'A'; 3000]),
            (&format!("y/{nfd}"), &[b'B'; 1000]),
        ],
    );
    let source = Source::Files(vec![
        dir.path().join("x").join(nfc),
        dir.path().join("y").join(nfd),
    ]);
    for verify in [false, true] {
        for round in 0..5 {
            let dest = dir.path().join(format!("dest-{verify}-{round}"));
            fs::create_dir_all(&dest).unwrap();
            let (report, _) = run(&plan_of(&source, &dest), &opts(verify));
            let ok: Vec<_> = report
                .outcomes
                .iter()
                .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
                .collect();
            assert_eq!(ok.len(), 1, "{report:?}");
            let on_disk = fs::read(dest.join(&ok[0].rel)).unwrap();
            assert_eq!(Some(hash_bytes(&on_disk)), ok[0].hash, "verify={verify}");
        }
    }
}

#[test]
fn a_panicking_event_handler_ends_the_job_instead_of_hanging() {
    let f = fixture();
    let plan = plan(&f.src, &f.dest);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_job(&plan, &opts(false), &JobControl::new(), &|e| {
                if let Event::FileFinished(_) = e {
                    panic!("event handler failed");
                }
            })
        }));
        let _ = tx.send(result.is_err());
    });
    let panicked = rx
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("run_job hung after a worker panicked");
    assert!(panicked);
}

#[test]
fn copying_never_runs_far_ahead_of_verification() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    for i in 0..3000 {
        write_files(&src, &[(&format!("d{}/f{i}.bin", i % 30), &pattern(100))]);
    }
    let o = JobOptions {
        verify: true,
        small_file_lanes: 8,
        large_file_lanes: 1,
        verify_lanes: 1,
        progress_interval: std::time::Duration::from_millis(1),
        ..JobOptions::default()
    };
    let max_active = std::sync::atomic::AtomicUsize::new(0);
    let report = run_job(&plan(&src, &dest), &o, &JobControl::new(), &|e| {
        if let Event::Progress(p) = e {
            max_active.fetch_max(p.active.len(), std::sync::atomic::Ordering::Relaxed);
        }
    });
    assert!(report.is_success(), "{report:?}");
    // 9 copy lanes + a small verify queue + 1 verify lane; never thousands.
    let max = max_active.into_inner();
    assert!(max <= 20, "{max} files in flight");
}

#[test]
fn a_paused_job_does_no_io_until_resumed() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    for i in 0..40 {
        write_files(&src, &[(&format!("f{i:02}.bin"), &pattern(2000))]);
    }
    let plan = plan(&src, &dest);
    let control = JobControl::new();
    let finished = std::sync::atomic::AtomicUsize::new(0);
    let saw_paused = std::sync::atomic::AtomicBool::new(false);
    let report = std::thread::scope(|s| {
        let job = s.spawn(|| {
            run_job(&plan, &opts(true), &control, &|e| match e {
                Event::FileFinished(_) => {
                    if finished.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                        control.pause();
                    }
                }
                Event::Progress(p) if p.paused => {
                    saw_paused.store(true, std::sync::atomic::Ordering::SeqCst)
                }
                _ => {}
            })
        });
        // Wait until the pause has taken hold, then check that nothing moves.
        while !saw_paused.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        let before = (
            finished.load(std::sync::atomic::Ordering::SeqCst),
            read_tree(&dest),
        );
        std::thread::sleep(std::time::Duration::from_millis(300));
        let after = (
            finished.load(std::sync::atomic::Ordering::SeqCst),
            read_tree(&dest),
        );
        assert_eq!(before, after, "files changed while paused");
        assert!(before.0 < 40, "the job finished before the pause");
        control.resume();
        job.join().unwrap()
    });
    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&dest.join("src")), read_tree(&src));
}

#[test]
fn partial_files_left_by_an_interrupted_job_are_replaced() {
    let f = fixture();
    let leftover = f.dest.join("CARD/.notes.txt.secopy-partial");
    write_files(&f.dest, &[("CARD/.notes.txt.secopy-partial", b"half")]);
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
    fs::File::options()
        .write(true)
        .open(&leftover)
        .unwrap()
        .set_modified(old)
        .unwrap();

    let (report, _) = run(&plan(&f.src, &f.dest), &opts(true));
    assert!(report.is_success(), "{report:?}");
    assert_eq!(report.removed_partials, 1);
    assert!(!leftover.exists());
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
}

/// Gives `rel` under `dest` the same contents and modification time as under `src`.
fn copy_like(src: &Path, dest: &Path, rel: &str) {
    let (from, to) = (src.join(rel), dest.join(rel));
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::copy(&from, &to).unwrap();
    let mtime = fs::metadata(&from).unwrap().modified().unwrap();
    fs::File::options()
        .write(true)
        .open(&to)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
}

#[test]
fn identical_files_are_skipped_and_left_out_of_the_checksum_file() {
    let f = fixture();
    copy_like(f.src.parent().unwrap(), &f.dest, "CARD/A001.mov");
    let plan = plan(&f.src, &f.dest);
    let (report, events) = run(&plan, &opts(true));

    assert!(report.is_success(), "{report:?}");
    let skipped: Vec<_> = report.skipped().collect();
    assert_eq!(skipped.len(), 1);
    assert_eq!(
        skipped[0].status,
        FileStatus::Skipped(SkipReason::Identical)
    );
    assert_eq!(skipped[0].hash, None, "a skipped file is not read");
    assert!(!skipped[0].in_checksum_file);
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    assert!(!sums.contains("A001"), "{sums}");
    assert_eq!(sums.lines().count(), 3);
    let last = events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Progress(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(last.files_skipped, 1);
    assert_eq!(last.total_bytes, plan.bytes_to_write());
}

#[test]
fn keep_both_copies_under_a_new_name_and_lists_it() {
    let f = fixture();
    write_files(&f.dest, &[("CARD/notes.txt", b"mine, and different")]);
    let (report, _) = run(&plan(&f.src, &f.dest), &opts(true));

    assert!(report.is_success(), "{report:?}");
    assert_eq!(
        fs::read(f.dest.join("CARD/notes.txt")).unwrap(),
        b"mine, and different"
    );
    assert_eq!(
        fs::read(f.dest.join("CARD/notes (1).txt")).unwrap(),
        b"hello"
    );
    let notes = report
        .outcomes
        .iter()
        .find(|o| o.rel.ends_with("notes.txt"))
        .unwrap();
    assert_eq!(notes.final_rel, Path::new("CARD").join("notes (1).txt"));
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    assert!(sums.contains("  CARD/notes (1).txt\n"), "{sums}");
}

#[test]
fn overwrite_replaces_a_different_file_after_verifying() {
    let f = fixture();
    write_files(&f.dest, &[("CARD/notes.txt", b"old")]);
    let source = Source::Directory {
        path: f.src.clone(),
        mode: DirMode::FolderItself,
    };
    let plan = plan_with(&source, &f.dest, DiffersPolicy::Overwrite);
    let (report, _) = run(&plan, &opts(true));

    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
}

#[test]
fn a_copy_that_fails_verification_never_replaces_the_old_file() {
    let f = fixture();
    write_files(&f.dest, &[("CARD/A001.mov", b"old")]);
    let source = Source::Directory {
        path: f.src.clone(),
        mode: DirMode::FolderItself,
    };
    let plan = plan_with(&source, &f.dest, DiffersPolicy::Overwrite);
    let mut o = opts(true);
    o.hooks = Hooks {
        before_copy: None,
        after_copy: Some(|p, _| {
            if p.to_string_lossy().contains("A001") {
                flip_first_byte(p);
            }
        }),
    };
    let (report, _) = run(&plan, &o);

    assert_eq!(report.failed().count(), 1);
    assert_eq!(fs::read(f.dest.join("CARD/A001.mov")).unwrap(), b"old");
}

#[test]
fn skip_leaves_a_different_file_alone() {
    let f = fixture();
    write_files(&f.dest, &[("CARD/notes.txt", b"old")]);
    let source = Source::Directory {
        path: f.src.clone(),
        mode: DirMode::FolderItself,
    };
    let plan = plan_with(&source, &f.dest, DiffersPolicy::Skip);
    let (report, _) = run(&plan, &opts(false));

    assert!(report.is_success(), "{report:?}");
    assert_eq!(fs::read(f.dest.join("CARD/notes.txt")).unwrap(), b"old");
    let skipped: Vec<_> = report.skipped().collect();
    assert_eq!(skipped[0].status, FileStatus::Skipped(SkipReason::Differs));
}

#[test]
fn failed_files_can_be_retried() {
    let f = fixture();
    let source = Source::Directory {
        path: f.src.clone(),
        mode: DirMode::FolderItself,
    };
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &f.dest).unwrap();
    let first = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    let mut o = opts(true);
    o.hooks = Hooks {
        before_copy: None,
        after_copy: Some(|p, _| {
            if p.to_string_lossy().contains("A001") {
                flip_first_byte(p);
            }
        }),
    };
    let (report, _) = run(&first, &o);
    let failed: Vec<usize> = report.failed().map(|o| o.id).collect();
    assert_eq!(failed.len(), 1);

    // "Retry failed": the same selection, only the failed files, checked again.
    let retry = sel.subset(&failed);
    let pf = preflight(&source, &retry, &f.dest).unwrap();
    let second = Plan::resolve(&retry, &pf, DiffersPolicy::KeepBoth);
    assert_eq!(second.files[0].action, Action::Copy);
    let (report, _) = run(&second, &opts(true));
    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
}

#[test]
fn small_files_never_take_the_pipelined_path_by_default() {
    let o = JobOptions::default();
    assert_eq!(o.small_file_threshold, o.copy.buffer_size as u64);
}

#[test]
fn file_and_folder_metadata_is_kept() {
    let f = fixture();
    let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    let file = f.src.join("clips/B002.mov");
    #[allow(unused_mut)]
    let mut times = fs::FileTimes::new().set_modified(t).set_accessed(t);
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::FileTimesExt;
        times = times.set_created(t);
    }
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_times(times)
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
    }
    let dir_t = t + std::time::Duration::from_secs(3600);
    let clips_set = fs::File::open(f.src.join("clips"))
        .and_then(|d| d.set_modified(dir_t))
        .is_ok();

    for verify in [false, true] {
        let dest = f.dest.join(if verify { "verify" } else { "copy" });
        fs::create_dir_all(&dest).unwrap();
        let (report, _) = run(&plan(&f.src, &dest), &opts(verify));
        assert!(report.is_success(), "{report:?}");
        let copy = fs::metadata(dest.join("CARD/clips/B002.mov")).unwrap();
        assert_eq!(copy.modified().unwrap(), t, "verify={verify}");
        #[cfg(target_os = "macos")]
        assert_eq!(copy.created().unwrap(), t);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(copy.permissions().mode() & 0o777, 0o640);
        }
        if clips_set {
            let clips = fs::metadata(dest.join("CARD/clips")).unwrap();
            assert_eq!(
                clips.modified().unwrap(),
                dir_t,
                "folder mtime (verify={verify})"
            );
        }
    }
}

/// Many files, one lane each, so a fault in the middle leaves files not started.
#[cfg(unix)]
fn long_fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src/CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    for i in 0..30 {
        write_files(&src, &[(&format!("f{i:02}.bin"), &pattern(2000))]);
    }
    Fixture {
        _dir: dir,
        src,
        dest,
    }
}

#[cfg(unix)]
fn one_lane(verify: bool) -> JobOptions {
    JobOptions {
        small_file_lanes: 1,
        large_file_lanes: 1,
        verify_lanes: 1,
        ..opts(verify)
    }
}

/// Unplugging the destination: its folder disappears mid-job.
#[cfg(unix)]
#[test]
fn a_destination_that_disappears_stops_the_job() {
    let f = long_fixture();
    let mut o = one_lane(false);
    o.hooks = Hooks {
        before_copy: None,
        after_copy: Some(|partial, _| {
            if partial.to_string_lossy().contains("f05") {
                // <dest>/CARD/.f05.bin.secopy-partial → move <dest> away
                let dest = partial.parent().unwrap().parent().unwrap();
                fs::rename(dest, dest.with_extension("gone")).unwrap();
            }
        }),
    };
    let (report, _) = run(&plan(&f.src, &f.dest), &o);

    assert_eq!(report.fatal, Some(FatalError::DestinationGone));
    assert!(report.not_started > 0, "{report:?}");
    assert!(!report.is_success());
}

/// Unplugging the card: the source folder disappears mid-job.
#[cfg(unix)]
#[test]
fn a_source_that_disappears_stops_the_job() {
    let f = long_fixture();
    let mut o = one_lane(true);
    o.hooks = Hooks {
        before_copy: Some(|source| {
            if source.to_string_lossy().contains("f05") {
                let card = source.parent().unwrap();
                fs::rename(card, card.with_extension("gone")).unwrap();
            }
        }),
        after_copy: None,
    };
    let (report, _) = run(&plan(&f.src, &f.dest), &o);

    assert_eq!(report.fatal, Some(FatalError::SourceGone));
    assert!(report.not_started > 0, "{report:?}");
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    assert_eq!(
        sums.lines().count(),
        5,
        "files before the fault are kept and listed"
    );
}

#[test]
fn a_source_file_that_changes_while_copied_fails_alone() {
    let f = fixture();
    let mut o = opts(true);
    o.hooks = Hooks {
        before_copy: Some(|source| {
            if source.to_string_lossy().contains("A001") {
                fs::write(source, b"shorter now").unwrap();
            }
        }),
        after_copy: None,
    };
    let (report, _) = run(&plan(&f.src, &f.dest), &o);

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(
        failed[0].status,
        FileStatus::Failed(FileError::SourceChanged)
    );
    assert_eq!(report.fatal, None);
    assert!(!f.dest.join("CARD/A001.mov").exists());
    assert_eq!(report.outcomes.len(), 4);
}

/// The CLI passes paths as typed, so a relative source must be watched too (FR-21).
#[cfg(unix)]
#[test]
fn a_relative_source_that_disappears_stops_the_job() {
    let f = long_fixture();
    let cwd = std::env::current_dir().unwrap();
    let relative = pathdiff(&f.src, &cwd);
    let source = Source::Directory {
        path: relative,
        mode: DirMode::FolderItself,
    };
    let mut o = one_lane(false);
    o.hooks = Hooks {
        before_copy: Some(|source| {
            if source.to_string_lossy().contains("f05") {
                let card = source.parent().unwrap();
                fs::rename(card, card.with_extension("gone")).unwrap();
            }
        }),
        after_copy: None,
    };
    let (report, _) = run(&plan_of(&source, &f.dest), &o);

    assert_eq!(report.fatal, Some(FatalError::SourceGone), "{report:?}");
    assert!(report.not_started > 0);
}

/// `to` relative to `from`, both absolute (`../..` steps up as needed).
#[cfg(unix)]
fn pathdiff(to: &Path, from: &Path) -> PathBuf {
    let (to, from) = (
        fs::canonicalize(to).unwrap(),
        fs::canonicalize(from).unwrap(),
    );
    let common = to
        .components()
        .zip(from.components())
        .take_while(|(a, b)| a == b)
        .count();
    let mut rel = PathBuf::new();
    for _ in from.components().skip(common) {
        rel.push("..");
    }
    for c in to.components().skip(common) {
        rel.push(c);
    }
    rel
}

/// Pre-flight lists every leftover partial file; ones next to files that are skipped
/// (already copied before the crash) are removed at the end too.
#[test]
fn leftover_partial_files_of_skipped_files_are_removed() {
    let f = fixture();
    copy_like(f.src.parent().unwrap(), &f.dest, "CARD/A001.mov");
    let leftover = f.dest.join("CARD/.A001.mov.secopy-partial");
    write_files(&f.dest, &[("CARD/.A001.mov.secopy-partial", b"half")]);
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
    fs::File::options()
        .write(true)
        .open(&leftover)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let plan = plan(&f.src, &f.dest);
    assert_eq!(plan.stale_partials, vec![leftover.clone()]);

    let (report, _) = run(&plan, &opts(false));
    assert!(report.is_success(), "{report:?}");
    assert_eq!(report.skipped().count(), 1);
    assert!(!leftover.exists());
    assert_eq!(report.removed_partials, 1);
}

/// A pulled card can't be read any more, but what was already copied only needs the
/// destination: those files are still verified and kept (FR-21, FR-25).
#[cfg(unix)]
#[test]
fn files_copied_before_the_source_disappears_are_still_verified() {
    let f = long_fixture();
    // A big file just before the fault: its verification is still running when the
    // next file finds the card gone.
    write_files(&f.src, &[("f04.bin", &pattern(4 << 20))]);
    let mut o = one_lane(true);
    o.hooks = Hooks {
        before_copy: Some(|source| {
            if source.to_string_lossy().contains("f05") {
                let card = source.parent().unwrap();
                fs::rename(card, card.with_extension("gone")).unwrap();
            }
        }),
        after_copy: None,
    };
    let (report, _) = run(&plan(&f.src, &f.dest), &o);

    assert_eq!(report.fatal, Some(FatalError::SourceGone));
    let verified: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| o.status == FileStatus::Verified)
        .map(|o| o.rel.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        verified,
        ["f00.bin", "f01.bin", "f02.bin", "f03.bin", "f04.bin"],
        "{report:?}"
    );
    assert!(!report.cancelled);
}
