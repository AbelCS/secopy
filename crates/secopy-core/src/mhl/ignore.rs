//! What a history doesn't list: the standard's defaults, the system files Secopy skips, and
//! Secopy's own files (spec "What gets recorded").

use std::collections::HashSet;

use glob::{MatchOptions, Pattern};

/// The standard's defaults: what a generation without `ignore` means.
pub fn standard_defaults() -> Vec<String> {
    [".DS_Store", "ascmhl", "ascmhl/"]
        .map(String::from)
        .to_vec()
}

/// The standard's defaults, then the system files Secopy never copies, then its own files
/// (the checksum file, and the report saved next to it).
pub fn secopy_patterns() -> Vec<String> {
    let mut patterns = standard_defaults();
    patterns.extend(crate::system::NAMES.iter().map(|n| n.to_string()));
    patterns.extend(["._*", "secopy_*.xxh64", "secopy_*.txt", "secopy_*.json"].map(String::from));
    dedup(patterns)
}

/// `previous` first, then Secopy's patterns it lacks: the list only grows.
pub fn merged(previous: &[String]) -> Vec<String> {
    dedup(previous.iter().cloned().chain(secopy_patterns()).collect())
}

fn dedup(list: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    list.into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

/// A list of patterns, the .gitignore subset the standard uses (spec Appendix C): without a
/// `/` (but a trailing one) a pattern matches a name at any depth; a trailing `/` matches
/// folders only; a `/` inside anchors it at the scope's root.
pub struct Ignore {
    rules: Vec<Rule>,
}

struct Rule {
    glob: Pattern,
    anchored: bool,
    dirs_only: bool,
}

const OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

impl Ignore {
    pub fn new(patterns: &[String]) -> Ignore {
        let rules = patterns
            .iter()
            .filter_map(|p| {
                let dirs_only = p.ends_with('/');
                let body = p.trim_end_matches('/');
                let anchored = body.contains('/');
                let glob = Pattern::new(body.trim_start_matches('/')).ok()?;
                Some(Rule {
                    glob,
                    anchored,
                    dirs_only,
                })
            })
            .collect();
        Ignore { rules }
    }

    /// `rel` is `/`-separated, relative to the scope. Anything inside an ignored folder is
    /// ignored too.
    pub fn matches(&self, rel: &str, is_dir: bool) -> bool {
        let parts: Vec<&str> = rel.split('/').collect();
        (0..parts.len()).any(|end| {
            let as_dir = end + 1 < parts.len() || is_dir;
            let prefix = parts[..=end].join("/");
            self.rules.iter().any(|r| {
                (!r.dirs_only || as_dir)
                    && if r.anchored {
                        r.glob.matches_with(&prefix, OPTIONS)
                    } else {
                        r.glob.matches_with(parts[end], OPTIONS)
                    }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ignore() -> Ignore {
        Ignore::new(&secopy_patterns())
    }

    #[test]
    fn the_standards_defaults_and_secopys_own_files_are_ignored() {
        let i = ignore();
        for rel in [
            ".DS_Store",
            "A001/.DS_Store",
            "ascmhl/0001_A_2026-10-01_101500Z.mhl",
            "A001/ascmhl/ascmhl_chain.xml",
            "secopy_2026-10-01_101500.xxh64",
            "secopy_2026-10-01_101500.txt",
            "secopy_2026-10-01_101500.json",
            ".secopy-checksums.xxh64",
            "._C0001.MP4",
            ".Spotlight-V100/Store-V2/x",
            ".fseventsd/0001",
        ] {
            assert!(i.matches(rel, false), "{rel}");
        }
    }

    #[test]
    fn media_isnt_ignored() {
        let i = ignore();
        for rel in [
            "C0001.MP4",
            "PRIVATE/M4ROOT/CLIP/C0001.MP4",
            "secopy notes.txt",
            "ascmhl.txt",
        ] {
            assert!(!i.matches(rel, false), "{rel}");
        }
    }

    #[test]
    fn a_trailing_slash_is_folders_only_and_a_slash_anchors() {
        let i = Ignore::new(&["cache/".into(), "/top.txt".into(), "sub/*.tmp".into()]);
        assert!(i.matches("a/cache", true));
        assert!(i.matches("a/cache/x.mov", false));
        assert!(!i.matches("a/cache", false));
        assert!(i.matches("top.txt", false));
        assert!(!i.matches("a/top.txt", false));
        assert!(i.matches("sub/x.tmp", false));
        assert!(!i.matches("a/sub/x.tmp", false));
    }

    #[test]
    fn the_list_only_grows() {
        let merged = merged(&["*.tmp".into(), ".DS_Store".into()]);
        assert_eq!(merged[0], "*.tmp");
        assert_eq!(merged.iter().filter(|p| *p == ".DS_Store").count(), 1);
        assert!(merged.contains(&"ascmhl".to_string()));
    }
}
