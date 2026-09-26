//! File names the destination file system can't store (FR-16).

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::fsinfo::{FsInfo, NameLimit};

/// Why a name can't be created on the destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameProblem {
    InvalidChar(char),
    /// `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, with any extension.
    Reserved,
    TrailingDotOrSpace,
    TooLong {
        limit: NameLimit,
    },
}

impl fmt::Display for NameProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameProblem::InvalidChar(c) if c.is_control() => {
                write!(f, "the name contains a control character")
            }
            NameProblem::InvalidChar(c) => {
                write!(
                    f,
                    "the name contains \"{c}\", which this drive doesn't allow"
                )
            }
            NameProblem::Reserved => write!(f, "the name is reserved on Windows"),
            NameProblem::TrailingDotOrSpace => {
                write!(
                    f,
                    "the name ends with a dot or a space, which this drive doesn't allow"
                )
            }
            NameProblem::TooLong { limit } => {
                let n = match limit {
                    NameLimit::Bytes(n) | NameLimit::Utf16Units(n) => n,
                };
                write!(
                    f,
                    "the name is longer than this drive allows ({n} characters)"
                )
            }
        }
    }
}

const WINDOWS_INVALID: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Checks one file or folder name against the destination file system. Windows rules
/// apply on NTFS, ReFS, exFAT and FAT, and on every destination when running on Windows.
pub fn check_name(name: &OsStr, fs: &FsInfo) -> Result<(), NameProblem> {
    let text = name.to_string_lossy();
    if too_long(name, &text, fs.name_limit) {
        return Err(NameProblem::TooLong {
            limit: fs.name_limit,
        });
    }
    if !(cfg!(windows) || fs.kind.has_windows_names()) {
        return Ok(());
    }
    if let Some(c) = text
        .chars()
        .find(|&c| c.is_ascii_control() || WINDOWS_INVALID.contains(&c))
    {
        return Err(NameProblem::InvalidChar(c));
    }
    if text.ends_with(['.', ' ']) {
        return Err(NameProblem::TrailingDotOrSpace);
    }
    let stem = text.split('.').next().unwrap_or_default().trim_end();
    if WINDOWS_RESERVED
        .iter()
        .any(|r| r.eq_ignore_ascii_case(stem))
    {
        return Err(NameProblem::Reserved);
    }
    Ok(())
}

/// Checks every component of a relative path; returns the first problem.
pub fn check_path(rel: &Path, fs: &FsInfo) -> Result<(), NameProblem> {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .try_for_each(|name| check_name(name, fs))
}

fn too_long(name: &OsStr, text: &str, limit: NameLimit) -> bool {
    match limit {
        NameLimit::Bytes(n) => name.as_encoded_bytes().len() > n,
        NameLimit::Utf16Units(n) => text.encode_utf16().count() > n,
    }
}

/// `clips/A001.mov` → `clips/A001 (n).mov`
pub fn numbered(rel: &Path, n: u32) -> PathBuf {
    let stem = rel.file_stem().unwrap_or_default();
    let mut name = OsString::from(stem);
    name.push(format!(" ({n})"));
    if let Some(ext) = rel.extension() {
        name.push(".");
        name.push(ext);
    }
    rel.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsinfo::FsKind;

    fn fs(kind: FsKind, name_limit: NameLimit) -> FsInfo {
        FsInfo {
            kind,
            case_sensitive: false,
            free_bytes: 0,
            max_file_size: None,
            name_limit,
            device: 0,
        }
    }

    fn exfat() -> FsInfo {
        fs(FsKind::ExFat, NameLimit::Utf16Units(255))
    }

    fn ext4() -> FsInfo {
        fs(FsKind::Ext4, NameLimit::Bytes(255))
    }

    fn check(name: &str, fs: &FsInfo) -> Result<(), NameProblem> {
        check_name(OsStr::new(name), fs)
    }

    #[test]
    fn windows_rules_apply_on_exfat() {
        let cases = [
            ("A001.mov", Ok(())),
            ("a:b.mov", Err(NameProblem::InvalidChar(':'))),
            ("what?.txt", Err(NameProblem::InvalidChar('?'))),
            ("tab\there", Err(NameProblem::InvalidChar('\t'))),
            ("CON", Err(NameProblem::Reserved)),
            ("con.txt", Err(NameProblem::Reserved)),
            ("LPT9.tar.gz", Err(NameProblem::Reserved)),
            ("CONSOLE.txt", Ok(())),
            ("COM10", Ok(())),
            ("trail.", Err(NameProblem::TrailingDotOrSpace)),
            ("trail ", Err(NameProblem::TrailingDotOrSpace)),
            ("日本語 クリップ.mov", Ok(())),
        ];
        for (name, expected) in cases {
            assert_eq!(check(name, &exfat()), expected, "{name}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_file_systems_only_check_length() {
        for name in ["a:b.mov", "CON", "trail.", "what?"] {
            assert_eq!(check(name, &ext4()), Ok(()), "{name}");
        }
    }

    #[test]
    fn length_is_counted_per_file_system() {
        let limit = NameLimit::Utf16Units(255);
        assert_eq!(check(&"a".repeat(255), &exfat()), Ok(()));
        assert_eq!(
            check(&"a".repeat(256), &exfat()),
            Err(NameProblem::TooLong { limit })
        );
        // 255 characters but 765 bytes: fine in UTF-16 units, too long in bytes.
        assert_eq!(check(&"漢".repeat(255), &exfat()), Ok(()));
        assert_eq!(
            check(&"漢".repeat(255), &ext4()),
            Err(NameProblem::TooLong {
                limit: NameLimit::Bytes(255)
            })
        );
    }

    #[test]
    fn every_path_component_is_checked() {
        let rel = Path::new("CARD").join("a:b").join("A001.mov");
        assert_eq!(
            check_path(&rel, &exfat()),
            Err(NameProblem::InvalidChar(':'))
        );
        assert_eq!(check_path(Path::new("CARD/A001.mov"), &exfat()), Ok(()));
    }

    #[test]
    fn messages_are_readable() {
        assert_eq!(
            NameProblem::InvalidChar(':').to_string(),
            "the name contains \":\", which this drive doesn't allow"
        );
    }
}
