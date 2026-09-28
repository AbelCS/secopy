//! Verify an existing copy (FR-34, plan 8): re-read every file the checksum files in a
//! directory list, and say which are intact, changed or missing, and which nothing lists.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

use crate::mirror::ARCHIVE_DIR;
use crate::system::is_system_file;

/// Lines of a checksum file that couldn't be used: 1-based line number, and why.
pub type BadLines = Vec<(usize, String)>;

/// Parses xxhsum/GNU lines, `<16 hex>  <path>`, with the coreutils escaping a leading `\`
/// announces (`\\`, `\n`, `\r`). Returns the entries and the bad lines (1-based, why).
pub fn parse(text: &str) -> (Vec<(PathBuf, u64)>, BadLines) {
    let (lines, bad) = parse_lines(text);
    (
        lines
            .into_iter()
            .map(|(_, path, hash)| (path, hash))
            .collect(),
        bad,
    )
}

/// `parse`, with each entry's 1-based line number.
fn parse_lines(text: &str) -> (Vec<(usize, PathBuf, u64)>, BadLines) {
    let mut entries = Vec::new();
    let mut bad = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let (escaped, line) = match line.strip_prefix('\\') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        let Some((hex, path)) = line.split_once("  ") else {
            bad.push((i + 1, "not a \"<checksum>  <path>\" line".into()));
            continue;
        };
        let hash = (hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| u64::from_str_radix(hex, 16).ok())
            .flatten();
        let Some(hash) = hash else {
            bad.push((i + 1, format!("\"{hex}\" isn't an xxHash64 checksum")));
            continue;
        };
        let path = if escaped {
            unescape(path)
        } else {
            Some(path.to_string())
        };
        match path {
            Some(path) if !path.is_empty() => entries.push((i + 1, PathBuf::from(path), hash)),
            _ => bad.push((i + 1, "the path can't be read".into())),
        }
    }
    (entries, bad)
}

fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            _ => return None,
        }
    }
    Some(out)
}

/// A mirror's checksum file in its destination (plan 8).
pub const MIRROR_CHECKSUMS: &str = ".secopy-checksums.xxh64";

/// One file a checksum file lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Relative to the checked directory.
    pub rel: PathBuf,
    pub expected: u64,
    /// Its size when planned; 0 when it was missing then.
    pub size: u64,
    /// The checksum file it came from, relative to the checked directory.
    pub from: PathBuf,
}

/// Something that keeps part of the directory from being checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The checksum file (or directory) concerned, relative to the checked directory.
    pub file: PathBuf,
    /// 1-based line in `file`; `None` when the whole file couldn't be read.
    pub line: Option<usize>,
    pub reason: String,
}

/// What a check reads, worked out before anything is read.
#[derive(Debug, Clone, Default)]
pub struct CheckPlan {
    pub dir: PathBuf,
    pub checksum_files: Vec<PathBuf>,
    pub files: Vec<Listed>,
    /// Files no checksum file lists, relative to the checked directory.
    pub not_checked: Vec<PathBuf>,
    pub problems: Vec<Problem>,
    pub total_bytes: u64,
}

pub fn plan(dir: &Path) -> io::Result<CheckPlan> {
    fs::read_dir(dir)?; // there, and readable
    let mut sums: Vec<(PathBuf, Option<SystemTime>)> = Vec::new();
    let mut others = Vec::new();
    let mut problems = Vec::new();
    let walk = WalkDir::new(dir)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && e.file_name() == ARCHIVE_DIR));
    for entry in walk {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let file = e
                    .path()
                    .and_then(|p| p.strip_prefix(dir).ok())
                    .map(Path::to_path_buf);
                problems.push(Problem {
                    file: file.unwrap_or_default(),
                    line: None,
                    reason: format!("couldn't be read: {e}"),
                });
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .to_path_buf();
        let name = entry.file_name().to_string_lossy();
        if name.ends_with(".xxh64") {
            sums.push((rel, entry.metadata().ok().and_then(|m| m.modified().ok())));
        } else if !is_system_file(entry.file_name())
            && !name.ends_with("_report.txt")
            && !name.ends_with("_report.json")
        {
            others.push(rel);
        }
    }
    // Oldest first, so the newest checksum file's entry is the one kept.
    sums.sort_by_key(|(_, modified)| *modified);
    let mut listed: HashMap<PathBuf, Listed> = HashMap::new();
    for (sum, _) in &sums {
        let text = match fs::read_to_string(dir.join(sum)) {
            Ok(text) => text,
            Err(e) => {
                problems.push(Problem {
                    file: sum.clone(),
                    line: None,
                    reason: format!("couldn't be read: {e}"),
                });
                continue;
            }
        };
        let (entries, bad) = parse_lines(&text);
        for (line, reason) in bad {
            problems.push(Problem {
                file: sum.clone(),
                line: Some(line),
                reason,
            });
        }
        let base = sum.parent().unwrap_or(Path::new(""));
        for (line, path, expected) in entries {
            if !inside(&path) {
                problems.push(Problem {
                    file: sum.clone(),
                    line: Some(line),
                    reason: format!("{} points outside the checked directory", path.display()),
                });
                continue;
            }
            let rel = base.join(&path);
            let size = fs::metadata(dir.join(&rel)).map_or(0, |m| m.len());
            listed.insert(
                rel.clone(),
                Listed {
                    rel,
                    expected,
                    size,
                    from: sum.clone(),
                },
            );
        }
    }
    let keys: HashSet<String> = listed.keys().map(|p| key(p)).collect();
    let mut not_checked: Vec<PathBuf> = others
        .into_iter()
        .filter(|p| !keys.contains(&key(p)))
        .collect();
    not_checked.sort();
    let mut files: Vec<Listed> = listed.into_values().collect();
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(CheckPlan {
        dir: dir.to_path_buf(),
        checksum_files: sums.into_iter().map(|(p, _)| p).collect(),
        total_bytes: files.iter().map(|f| f.size).sum(),
        files,
        not_checked,
        problems,
    })
}

/// Only plain names: no `..`, no root, nothing that leaves the checked directory.
fn inside(path: &Path) -> bool {
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// A name compared in one Unicode form: what a checksum file wrote and what the disk shows
/// can differ in form on some file systems.
fn key(p: &Path) -> String {
    p.to_string_lossy().nfc().collect()
}
