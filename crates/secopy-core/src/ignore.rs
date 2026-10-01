//! What copies and mirrors leave out (#158): the user's list of name patterns, which starts as
//! the files computers leave behind, and Secopy's own working files, always.

use std::collections::HashSet;
use std::ffi::OsStr;

/// Patterns a list holds at most.
pub const MAX_PATTERNS: usize = 200;
/// Characters a pattern has at most.
pub const MAX_LEN: usize = 255;

/// The list's defaults: macOS and Windows bookkeeping. `._*` is AppleDouble data macOS writes
/// next to files on FAT and exFAT.
pub const DEFAULTS: &[&str] = &[
    ".DS_Store",
    "._*",
    ".Spotlight-V100",
    ".fseventsd",
    ".Trashes",
    ".Trash",
    ".TemporaryItems",
    ".DocumentRevisions-V100",
    ".VolumeIcon.icns",
    ".apdisk",
    ".localized",
    "Icon\r",
    "System Volume Information",
    "$RECYCLE.BIN",
    "Thumbs.db",
    "desktop.ini",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternError {
    /// A pattern is a name: it can't contain `/`.
    HasSlash,
    TooLong,
    TooMany,
}

/// A checked list of name patterns: trimmed, none empty, no repeats in any case. `*` is any
/// run of characters, `?` one; letter case is ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patterns(Vec<String>);

impl Default for Patterns {
    fn default() -> Self {
        Patterns::defaults()
    }
}

impl Patterns {
    pub fn defaults() -> Patterns {
        Patterns(DEFAULTS.iter().map(|p| p.to_string()).collect())
    }

    pub fn none() -> Patterns {
        Patterns(Vec::new())
    }

    /// What a person typed, or a CLI flag: the first bad pattern is an error.
    pub fn new<I: IntoIterator<Item = String>>(list: I) -> Result<Patterns, PatternError> {
        let mut kept = Vec::new();
        let mut seen = HashSet::new();
        for p in list {
            let p = p.trim();
            if p.is_empty() {
                continue;
            }
            check(p)?;
            if seen.insert(p.to_lowercase()) {
                kept.push(p.to_string());
            }
        }
        if kept.len() > MAX_PATTERNS {
            return Err(PatternError::TooMany);
        }
        Ok(Patterns(kept))
    }

    /// What a file says: bad patterns are dropped, the rest kept.
    pub fn lenient<I: IntoIterator<Item = String>>(list: I) -> Patterns {
        let good = list
            .into_iter()
            .filter(|p| check(p.trim()).is_ok())
            .collect::<Vec<_>>();
        let mut p = Patterns::new(good).unwrap_or_else(|_| Patterns::none());
        p.0.truncate(MAX_PATTERNS);
        p
    }

    /// Whether a file or directory with this `name` is left out.
    pub fn matches(&self, name: &OsStr) -> bool {
        let name: Vec<char> = name.to_string_lossy().to_lowercase().chars().collect();
        self.0.iter().any(|p| {
            let p: Vec<char> = p.to_lowercase().chars().collect();
            glob(&p, &name)
        })
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

fn check(p: &str) -> Result<(), PatternError> {
    if p.contains('/') {
        Err(PatternError::HasSlash)
    } else if p.chars().count() > MAX_LEN {
        Err(PatternError::TooLong)
    } else {
        Ok(())
    }
}

/// `*` is any run of characters, `?` one; everything else is itself. Linear: on a mismatch
/// only the last `*` takes one more character, so no pattern can make a scan hang.
fn glob(p: &[char], s: &[char]) -> bool {
    let (mut pi, mut si) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, si));
            pi += 1;
        } else if let Some((sp, ss)) = star {
            pi = sp + 1;
            si = ss + 1;
            star = Some((sp, ss + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Secopy's own working files: always left out, whatever the list says.
pub fn is_own_file(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    crate::system::is_secopy_partial(&name)
        || name == ".secopy-checksums.xxh64"
        // A mirror's checksum file set aside (#114).
        || name.starts_with(".secopy-checksums.xxh64.damaged-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn star_and_question_mark_and_any_case() {
        let p = Patterns::new(["*.LRF".to_string(), "C000?.XML".to_string()]).unwrap();
        for name in ["A001.LRF", "a001.lrf", "C0001.xml"] {
            assert!(p.matches(OsStr::new(name)), "{name}");
        }
        for name in ["A001.MP4", "LRF", "C00012.XML"] {
            assert!(!p.matches(OsStr::new(name)), "{name}");
        }
    }

    /// A pattern can't make a scan hang: matching is linear, whatever the stars.
    #[test]
    fn many_stars_are_quick() {
        let p = Patterns::new([format!("{}b", "*a".repeat(60))]).unwrap();
        let started = std::time::Instant::now();
        assert!(!p.matches(OsStr::new(&"a".repeat(250))));
        assert!(started.elapsed() < std::time::Duration::from_millis(100));
    }

    #[test]
    fn only_star_and_question_mark_are_special() {
        let p = Patterns::new(["$RECYCLE.BIN".to_string(), "[x]".to_string()]).unwrap();
        assert!(p.matches(OsStr::new("$recycle.bin")));
        assert!(p.matches(OsStr::new("[x]")));
        assert!(!p.matches(OsStr::new("x")));
    }

    #[test]
    fn the_defaults_are_todays_system_files() {
        let d = Patterns::defaults();
        for name in [
            ".DS_Store",
            "._A001.MOV",
            ".Spotlight-V100",
            "Thumbs.db",
            "Icon\r",
            "DESKTOP.INI",
        ] {
            assert!(d.matches(OsStr::new(name)), "{name:?}");
        }
        for name in ["A001.MOV", "DCIM", ".hidden_clip.mov"] {
            assert!(!d.matches(OsStr::new(name)), "{name}");
        }
    }

    /// What a camera or a person names is never in the defaults (moved from system.rs).
    #[test]
    fn camera_and_user_files_are_not_defaults() {
        let d = Patterns::defaults();
        for name in [
            ".camera_index",
            ".config",
            "A001.MOV",
            "PRIVATE",
            "DS_Store",
            "_A001.MOV",
        ] {
            assert!(!d.matches(OsStr::new(name)), "{name}");
        }
    }

    #[test]
    fn patterns_are_checked() {
        assert_eq!(
            Patterns::new(["a/b".to_string()]),
            Err(PatternError::HasSlash)
        );
        assert_eq!(Patterns::new(["x".repeat(256)]), Err(PatternError::TooLong));
        assert_eq!(
            Patterns::new((0..201).map(|i| i.to_string())),
            Err(PatternError::TooMany)
        );
        let p = Patterns::new([
            " .gitkeep ".to_string(),
            "".to_string(),
            ".GITKEEP".to_string(),
        ])
        .unwrap();
        assert_eq!(p.as_slice(), [".gitkeep"]);
    }

    #[test]
    fn a_file_says_what_it_says_bad_patterns_are_dropped() {
        let p = Patterns::lenient(["a/b".to_string(), "*.LRF".to_string()]);
        assert_eq!(p.as_slice(), ["*.LRF"]);
    }

    #[test]
    fn secopys_own_files_are_its_own() {
        for name in [
            ".A001.MOV.secopy-partial",
            ".secopy-0123.partial",
            ".secopy-checksums.xxh64",
            ".secopy-checksums.xxh64.damaged-2026",
        ] {
            assert!(is_own_file(OsStr::new(name)), "{name}");
        }
        assert!(!is_own_file(OsStr::new(".DS_Store")));
    }
}
