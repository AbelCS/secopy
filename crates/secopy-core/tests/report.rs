mod common;

use std::fs;
use std::path::Path;

use chrono::Local;
use common::{pattern, write_files};
use secopy_core::filter::ExtensionFilter;
use secopy_core::job::{Hooks, JobControl, JobOptions, run_job};
use secopy_core::plan::{DiffersPolicy, Plan};
use secopy_core::preflight::preflight;
use secopy_core::report::{JobMeta, Report};
use secopy_core::scan::{ScanOptions, scan};
use secopy_core::source::{DirMode, Source};

/// CARD/ with a file that fails verification, one already at the destination, and
/// one that gets renamed (Keep both).
fn job() -> (tempfile::TempDir, Report) {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    write_files(
        &src,
        &[
            ("bad.mov", &pattern(100)),
            ("same.wav", b"same"),
            ("new.txt", b"new"),
        ],
    );
    write_files(&dest, &[("CARD/new.txt", b"older and different")]);
    // same.wav: identical copy already there.
    fs::copy(src.join("same.wav"), dest.join("CARD/same.wav")).unwrap();
    let mtime = fs::metadata(src.join("same.wav"))
        .unwrap()
        .modified()
        .unwrap();
    fs::File::options()
        .write(true)
        .open(dest.join("CARD/same.wav"))
        .unwrap()
        .set_modified(mtime)
        .unwrap();

    let source = Source::Directory {
        path: src,
        mode: DirMode::FolderItself,
    };
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &dest).unwrap();
    let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
    let opts = JobOptions {
        hooks: Hooks {
            before_copy: None,
            after_copy: Some(|p: &Path, _| {
                if p.to_string_lossy().contains("bad") {
                    let mut data = fs::read(p).unwrap();
                    data[0] ^= 0xFF;
                    fs::write(p, data).unwrap();
                }
            }),
        },
        ..JobOptions::default()
    };
    let started = Local::now();
    let job = run_job(&plan, &opts, &JobControl::new(), &|_| {});
    let meta = JobMeta {
        app_version: "0.2.0".into(),
        source: "/Volumes/CARD".into(),
        verify: true,
        started,
        finished: Local::now(),
    };
    let report = Report::new(&plan, &job, &meta);
    (dir, report)
}

#[test]
fn counts_and_statuses_are_complete() {
    let (_dir, r) = job();
    assert_eq!(r.result, "1 file failed");
    assert_eq!(r.mode, "copy+verify");
    assert_eq!(r.counts.files, 3);
    assert_eq!(r.counts.verified, 1);
    assert_eq!(r.counts.skipped_identical, 1);
    assert_eq!(r.counts.failed, 1);
    assert_eq!(r.counts.bytes_written, 3);
    assert_eq!(r.cache_bypass, Some(true));

    let file = |name: &str| r.files.iter().find(|f| f.path.ends_with(name)).unwrap();
    assert_eq!(file("bad.mov").status, "failed");
    assert!(
        file("bad.mov")
            .reason
            .as_deref()
            .unwrap()
            .contains("hash mismatch")
    );
    assert_eq!(file("same.wav").status, "skipped");
    assert!(!file("same.wav").in_checksum_file);
    assert_eq!(
        file("new.txt").copied_to.as_deref(),
        Some("CARD/new (1).txt")
    );
    assert!(file("new.txt").in_checksum_file);
    assert_eq!(file("new.txt").xxh128.as_ref().unwrap().len(), 32);
}

#[test]
fn json_is_valid_and_has_every_file() {
    let (_dir, r) = job();
    let v: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
    assert_eq!(v["counts"]["failed"], 1);
    assert_eq!(v["files"].as_array().unwrap().len(), 3);
    assert_eq!(v["files"][0]["path"], "CARD/bad.mov");
}

#[test]
fn text_lists_the_result_and_the_problems() {
    let (_dir, r) = job();
    let text = r.to_text();
    assert!(text.contains("Result:       1 file failed"), "{text}");
    assert!(text.contains("Mode:         Copy & Verify"), "{text}");
    assert!(
        text.contains("1 skipped, already at the destination (not checked)"),
        "{text}"
    );
    assert!(
        text.contains("PROBLEMS\n  CARD/bad.mov: hash mismatch"),
        "{text}"
    );
    assert!(text.contains("CARD/new.txt -> CARD/new (1).txt"), "{text}");
}

#[test]
fn reports_are_saved_next_to_the_checksum_file_and_never_overwritten() {
    let (_dir, r) = job();
    let checksum = Path::new(r.checksum_file.as_ref().unwrap()).to_path_buf();
    let (text, json) = r.write_next_to(&checksum).unwrap();
    let stem = checksum.file_stem().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        text.file_name().unwrap().to_string_lossy(),
        format!("{stem}_report.txt")
    );
    assert_eq!(
        json.file_name().unwrap().to_string_lossy(),
        format!("{stem}_report.json")
    );
    assert_eq!(fs::read_to_string(&text).unwrap(), r.to_text());
    // Another report with that name (two jobs in the same second, #116): a free name, and the
    // first one stays.
    let (again, again_json) = r.write_next_to(&checksum).unwrap();
    assert_eq!(
        again.file_name().unwrap().to_string_lossy(),
        format!("{stem}_report (2).txt")
    );
    assert_eq!(
        again_json.file_name().unwrap().to_string_lossy(),
        format!("{stem}_report (2).json")
    );
    assert_eq!(fs::read_to_string(&text).unwrap(), r.to_text());
}

/// QA review (#135): with nothing copied (every file already there) no checksum file is
/// written, and the report says so instead of nothing.
#[test]
fn a_report_says_when_no_checksum_file_was_written() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    write_files(&src, &[("a.mov", b"a")]);
    std::fs::create_dir_all(&dest).unwrap();
    let source = Source::Directory {
        path: src,
        mode: DirMode::FolderItself,
    };
    let job = |dest: &Path| {
        let sel = scan(&source, &ScanOptions::default())
            .unwrap()
            .select(&ExtensionFilter::All);
        let pf = preflight(&source, &sel, dest).unwrap();
        let plan = Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth);
        let report = run_job(&plan, &JobOptions::default(), &JobControl::new(), &|_| {});
        let meta = JobMeta {
            app_version: "0".into(),
            source: "CARD".into(),
            verify: true,
            started: Local::now(),
            finished: Local::now(),
        };
        Report::new(&plan, &report, &meta).to_text()
    };
    job(&dest); // copies it
    let again = job(&dest); // already there: nothing copied
    assert!(
        again.contains("Checksum file: none (nothing copied)"),
        "{again}"
    );
}
