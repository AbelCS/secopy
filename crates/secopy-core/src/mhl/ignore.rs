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

/// The standard's defaults, then the names the user's list leaves out of the copy (#158), then
/// Secopy's own files (its working files, the checksum file and the report next to it).
pub fn secopy_patterns(user: &crate::ignore::Patterns) -> Vec<String> {
    let mut patterns = standard_defaults();
    patterns.extend(user.as_slice().iter().map(|p| as_gitignore(p)));
    patterns.extend(
        [
            ".secopy-checksums.xxh64",
            "*.secopy-partial",
            ".secopy-*.partial",
            "secopy_*.xxh64",
            "secopy_*.txt",
            "secopy_*.json",
        ]
        .map(String::from),
    );
    dedup(patterns)
}

/// A name pattern (only `*` and `?` special, any case) as a gitignore one with the same
/// meaning: each letter as `[lL]` (gitignore is case-sensitive), `[` as `[[]`, a run of `*`
/// as one, and a leading `!` or `#` escaped (not a negation or a comment).
fn as_gitignore(p: &str) -> String {
    let mut out = String::new();
    let mut last = None;
    for c in p.chars() {
        if c == '*' && last == Some('*') {
            continue;
        }
        let (lower, upper) = (c.to_lowercase().to_string(), c.to_uppercase().to_string());
        if c == '[' {
            out.push_str("[[]");
        } else if lower != upper && lower.chars().count() == 1 && upper.chars().count() == 1 {
            out.push_str(&format!("[{lower}{upper}]"));
        } else {
            out.push(c);
        }
        last = Some(c);
    }
    if out.starts_with('!') || out.starts_with('#') {
        format!("\\{out}")
    } else {
        out
    }
}

/// `previous` first, then `additions` it lacks: the list only grows.
pub fn merged(previous: &[String], additions: &[String]) -> Vec<String> {
    dedup(
        previous
            .iter()
            .cloned()
            .chain(additions.iter().cloned())
            .collect(),
    )
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
    /// `!pattern`: takes a match of an earlier pattern back.
    negated: bool,
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
                // `\!` and `\#` are a literal `!` or `#` at the start, not a negation.
                let (negated, p) = match (p.strip_prefix('!'), p.strip_prefix('\\')) {
                    (Some(rest), _) => (true, rest),
                    (None, Some(rest)) if rest.starts_with(['!', '#']) => (false, rest),
                    _ => (false, p.as_str()),
                };
                let dirs_only = p.ends_with('/');
                let body = p.trim_end_matches('/');
                let anchored = body.contains('/');
                let glob = Pattern::new(body.trim_start_matches('/')).ok()?;
                Some(Rule {
                    glob,
                    anchored,
                    dirs_only,
                    negated,
                })
            })
            .collect();
        Ignore { rules }
    }

    /// `rel` is `/`-separated, relative to the scope. As in .gitignore, the last pattern that
    /// matches decides, and anything inside an ignored folder is ignored too.
    pub fn matches(&self, rel: &str, is_dir: bool) -> bool {
        let parts: Vec<&str> = rel.split('/').collect();
        for end in 0..parts.len() {
            let last = end + 1 == parts.len();
            let as_dir = !last || is_dir;
            let prefix = parts[..=end].join("/");
            let ignored = self
                .rules
                .iter()
                .rev()
                .find(|r| {
                    (!r.dirs_only || as_dir)
                        && if r.anchored {
                            r.glob.matches_with(&prefix, OPTIONS)
                        } else {
                            r.glob.matches_with(parts[end], OPTIONS)
                        }
                })
                .is_some_and(|r| !r.negated);
            if ignored || last {
                return ignored;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ignore() -> Ignore {
        Ignore::new(&secopy_patterns(&crate::ignore::Patterns::defaults()))
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

    /// Review #12: a later `!pattern` takes a file back out of an earlier pattern.
    #[test]
    fn a_negated_pattern_isnt_ignored() {
        let i = Ignore::new(&["*.mov".into(), "!keep.mov".into()]);
        assert!(i.matches("other.mov", false));
        assert!(!i.matches("keep.mov", false));
        assert!(!i.matches("sub/keep.mov", false));
    }

    fn user(list: &[&str]) -> Ignore {
        let patterns = crate::ignore::Patterns::new(list.iter().map(|p| p.to_string())).unwrap();
        Ignore::new(&secopy_patterns(&patterns))
    }

    /// Review #5: the history ignores a user pattern in any case, as the copy does.
    #[test]
    fn user_patterns_match_in_any_case() {
        let i = user(&["*.LRF"]);
        for rel in ["a.LRF", "extra.lrf", "sub/x.Lrf"] {
            assert!(i.matches(rel, false), "{rel}");
        }
        assert!(!i.matches("a.mov", false));
    }

    /// Review #6: a leading ! or #, `**` and `[` mean what they meant in the list.
    #[test]
    fn user_patterns_keep_their_meaning() {
        let i = user(&["!notes", "#tag", "**.LRF", "[x]"]);
        for rel in ["!notes", "#tag", "a.lrf", "[x]"] {
            assert!(i.matches(rel, false), "{rel}");
        }
        assert!(!i.matches("x", false));
    }

    #[test]
    fn the_list_only_grows() {
        let merged = merged(
            &["*.tmp".into(), ".DS_Store".into()],
            &secopy_patterns(&crate::ignore::Patterns::defaults()),
        );
        assert_eq!(merged[0], "*.tmp");
        assert_eq!(merged.iter().filter(|p| *p == ".DS_Store").count(), 1);
        assert!(merged.contains(&"ascmhl".to_string()));
    }
}
