mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::write_files;
use secopy_core::filter::ExtensionFilter;
use secopy_core::scan::{ExtStat, Scan, ScanOptions, scan};
use secopy_core::source::{DirMode, Source};

/// CARD/
///   A001.MOV, sound.wav, README, clips/B002.mov, clips/empty/,
///   .camera_index                          (hidden, but the camera's: copied)
///   .DS_Store, ._A001.MOV, clips/.DS_Store, .Spotlight-V100/Store/db, .fseventsd/log,
///   Thumbs.db, System Volume Information/guid, .A001.MOV.secopy-partial
///                                          (system files: skipped)
fn card() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("CARD");
    write_files(
        &card,
        &[
            ("A001.MOV", b"mov-a"),
            ("sound.wav", b"wav"),
            ("README", b"r"),
            ("clips/B002.mov", b"mov-b"),
            (".camera_index", b"i"),
            (".DS_Store", b"x"),
            ("._A001.MOV", b"x"),
            ("clips/.DS_Store", b"x"),
            (".Spotlight-V100/Store/db", b"x"),
            (".fseventsd/log", b"x"),
            ("Thumbs.db", b"x"),
            ("System Volume Information/guid", b"x"),
            (".A001.MOV.secopy-partial", b"x"),
        ],
    );
    fs::create_dir_all(card.join("clips/empty")).unwrap();
    (dir, card)
}

/// Skipped in `card()`: 5 files and 3 directories (each directory counts once).
const SYSTEM_ITEMS: u64 = 8;

fn scan_card(card: &Path, mode: DirMode, include_system_files: bool) -> Scan {
    let source = Source::Directory {
        path: card.to_path_buf(),
        mode,
    };
    scan(
        &source,
        &ScanOptions {
            include_system_files,
        },
    )
    .unwrap()
}

fn rels(paths: impl IntoIterator<Item = PathBuf>) -> BTreeSet<String> {
    paths
        .into_iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect()
}

#[test]
fn folder_itself_prefixes_paths_with_the_folder_name() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::FolderItself, false);
    assert_eq!(scan.root_dir, Some(PathBuf::from("CARD")));
    assert_eq!(
        rels(scan.files.iter().map(|f| f.rel.clone())),
        rels(
            [
                "CARD/.camera_index",
                "CARD/A001.MOV",
                "CARD/README",
                "CARD/clips/B002.mov",
                "CARD/sound.wav"
            ]
            .map(PathBuf::from)
        )
    );
}

#[test]
fn contents_only_has_no_prefix() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert_eq!(scan.root_dir, None);
    assert_eq!(
        rels(scan.files.iter().map(|f| f.rel.clone())),
        rels(
            [
                ".camera_index",
                "A001.MOV",
                "README",
                "clips/B002.mov",
                "sound.wav"
            ]
            .map(PathBuf::from)
        )
    );
}

#[test]
fn system_files_are_skipped_and_counted_once() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert_eq!(scan.skipped_system, SYSTEM_ITEMS);
    assert_eq!(scan.files.len(), 5);
}

#[test]
fn include_system_files_scans_everything() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::ContentsOnly, true);
    assert_eq!(scan.skipped_system, 0);
    assert_eq!(
        scan.files.len(),
        5 + 8,
        "the 8 system files are 5 files plus one in each directory"
    );
}

/// Cameras can set the hidden attribute on their own files on FAT/exFAT cards; macOS
/// reports it as `UF_HIDDEN`. Those are part of the card and are copied.
#[test]
fn files_a_camera_marked_hidden_are_copied() {
    use std::os::unix::ffi::OsStrExt;
    let (_dir, card) = card();
    let flagged = card.join("clips/B002.mov");
    let c = std::ffi::CString::new(flagged.as_os_str().as_bytes()).unwrap();
    // SAFETY: a valid C string path; UF_HIDDEN is 0x8000.
    assert_eq!(unsafe { libc::chflags(c.as_ptr(), 0x8000) }, 0);
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert!(
        scan.files
            .iter()
            .any(|f| f.rel == Path::new("clips/B002.mov")),
        "the hidden-flagged file is copied"
    );
}

#[test]
fn a_hidden_source_folder_is_still_scanned() {
    let dir = tempfile::tempdir().unwrap();
    let hidden_root = dir.path().join(".config");
    write_files(&hidden_root, &[("app.toml", b"x")]);
    let scan = scan_card(&hidden_root, DirMode::FolderItself, false);
    assert_eq!(
        rels(scan.files.iter().map(|f| f.rel.clone())),
        rels([PathBuf::from(".config/app.toml")])
    );
}

#[test]
fn extension_stats_group_case_insensitively() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert_eq!(
        scan.ext_stats[&Some("mov".into())],
        ExtStat {
            files: 2,
            bytes: 10
        }
    );
    assert_eq!(
        scan.ext_stats[&Some("wav".into())],
        ExtStat { files: 1, bytes: 3 }
    );
    // README and .camera_index
    assert_eq!(scan.ext_stats[&None], ExtStat { files: 2, bytes: 2 });
}

#[test]
fn empty_source_dirs_are_recorded() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::FolderItself, false);
    assert_eq!(
        rels(scan.empty_dirs.clone()),
        rels([PathBuf::from("CARD/clips/empty")])
    );
}

