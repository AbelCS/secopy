//! Verify an existing copy (FR-34, plan 8): re-read every file the checksum files in a
//! directory list, and say which are intact, changed or missing, and which nothing lists.

use std::path::PathBuf;

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
