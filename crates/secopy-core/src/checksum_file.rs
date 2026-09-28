//! The xxhsum-compatible checksum file written to the destination (FR-29..FR-32).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local};

use crate::hash::to_hex;

/// `secopy_YYYY-MM-DD_HHMMSS.xxh64`
pub fn file_name(now: DateTime<Local>) -> String {
    format!("secopy_{}.xxh64", now.format("%Y-%m-%d_%H%M%S"))
}

/// Relative path with `/` separators on every OS.
pub fn slash_path(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// One line without the trailing newline: `<hash>  <path>`. Paths containing `\`, CR or
/// LF use the GNU coreutils escaping: a leading `\` and escaped characters.
fn format_line(hash: u64, rel: &Path) -> String {
    let path = slash_path(rel);
    if path.contains(['\\', '\n', '\r']) {
        let escaped = path
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        format!("\\{}  {}", to_hex(hash), escaped)
    } else {
        format!("{}  {}", to_hex(hash), path)
    }
}

/// Writes the checksum file into `dest`, sorted by path, UTF-8, LF endings.
/// Never overwrites: adds `_2`, `_3`… if the name is taken. Returns the file's path.
pub fn write(dest: &Path, entries: &[(PathBuf, u64)], now: DateTime<Local>) -> io::Result<PathBuf> {
    let mut lines: Vec<(String, String)> = entries
        .iter()
        .map(|(rel, hash)| (slash_path(rel), format_line(*hash, rel)))
        .collect();
    lines.sort();
    let mut body = String::new();
    for (_, line) in &lines {
        body.push_str(line);
        body.push('\n');
    }
    let (path, mut file) = create_unique(dest, now)?;
    let written = file
        .write_all(body.as_bytes())
        .and_then(|()| crate::os::sync_durable(&file));
    keep_only_if_written(path, written)
}

/// A checksum file cut short would look finished and list too few files: it goes.
fn keep_only_if_written(path: PathBuf, written: io::Result<()>) -> io::Result<PathBuf> {
    match written {
        Ok(()) => Ok(path),
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            Err(e)
        }
    }
}

/// Writes `entries` to `path` whole or not at all: a temporary `<name>.partial` beside it
/// (a Secopy partial file, which no copy or mirror picks up), synced, then renamed over it.
pub fn write_replacing(path: &Path, entries: &[(PathBuf, u64)]) -> io::Result<()> {
    let mut lines: Vec<(String, String)> = entries
        .iter()
        .map(|(rel, hash)| (slash_path(rel), format_line(*hash, rel)))
        .collect();
    lines.sort();
    let mut body = String::new();
    for (_, line) in &lines {
        body.push_str(line);
        body.push('\n');
    }
    let tmp = path.with_extension("partial");
    let written = File::create(&tmp).and_then(|mut f| {
        f.write_all(body.as_bytes())
            .and_then(|()| crate::os::sync_durable(&f))
    });
    if let Err(e) = written.and_then(|()| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn create_unique(dest: &Path, now: DateTime<Local>) -> io::Result<(PathBuf, File)> {
    let name = file_name(now);
    let stem = name.trim_end_matches(".xxh64");
    for n in 1u32.. {
        let candidate = if n == 1 {
            dest.join(&name)
        } else {
            dest.join(format!("{stem}_{n}.xxh64"))
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("ran out of checksum file names")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs;

    /// #58: a checksum file that couldn't be written in full doesn't stay, looking finished.
    #[test]
    fn a_checksum_file_that_fails_while_written_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secopy_x.xxh64");
        fs::write(&path, "half a li").unwrap();
        let result = keep_only_if_written(path.clone(), Err(io::Error::other("disk full")));
        assert_eq!(result.unwrap_err().to_string(), "disk full");
        assert!(!path.exists());
    }

    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 9, 26, h, m, s).unwrap()
    }

    #[test]
    fn file_name_uses_local_timestamp() {
        assert_eq!(file_name(at(14, 3, 2)), "secopy_2026-09-26_140302.xxh64");
    }

    #[test]
    fn line_uses_two_spaces_and_forward_slashes() {
        let rel = Path::new("DCIM").join("A001.mov");
        assert_eq!(
            format_line(0xef46_db37_51d8_e999, &rel),
            "ef46db3751d8e999  DCIM/A001.mov"
        );
    }

    #[test]
    fn names_with_newline_or_backslash_are_escaped() {
        assert_eq!(
            format_line(1, Path::new("a\nb")),
            "\\0000000000000001  a\\nb"
        );
        assert_eq!(
            format_line(1, Path::new("a\\b")),
            "\\0000000000000001  a\\\\b"
        );
    }

    #[test]
    fn write_sorts_lines_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (Path::new("b").join("2.mov"), 2),
            (PathBuf::from("a.mov"), 1),
        ];
        let first = write(dir.path(), &entries, at(9, 0, 0)).unwrap();
        let second = write(dir.path(), &entries, at(9, 0, 0)).unwrap();

        assert_eq!(first.file_name().unwrap(), "secopy_2026-09-26_090000.xxh64");
        assert_eq!(
            second.file_name().unwrap(),
            "secopy_2026-09-26_090000_2.xxh64"
        );
        assert_eq!(
            fs::read_to_string(&first).unwrap(),
            "0000000000000001  a.mov\n0000000000000002  b/2.mov\n"
        );
    }
}