#[test]
fn selecting_without_filter_keeps_every_file_and_empty_dirs() {
    let (_dir, card) = card();
    let sel = scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::All);
    assert_eq!(sel.files.len(), 5);
    assert_eq!(sel.total_bytes, 15);
    assert_eq!(
        rels(sel.dirs.iter().map(|d| d.rel.clone())),
        rels(["CARD", "CARD/clips", "CARD/clips/empty"].map(PathBuf::from))
    );
}

#[test]
fn selecting_with_filter_drops_other_files_and_empty_dirs() {
    let (_dir, card) = card();
    let sel =
        scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::parse_list("mov"));
    assert_eq!(
        rels(sel.files.iter().map(|f| f.rel.clone())),
        rels(["CARD/A001.MOV", "CARD/clips/B002.mov"].map(PathBuf::from))
    );
    assert_eq!(sel.total_bytes, 10);
    assert_eq!(
        rels(sel.dirs.iter().map(|d| d.rel.clone())),
        rels(["CARD", "CARD/clips"].map(PathBuf::from))
    );
}

#[test]
fn symlinks_are_skipped_and_reported() {
    let (_dir, card) = card();
    std::os::unix::fs::symlink(card.join("A001.MOV"), card.join("link.mov")).unwrap();
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert_eq!(scan.skipped_symlinks.len(), 1);
    assert!(scan.files.iter().all(|f| f.rel != Path::new("link.mov")));
}

#[test]
fn file_sources_are_copied_flat() {
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &[("x/a.txt", b"a"), ("y/z/b.txt", b"bb")]);
    let source = Source::Files(vec![
        dir.path().join("x/a.txt"),
        dir.path().join("y/z/b.txt"),
    ]);
    let scan = scan(&source, &ScanOptions::default()).unwrap();
    assert_eq!(
        rels(scan.files.iter().map(|f| f.rel.clone())),
        rels(["a.txt", "b.txt"].map(PathBuf::from))
    );
    assert!(scan.select(&ExtensionFilter::All).dirs.is_empty());
}

#[test]
fn missing_file_source_is_reported_as_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    let source = Source::Files(vec![dir.path().join("nope.mov")]);
    let scan = scan(&source, &ScanOptions::default()).unwrap();
    assert!(scan.files.is_empty());
    assert_eq!(scan.problems.len(), 1);
}

#[test]
fn file_and_folder_mtimes_are_recorded() {
    let (_dir, card) = card();
    let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    let file = fs::File::options()
        .write(true)
        .open(card.join("clips/B002.mov"))
        .unwrap();
    file.set_times(fs::FileTimes::new().set_modified(t))
        .unwrap();
    drop(file);
    // Skip the folder check where setting its time fails.
    let dir_set = fs::File::open(card.join("clips"))
        .and_then(|d| d.set_times(fs::FileTimes::new().set_modified(t)))
        .is_ok();

    let sel = scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::All);
    let b002 = sel
        .files
        .iter()
        .find(|f| f.rel.ends_with("B002.mov"))
        .unwrap();
    assert_eq!(b002.mtime, Some(t));
    let clips = sel
        .dirs
        .iter()
        .find(|d| d.rel == Path::new("CARD").join("clips"))
        .unwrap();
    assert!(clips.mtime.is_some());
    if dir_set {
        assert_eq!(clips.mtime, Some(t));
    }
    let root = sel
        .dirs
        .iter()
        .find(|d| d.rel == Path::new("CARD"))
        .unwrap();
    assert!(
        root.mtime.is_some(),
        "the copied folder itself has an mtime"
    );
}

#[test]
fn a_symlinked_source_folder_keeps_its_own_name() {
    let (dir, card) = card();
    let link = dir.path().join("TODAY");
    std::os::unix::fs::symlink(&card, &link).unwrap();
    let scan = scan_card(&link, DirMode::FolderItself, false);
    assert_eq!(scan.root_dir, Some(PathBuf::from("TODAY")));
    assert!(scan.files.iter().all(|f| f.rel.starts_with("TODAY")));
}

#[test]
fn a_path_ending_in_dotdot_uses_the_resolved_folder_name() {
    let (_dir, card) = card();
    let scan = scan_card(&card.join("clips/.."), DirMode::FolderItself, false);
    assert_eq!(scan.root_dir, Some(PathBuf::from("CARD")));
}

#[test]
fn subset_keeps_only_the_given_files_and_their_folders() {
    let (_dir, card) = card();
    let sel = scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::All);
    let b002 = sel
        .files
        .iter()
        .position(|f| f.rel.ends_with("B002.mov"))
        .unwrap();
    let sub = sel.subset(&[b002]);
    assert_eq!(sub.files.len(), 1);
    assert_eq!(sub.total_bytes, 5);
    assert_eq!(
        rels(sub.dirs.iter().map(|d| d.rel.clone())),
        rels(["CARD", "CARD/clips"].map(PathBuf::from))
    );
}

/// QA review (#115): every folder on the way to a file is listed, so it gets its date back
/// and is made durable: DCIM, not only DCIM/100CANON.
#[test]
fn every_folder_on_the_way_to_a_file_is_listed() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("CARD");
    write_files(&card, &[("DCIM/100CANON/IMG_0001.JPG", b"x")]);
    let sel = scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::All);
    assert_eq!(
        rels(sel.dirs.iter().map(|d| d.rel.clone())),
        rels(["CARD", "CARD/DCIM", "CARD/DCIM/100CANON"].map(PathBuf::from))
    );
}
