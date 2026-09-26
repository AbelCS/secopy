//! Fault injection on real (RAM-disk) volumes: disk full, unplugging, FAT and exFAT
//! rules, case-sensitive APFS (RFD §9). macOS only, where `hdiutil` needs no root.
//! Run with `SECOPY_DEVICE_TESTS=1`; CI's macOS job sets it.
#![cfg(target_os = "macos")]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use common::{pattern, read_tree, write_files};
use secopy_core::error::FatalError;
use secopy_core::filter::ExtensionFilter;
use secopy_core::fsinfo::FsKind;
use secopy_core::job::{Event, FileStatus, JobControl, JobOptions, run_job};
use secopy_core::plan::{DiffersPolicy, Plan};
use secopy_core::preflight::{ProblemKind, preflight};
use secopy_core::scan::{ScanOptions, scan};
use secopy_core::source::{DirMode, Source};

fn enabled() -> bool {
    std::env::var("SECOPY_DEVICE_TESTS").is_ok_and(|v| v == "1")
}

/// A RAM disk formatted with `format` and mounted under /Volumes; detached on drop.
struct RamDisk {
    dev: String,
    mount: PathBuf,
}

impl RamDisk {
    /// `format` as `diskutil` names it; `mib` in MiB.
    fn new(format: &str, mib: u32) -> RamDisk {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        // `diskutil` fails with "Resource busy" when two erases overlap.
        static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
        let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let out = Command::new("hdiutil")
            .args(["attach", "-nomount", &format!("ram://{}", mib * 2048)])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let dev = String::from_utf8(out.stdout).unwrap().trim().to_string();
        // FAT volume names: at most 11 characters, upper case.
        let name = format!(
            "SCT{}{}",
            std::process::id() % 10_000,
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let status = Command::new("diskutil")
            .args(["erasevolume", format, &name, &dev])
            .output()
            .unwrap();
        let disk = RamDisk {
            dev,
            mount: PathBuf::from("/Volumes").join(&name),
        };
        assert!(status.status.success(), "{status:?}");
        disk
    }

    fn unplug(&self) {
        let out = Command::new("hdiutil")
            .args(["detach", "-force", &self.dev])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    }
}

impl Drop for RamDisk {
    fn drop(&mut self) {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force", &self.dev])
            .output();
    }
}

fn plan_for(source: &Source, dest: &Path) -> Plan {
    let sel = scan(source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(source, &sel, dest).unwrap();
    Plan::resolve(&sel, &pf, DiffersPolicy::KeepBoth)
}

fn card(root: &Path, files: &[(&str, &[u8])]) -> Source {
    let src = root.join("CARD");
    write_files(&src, files);
    Source::Directory {
        path: src,
        mode: DirMode::FolderItself,
    }
}

fn no_partials(root: &Path) -> bool {
    read_tree(root)
        .keys()
        .all(|k| !k.contains("secopy-partial"))
}

#[test]
fn a_full_disk_stops_the_job_and_leaves_no_partial_files() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("ExFAT", 4);
    let dir = tempfile::tempdir().unwrap();
    let big = pattern(1 << 20);
    let files: Vec<(String, &[u8])> = (0..8)
        .map(|i| (format!("clip{i}.mov"), big.as_slice()))
        .collect();
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    let plan = plan_for(&card(dir.path(), &files), &disk.mount);
    assert!(!plan.blockers().is_empty(), "pre-flight sees it won't fit");

    // Started anyway, as if the disk filled up after pre-flight.
    let report = run_job(&plan, &JobOptions::default(), &JobControl::new(), &|_| {});
    assert_eq!(report.fatal, Some(FatalError::DiskFull), "{report:?}");
    assert!(no_partials(&disk.mount));
}

#[test]
fn unplugging_the_destination_stops_the_job() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("ExFAT", 16);
    let dir = tempfile::tempdir().unwrap();
    let data = pattern(512 << 10);
    let files: Vec<(String, &[u8])> = (0..10)
        .map(|i| (format!("clip{i}.mov"), data.as_slice()))
        .collect();
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    let plan = plan_for(&card(dir.path(), &files), &disk.mount);
    let control = JobControl::new();
    let opts = JobOptions {
        small_file_lanes: 1,
        ..JobOptions::default()
    };
    let report = std::thread::scope(|s| {
        let job = s.spawn(|| {
            run_job(&plan, &opts, &control, &|e| {
                if let Event::FileFinished(_) = e {
                    control.pause();
                }
            })
        });
        while !control.is_paused() {
            std::thread::yield_now();
        }
        disk.unplug();
        control.resume();
        job.join().unwrap()
    });
    assert_eq!(
        report.fatal,
        Some(FatalError::DestinationGone),
        "{report:?}"
    );
    assert!(report.not_started > 0);
}

#[test]
fn fat32_limits_are_found_in_preflight() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("MS-DOS FAT32", 40);
    let dir = tempfile::tempdir().unwrap();
    let source = card(dir.path(), &[("ok.mov", b"ok"), ("a:b.mov", b"colon")]);
    // A sparse file over 4 GiB: takes no space on APFS.
    fs::File::create(dir.path().join("CARD/huge.mov"))
        .unwrap()
        .set_len(4 << 30)
        .unwrap();
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let pf = preflight(&source, &sel, &disk.mount).unwrap();
    assert_eq!(pf.fs.kind, FsKind::Fat);
    assert!(!pf.fs.case_sensitive);
    let problem = |name: &str| {
        let id = sel
            .files
            .iter()
            .position(|f| f.rel.ends_with(name))
            .unwrap();
        pf.file_problems
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.kind.clone())
    };
    assert!(matches!(
        problem("a:b.mov"),
        Some(ProblemKind::InvalidName(_))
    ));
    assert!(matches!(
        problem("huge.mov"),
        Some(ProblemKind::TooLarge { .. })
    ));
    assert_eq!(problem("ok.mov"), None);
}

/// exFAT has no no-replace rename and no hard links: commits take the fallback path.
#[test]
fn exfat_copies_and_verifies_through_the_fallback_commit() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("ExFAT", 16);
    let dir = tempfile::tempdir().unwrap();
    let source = card(
        dir.path(),
        &[("A001.mov", &pattern(100_000)), ("clips/B002.mov", b"b")],
    );
    let plan = plan_for(&source, &disk.mount);
    assert_eq!(plan.fs.kind, FsKind::ExFat);
    let report = run_job(&plan, &JobOptions::default(), &JobControl::new(), &|_| {});
    assert!(report.is_success(), "{report:?}");
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| o.status == FileStatus::Verified)
    );
    let copied = read_tree(&disk.mount.join("CARD"));
    assert_eq!(copied.get("A001.mov"), Some(&pattern(100_000)));
    assert!(no_partials(&disk.mount));
}

#[test]
fn case_sensitive_apfs_copies_names_that_differ_only_in_case() {
    if !enabled() {
        return;
    }
    let disk = RamDisk::new("Case-sensitive APFS", 40);
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &[("x/a.txt", b"lower"), ("y/A.TXT", b"upper")]);
    let source = Source::Files(vec![dir.path().join("x/a.txt"), dir.path().join("y/A.TXT")]);
    let plan = plan_for(&source, &disk.mount);
    assert!(plan.fs.case_sensitive);
    let report = run_job(&plan, &JobOptions::default(), &JobControl::new(), &|_| {});
    assert!(report.is_success(), "{report:?}");
    assert_eq!(fs::read(disk.mount.join("a.txt")).unwrap(), b"lower");
    assert_eq!(fs::read(disk.mount.join("A.TXT")).unwrap(), b"upper");
}
