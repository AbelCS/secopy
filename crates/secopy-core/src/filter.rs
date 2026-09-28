//! Extension keys and the extension filter (FR-7..FR-11).

use std::collections::BTreeSet;
use std::path::Path;

/// Lowercase last extension without the dot, or `None` for files without one (FR-9).
pub type ExtKey = Option<String>;

/// How "no extension" is written in lists such as the CLI `--ext` flag.
const NO_EXTENSION: &str = "(none)";

pub fn ext_key(path: &Path) -> ExtKey {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| !e.is_empty())
}

/// Which extensions to copy. Use `All` when every extension is selected, so
/// empty source directories are still recreated (FR-10).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ExtensionFilter {
    #[default]
    All,
    Only(BTreeSet<ExtKey>),
}

impl ExtensionFilter {
    pub fn matches(&self, key: &ExtKey) -> bool {
        match self {
            ExtensionFilter::All => true,
            ExtensionFilter::Only(keys) => keys.contains(key),
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ExtensionFilter::Only(_))
    }

    /// Parses a comma-separated list such as `"MOV, .wav, (none)"` (FR-11).
    pub fn parse_list(list: &str) -> Self {
        let keys = list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s == NO_EXTENSION {
                    None
                } else {
                    Some(s.trim_start_matches('.').to_lowercase())
                }
            })
            .collect();
        ExtensionFilter::Only(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> ExtKey {
        Some(s.to_string())
    }

    #[test]
    fn ext_key_is_lowercase_last_extension() {
        assert_eq!(ext_key(Path::new("A001.MOV")), key("mov"));
        assert_eq!(ext_key(Path::new("backup.tar.gz")), key("gz"));
        assert_eq!(ext_key(Path::new("README")), None);
        assert_eq!(ext_key(Path::new("trailing.")), None);
    }

    #[test]
    fn all_matches_everything_and_is_inactive() {
        assert!(ExtensionFilter::All.matches(&None));
        assert!(ExtensionFilter::All.matches(&key("mov")));
        assert!(!ExtensionFilter::All.is_active());
    }

    #[test]
    fn parse_list_normalises_entries() {
        let f = ExtensionFilter::parse_list(" MOV, .wav ,(none),");
        assert!(f.is_active());
        assert!(f.matches(&key("mov")));
        assert!(f.matches(&key("wav")));
        assert!(f.matches(&None));
        assert!(!f.matches(&key("xml")));
    }
}
