//! What copies and mirrors leave out (#158): the user's list of name patterns, which starts as
//! the files computers leave behind, and Secopy's own working files, always.

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
    /// A control character (one an ASC MHL history couldn't hold).
    BadChar,
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
        let mut kept = Patterns::none();
        for p in list {
            if let Some(p) = trimmed(&p) {
                check(p)?;
                kept.push(p);
            }
        }
        if kept.0.len() > MAX_PATTERNS {
            return Err(PatternError::TooMany);
        }
        Ok(kept)
    }

    /// Adds `p` unless the list has it already (in any case or Unicode form).
    fn push(&mut self, p: &str) {
        if !self.0.iter().any(|q| key(q) == key(p)) {
            self.0.push(p.to_string());
        }
    }

    /// What a file says: bad patterns are dropped, the rest kept.
    /// What a file says: bad patterns are dropped, and past the most a list holds, the rest:
    /// the first ones always stay.
    pub fn lenient<I: IntoIterator<Item = String>>(list: I) -> Patterns {
        let mut kept = Patterns::none();
        for p in list {
            if kept.0.len() == MAX_PATTERNS {
                break;
            }
            // The old `Icon\r` default (lists saved before #161): Secopy leaves it out anyway.
            if let Some(p) = trimmed(&p).filter(|p| check(p).is_ok() && !is_folder_icon(p)) {
                kept.push(p);
            }
        }
        kept
    }

    /// Whether a file or directory with this `name` is left out.
    pub fn matches(&self, name: &OsStr) -> bool {
        let name: Vec<char> = key(&name.to_string_lossy()).chars().collect();
        self.0.iter().any(|p| {
            let p: Vec<char> = key(p).chars().collect();
            glob(&p, &name)
        })
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

/// What names are compared by: one Unicode form (macOS may store either) and one case.
fn key(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect::<String>().to_lowercase()
}

/// A pattern without the spaces at its ends (only spaces: `Icon\r` is a name with a carriage
/// return in it); `None` when nothing is left.
fn trimmed(p: &str) -> Option<&str> {
    Some(p.trim_matches(' ')).filter(|p| !p.is_empty())
}

fn check(p: &str) -> Result<(), PatternError> {
    if p.contains('/') {
        Err(PatternError::HasSlash)
    } else if !crate::mhl::write::xml_can_hold(p) || p.chars().any(|c| c < ' ' && c != '\r') {
        Err(PatternError::BadChar)
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
        // `*` first: a name can have a `*` in it, which isn't the pattern's wildcard.
        if pi < p.len() && p[pi] == '*' {
            star = Some((pi, si));
            pi += 1;
        } else if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
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

/// macOS's custom-folder-icon file, `Icon\r`, in any case (as the old default matched it).
fn is_folder_icon(name: &str) -> bool {
    name.eq_ignore_ascii_case("Icon\r")
}

/// Secopy's own working files: always left out, whatever the list says.
pub fn is_own_file(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    crate::system::is_secopy_partial(&name)
        || name == ".secopy-checksums.xxh64"
        // macOS's custom-folder-icon file: never media (#161).
        || is_folder_icon(&name)
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

    /// Review #2: a list longer than allowed keeps its first patterns, never none.
    #[test]
    fn a_long_list_from_a_file_keeps_its_first_patterns() {
        let list = std::iter::once("*.LRF".to_string()).chain((0..300).map(|i| format!("x{i}")));
        let p = Patterns::lenient(list);
        assert_eq!(p.as_slice().len(), MAX_PATTERNS);
        assert!(p.matches(OsStr::new("a.LRF")));
    }

    /// Review #3: a name with `*` in it still matches a `*` pattern.
    #[test]
    fn a_star_in_a_name_is_a_character() {
        let p = Patterns::new(["*.LRF".to_string()]).unwrap();
        assert!(p.matches(OsStr::new("*take.LRF")));
        assert!(p.matches(OsStr::new("a*b.lrf")));
    }

    /// Review #4: the same name in either Unicode form matches.
    #[test]
    fn either_unicode_form_matches() {
        let composed = Patterns::new(["Caf\u{e9}*".to_string()]).unwrap();
        assert!(composed.matches(OsStr::new("Cafe\u{301}.LRF")));
        let decomposed = Patterns::new(["Cafe\u{301}*".to_string()]).unwrap();
        assert!(decomposed.matches(OsStr::new("Caf\u{e9}.LRF")));
        let both = Patterns::new(["Caf\u{e9}".to_string(), "Cafe\u{301}".to_string()]).unwrap();
        assert_eq!(both.as_slice().len(), 1, "the same name twice");
    }

    /// Review #7: a pattern a history's XML can't hold is refused (Icon\r stays).
    #[test]
    fn control_characters_are_refused() {
        assert_eq!(
            Patterns::new(["a\u{1}b".to_string()]),
            Err(PatternError::BadChar)
        );
        assert!(Patterns::new(["Icon\r".to_string()]).is_ok());
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

    /// The defaults survive being checked again: `Icon\r` keeps its `\r` (only spaces trim).
    #[test]
    fn the_defaults_check_as_they_are() {
        let again = Patterns::new(Patterns::defaults().as_slice().to_vec()).unwrap();
        assert_eq!(again, Patterns::defaults());
        assert_eq!(
            Patterns::lenient(DEFAULTS.iter().map(|p| p.to_string())),
            Patterns::defaults()
        );
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

    /// #161: macOS's custom-folder-icon file is never media: always left out, not in the list
    /// (a list read from a file drops it), so the list only holds names a person can type.
    #[test]
    fn the_folder_icon_file_is_always_left_out() {
        assert!(is_own_file(OsStr::new("Icon\r")));
        assert!(
            !Patterns::defaults()
                .as_slice()
                .iter()
                .any(|p| p.contains('\r'))
        );
        let read = Patterns::lenient(["Icon\r".to_string(), "*.LRF".to_string()]);
        assert_eq!(read.as_slice(), ["*.LRF"]);
        assert!(!is_own_file(OsStr::new("Icon")) && !is_own_file(OsStr::new("Icons")));
    }

    /// #161 review: only the old `Icon\r` default leaves a saved list; a user's own patterns
    /// stay, even ones that look like Secopy's files.
    #[test]
    fn a_saved_list_keeps_every_user_pattern() {
        let read = Patterns::lenient([".secopy-*.partial".to_string(), "ICON\r".to_string()]);
        assert_eq!(read.as_slice(), [".secopy-*.partial"]);
        assert!(read.matches(OsStr::new(".secopy-user.PARTIAL")));
    }

    /// #161 review: the folder icon file is left out in any case, as the old default did.
    #[test]
    fn the_folder_icon_file_is_left_out_in_any_case() {
        assert!(is_own_file(OsStr::new("icon\r")) && is_own_file(OsStr::new("ICON\r")));
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
