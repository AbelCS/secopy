//! The xxhsum-compatible checksum file written to the destination (FR-29..FR-32).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local};

use crate::hash::Hash;

/// The checksum file's extension: its hash, as `xxhsum` names it.
pub const EXT: &str = "xxh128";

/// `secopy_YYYY-MM-DD_HHMMSS.xxh128`
pub fn file_name(now: DateTime<Local>) -> String {
    format!("secopy_{}.{EXT}", now.format("%Y-%m-%d_%H%M%S"))
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
fn format_line(hash: Hash, rel: &Path) -> String {
    let path = slash_path(rel);
    if path.contains(['\\', '\n', '\r']) {
        let escaped = path
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        format!("\\{}  {}", hash.to_hex(), escaped)
    } else {
        format!("{}  {}", hash.to_hex(), path)
    }
}

/// Writes the checksum file into `dest`, sorted by path, UTF-8, LF endings.
/// Never overwrites: adds `_2`, `_3`… if the name is taken. Returns the file's path.
pub fn write(
    dest: &Path,
    entries: &[(PathBuf, Hash)],
    now: DateTime<Local>,
) -> io::Result<PathBuf> {
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
/// (a Secopy partial file, which no copy or mirror picks up), synced, then renamed over it,
/// and the directory synced so the new name is on disk (`F_FULLFSYNC`). A device error
/// doing that is an error, even though the new file is in place.
pub fn write_replacing(path: &Path, entries: &[(PathBuf, Hash)]) -> io::Result<()> {
    replace_then_sync(path, entries, crate::os::full_barrier)
}

/// `write_replacing`, with how the directory is synced after the rename.
fn replace_then_sync(
    path: &Path,
    entries: &[(PathBuf, Hash)],
    sync_dir: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
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
    // A fresh file of our own: whatever is there (a leftover, or a link to elsewhere) goes
    // first, and the new one is never opened through a link.
    let _ = std::fs::remove_file(&tmp);
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| {
            f.write_all(body.as_bytes())
                .and_then(|()| crate::os::sync_durable(&f))
        });
    if let Err(e) = written.and_then(|()| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    // The rename is in the directory: until that is on disk, a power cut can bring the old
    // file back. As in a copy job, only a device error fails (not a file system that can't).
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
    match sync_dir(dir.unwrap_or(Path::new("."))) {
        Err(e) if crate::os::is_device_error(&e) => Err(e),
        _ => Ok(()),
    }
}

fn create_unique(dest: &Path, now: DateTime<Local>) -> io::Result<(PathBuf, File)> {
    let name = file_name(now);
    let stem = name.trim_end_matches(&format!(".{EXT}"));
    for n in 1u32.. {
        let candidate = if n == 1 {
            dest.join(&name)
        } else {
            dest.join(format!("{stem}_{n}.{EXT}"))
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
        let path = dir.path().join("secopy_x.xxh128");
        fs::write(&path, "half a li").unwrap();
        let result = keep_only_if_written(path.clone(), Err(io::Error::other("disk full")));
        assert_eq!(result.unwrap_err().to_string(), "disk full");
        assert!(!path.exists());
    }

    /// #69 V6: after the rename the directory is synced, or a power cut could bring the old
    /// file back; a device error doing so is an error, never a success.
    #[test]
    fn replacing_syncs_the_directory_after_the_rename() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sums.xxh128");
        fs::write(&path, "old\n").unwrap();
        let entries = [(PathBuf::from("a.mov"), Hash::from_u128(1))];
        let mut synced = None;
        replace_then_sync(&path, &entries, |d| {
            synced = Some((d.to_path_buf(), fs::read_to_string(&path).unwrap()));
            Ok(())
        })
        .unwrap();
        let new = "00000000000000000000000000000001  a.mov\n".to_string();
        assert_eq!(synced, Some((dir.path().to_path_buf(), new)));
        let err = |code| move |_: &Path| Err(io::Error::from_raw_os_error(code));
        let failed = replace_then_sync(&path, &entries, err(libc::EIO));
        assert_eq!(failed.unwrap_err().raw_os_error(), Some(libc::EIO));
        // As in a copy job: a file system that can't sync a directory is not a failure.
        assert!(replace_then_sync(&path, &entries, err(libc::ENOTSUP)).is_ok());
    }

    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 9, 26, h, m, s).unwrap()
    }

    #[test]
    fn file_name_uses_local_timestamp() {
        assert_eq!(file_name(at(14, 3, 2)), "secopy_2026-09-26_140302.xxh128");
    }

    #[test]
    fn line_uses_two_spaces_and_forward_slashes() {
        let rel = Path::new("DCIM").join("A001.mov");
        assert_eq!(
            format_line(Hash::from_u128(0xef46_db37_51d8_e999), &rel),
            "0000000000000000ef46db3751d8e999  DCIM/A001.mov"
        );
    }

    #[test]
    fn names_with_newline_or_backslash_are_escaped() {
        assert_eq!(
            format_line(Hash::from_u128(1), Path::new("a\nb")),
            "\\00000000000000000000000000000001  a\\nb"
        );
        assert_eq!(
            format_line(Hash::from_u128(1), Path::new("a\\b")),
            "\\00000000000000000000000000000001  a\\\\b"
        );
    }

    #[test]
    fn write_sorts_lines_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (Path::new("b").join("2.mov"), Hash::from_u128(2)),
            (PathBuf::from("a.mov"), Hash::from_u128(1)),
        ];
        let first = write(dir.path(), &entries, at(9, 0, 0)).unwrap();
        let second = write(dir.path(), &entries, at(9, 0, 0)).unwrap();

        assert_eq!(
            first.file_name().unwrap(),
            "secopy_2026-09-26_090000.xxh128"
        );
        assert_eq!(
            second.file_name().unwrap(),
            "secopy_2026-09-26_090000_2.xxh128"
        );
        assert_eq!(
            fs::read_to_string(&first).unwrap(),
            "00000000000000000000000000000001  a.mov\n00000000000000000000000000000002  b/2.mov\n"
        );
    }
}
