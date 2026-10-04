//! Reading a history to continue it: checked against its chain first, never repaired.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::{CHAIN, ChainEntry, FOLDER};
use crate::hash::Hash;

/// A history as read, checked against its chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    pub scope: PathBuf,
    pub chain_bytes: Vec<u8>,
    pub entries: Vec<ChainEntry>,
    /// Every path any generation lists (files and folders), relative to the scope, in NFC:
    /// look one up with `records`.
    pub recorded: HashSet<String>,
    /// The first `original` XXH128 per path (NFC); paths only another format recorded aren't
    /// here. Look one up with `first_hash`.
    pub first_xxh128: HashMap<String, Hash>,
    /// The latest generation's ignore patterns (the standard's defaults if it has none).
    pub ignore: Vec<String>,
}

impl History {
    /// Whether a generation lists `rel`, however its name is spelled in Unicode: macOS file
    /// systems take the NFC and NFD forms of a name for one file (#192).
    pub fn records(&self, rel: &str) -> bool {
        self.recorded.contains(&key(rel))
    }

    /// The first known-good XXH128 of `rel`, however it's spelled.
    pub fn first_hash(&self, rel: &str) -> Option<Hash> {
        self.first_xxh128.get(&key(rel)).copied()
    }
}

/// A path in one Unicode form (NFC), to compare.
pub(crate) fn key(rel: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    rel.nfc().collect()
}

/// Why a history can't be continued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Damage {
    /// An `ascmhl` folder without a chain file (or an `ascmhl` that isn't a folder).
    NoChain,
    ChainUnreadable,
    /// `sequencenr` doesn't run 1, 2, 3…
    Gap,
    /// A manifest the chain names isn't there.
    Missing(String),
    /// A manifest isn't what the chain's C4 says.
    Altered(String),
    /// A manifest (or a name in the chain) can't be read.
    Unreadable(String),
    /// A reference to a nested history's manifest that's missing or not what its C4 says.
    BadReference(String),
}

/// The history at `scope`; `None` when it has no `ascmhl` folder.
pub fn read(scope: &Path) -> Result<Option<History>, Damage> {
    let folder = scope.join(FOLDER);
    match fs::symlink_metadata(&folder) {
        Err(_) => return Ok(None),
        Ok(m) if !m.is_dir() => return Err(Damage::NoChain),
        Ok(_) => {}
    }
    let chain_bytes = fs::read(folder.join(CHAIN)).map_err(|_| Damage::NoChain)?;
    let entries = std::str::from_utf8(&chain_bytes)
        .ok()
        .and_then(chain_entries)
        .ok_or(Damage::ChainUnreadable)?;
    if entries
        .iter()
        .enumerate()
        .any(|(i, e)| e.sequence as usize != i + 1)
    {
        return Err(Damage::Gap);
    }
    let mut history = History {
        scope: scope.to_path_buf(),
        chain_bytes: chain_bytes.clone(),
        entries: entries.clone(),
        recorded: HashSet::new(),
        first_xxh128: HashMap::new(),
        ignore: super::ignore::standard_defaults(),
    };
    for e in &entries {
        // The chain names files in the ascmhl folder only.
        if e.file.is_empty() || e.file.contains(['/', '\\']) || e.file.starts_with('.') {
            return Err(Damage::Unreadable(e.file.clone()));
        }
        let bytes = fs::read(folder.join(&e.file)).map_err(|_| Damage::Missing(e.file.clone()))?;
        if super::c4::c4(&bytes) != e.c4 {
            return Err(Damage::Altered(e.file.clone()));
        }
        let m = std::str::from_utf8(&bytes)
            .ok()
            .and_then(manifest)
            .ok_or_else(|| Damage::Unreadable(e.file.clone()))?;
        for r in m.hashes {
            let path = key(&r.path);
            // A file another tool renamed: its record moves to the new name.
            if let Some(previous) = r.previous.as_deref().map(key) {
                history.recorded.remove(&previous);
                if let Some(old) = history.first_xxh128.remove(&previous) {
                    history.first_xxh128.entry(path.clone()).or_insert(old);
                }
            }
            if let Some(h) = r.xxh128 {
                history.first_xxh128.entry(path.clone()).or_insert(h);
            }
            history.recorded.insert(path);
        }
        for (path, c4) in m.references {
            let ok = !path
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
                && fs::read(scope.join(&path)).is_ok_and(|b| super::c4::c4(&b) == c4);
            if !ok {
                return Err(Damage::BadReference(path));
            }
        }
        if let Some(ignore) = m.ignore {
            history.ignore = ignore;
        }
    }
    Ok(Some(history))
}

struct Manifest {
    hashes: Vec<HashRecord>,
    ignore: Option<Vec<String>>,
    /// Nested histories' manifests: path from this scope, and C4.
    references: Vec<(String, String)>,
}

/// A `hash` or `directoryhash` entry.
struct HashRecord {
    path: String,
    /// Its `previousPath`: the file was renamed.
    previous: Option<String>,
    /// Its XXH128 when it was `original` or `verified` (a known-good hash), not `failed`. An
    /// xxh64 (Secopy before #178), md5, sha1 or c4 is another format: the path only.
    xxh128: Option<Hash>,
}

fn local(e: &BytesStart) -> String {
    e.local_name().as_ref().to_string()
}

fn attr(e: &BytesStart, name: &str) -> Option<Option<String>> {
    match e.try_get_attribute(name).ok()? {
        None => Some(None),
        Some(a) => Some(Some(
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()?
                .into_owned(),
        )),
    }
}

/// Walks `text` as XML: `on` sees each start (with the open elements above it) and each end
/// (with the element's text). `None` on any XML error, a DOCTYPE (a history never has
/// entities), or a root other than `root`.
fn walk(text: &str, root: &str, mut on: impl FnMut(Step<'_>) -> Option<()>) -> Option<()> {
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut seen_root = false;
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) => {
                let name = local(&e);
                if stack.is_empty() {
                    if seen_root || name != root {
                        return None;
                    }
                    seen_root = true;
                }
                on(Step::Start {
                    name: &name,
                    parents: &stack,
                    element: &e,
                })?;
                stack.push(name);
                buf.clear();
            }
            Event::Empty(_) if stack.is_empty() => return None,
            Event::Text(t) => buf.push_str(&t.xml10_content()),
            Event::CData(t) => buf.push_str(&t.xml10_content()),
            Event::GeneralRef(r) => {
                let name = format!("&{};", r.xml10_content());
                buf.push_str(&quick_xml::escape::unescape(&name).ok()?);
            }
            Event::End(_) => {
                let name = stack.pop()?;
                on(Step::End {
                    name: &name,
                    parents: &stack,
                    text: &buf,
                })?;
                buf.clear();
            }
            Event::DocType(_) => return None,
            Event::Eof => break,
            _ => {}
        }
    }
    (seen_root && stack.is_empty()).then_some(())
}

enum Step<'a> {
    Start {
        name: &'a str,
        parents: &'a [String],
        element: &'a BytesStart<'a>,
    },
    End {
        name: &'a str,
        parents: &'a [String],
        text: &'a str,
    },
}

fn parent(parents: &[String]) -> Option<&str> {
    parents.last().map(String::as_str)
}

fn chain_entries(text: &str) -> Option<Vec<ChainEntry>> {
    let mut entries = Vec::new();
    let (mut sequence, mut file, mut c4) = (None, None, None);
    walk(text, "ascmhldirectory", |step| {
        match step {
            Step::Start {
                name: "hashlist",
                element,
                ..
            } => {
                sequence = Some(attr(element, "sequencenr")??.trim().parse::<u32>().ok()?);
                (file, c4) = (None, None);
            }
            Step::End {
                name: "path",
                parents,
                text,
            } if parent(parents) == Some("hashlist") => file = Some(text.to_string()),
            Step::End {
                name: "c4",
                parents,
                text,
            } if parent(parents) == Some("hashlist") => c4 = Some(text.trim().to_string()),
            Step::End {
                name: "hashlist", ..
            } => entries.push(ChainEntry {
                sequence: sequence.take()?,
                file: file.take()?,
                c4: c4.take()?,
            }),
            _ => {}
        }
        Some(())
    })?;
    Some(entries)
}

fn manifest(text: &str) -> Option<Manifest> {
    let mut hashes = Vec::new();
    let mut references = Vec::new();
    let mut ignore: Option<Vec<String>> = None;
    let (mut path, mut previous, mut xxh128, mut good) = (None, None, None, false);
    let (mut ref_path, mut ref_c4) = (None, None);
    walk(text, "hashlist", |step| {
        match step {
            Step::Start {
                name: "hash" | "directoryhash",
                ..
            } => (path, previous, xxh128) = (None, None, None),
            Step::Start {
                name: "xxh128",
                parents,
                element,
            } if parent(parents) == Some("hash") => {
                good = matches!(
                    attr(element, "action")?.as_deref(),
                    Some("original" | "verified")
                );
            }
            Step::End {
                name: "path",
                parents,
                text,
            } if parent(parents) == Some("hashlistreference") => {
                ref_path = Some(text.to_string());
            }
            Step::End {
                name: "c4",
                parents,
                text,
            } if parent(parents) == Some("hashlistreference") => {
                ref_c4 = Some(text.trim().to_string());
            }
            Step::End {
                name: "hashlistreference",
                ..
            } => references.push((ref_path.take()?, ref_c4.take()?)),
            Step::Start {
                name: "ignore",
                parents,
                ..
            } if parent(parents) == Some("processinfo") => ignore = Some(Vec::new()),
            Step::End {
                name: "path",
                parents,
                text,
            } if matches!(parent(parents), Some("hash" | "directoryhash")) => {
                path = Some(text.to_string());
            }
            Step::End {
                name: "previousPath",
                parents,
                text,
            } if matches!(parent(parents), Some("hash" | "directoryhash")) => {
                previous = Some(text.to_string());
            }
            Step::End {
                name: "xxh128",
                parents,
                text,
            } if parent(parents) == Some("hash") && good => {
                xxh128 = Some(Hash::from_hex(text.trim())?);
            }
            Step::End {
                name: "pattern",
                parents,
                text,
            } if parent(parents) == Some("ignore") => ignore.as_mut()?.push(text.to_string()),
            Step::End {
                name: "hash" | "directoryhash",
                ..
            } => hashes.push(HashRecord {
                path: path.take()?,
                previous: previous.take(),
                xxh128: xxh128.take(),
            }),
            _ => {}
        }
        Some(())
    })?;
    Some(Manifest {
        hashes,
        ignore,
        references,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mhl::write::{append, chain_xml, manifest_xml};
    use crate::mhl::{Action, Generation, Record};
    use chrono::TimeZone;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ascmhl");

    fn at() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap()
    }

    fn g(records: Vec<Record>) -> Generation {
        Generation {
            created: chrono::Local
                .with_ymd_and_hms(2026, 10, 1, 10, 15, 0)
                .unwrap(),
            hostname: "h".into(),
            tool_version: "0.19.0".into(),
            ignore: vec!["*.tmp".into()],
            records,
            references: Vec::new(),
        }
    }

    fn rec(rel: &str, xxh128: u128, action: Action) -> Record {
        Record {
            rel: rel.into(),
            size: 1,
            modified: None,
            xxh128: Hash::from_u128(xxh128),
            action,
        }
    }

    #[test]
    fn no_folder_is_no_history() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(read(d.path()), Ok(None));
    }

    #[test]
    fn the_first_original_counts_and_the_latest_ignore_list() {
        let d = tempfile::tempdir().unwrap();
        let w = append(
            d.path(),
            None,
            &[],
            &g(vec![rec("A/a.mov", 7, Action::Original)]),
            at(),
        )
        .unwrap();
        let h1 = read(d.path()).unwrap().unwrap();
        let mut second = g(vec![
            rec("A/a.mov", 9, Action::Failed),
            rec("b.wav", 3, Action::Original),
        ]);
        second.ignore.push("*.bak".into());
        append(d.path(), Some(&h1.chain_bytes), &h1.entries, &second, at()).unwrap();
        let h = read(d.path()).unwrap().unwrap();
        assert_eq!(h.first_xxh128["A/a.mov"], Hash::from_u128(7));
        assert_eq!(h.first_xxh128["b.wav"], Hash::from_u128(3));
        assert!(h.recorded.contains("A/a.mov"));
        assert!(h.ignore.contains(&"*.bak".to_string()));
        assert_eq!(h.entries.len(), 2);
        assert_eq!(h.entries[0].c4, w.c4);
    }

    #[test]
    fn another_format_is_recorded_as_original() {
        // A manifest by another tool, md5 only: the path is recorded, with no XXH128 to check.
        let d = tempfile::tempdir().unwrap();
        let folder = d.path().join("ascmhl");
        std::fs::create_dir(&folder).unwrap();
        let text = manifest_xml(&g(Vec::new())).replace(
            "</processinfo>\n",
            "</processinfo>\n  <hashes>\n    <hash>\n      <path size=\"1\">a.mov</path>\n      <md5 action=\"original\">0cc175b9c0f1b6a831c399e269772661</md5>\n    </hash>\n  </hashes>\n",
        );
        let name = "0001_x_2026-10-01_081500Z.mhl";
        std::fs::write(folder.join(name), &text).unwrap();
        std::fs::write(
            folder.join("ascmhl_chain.xml"),
            chain_xml(&[ChainEntry {
                sequence: 1,
                file: name.into(),
                c4: crate::mhl::c4::c4(text.as_bytes()),
            }]),
        )
        .unwrap();
        let h = read(d.path()).unwrap().unwrap();
        assert!(h.recorded.contains("a.mov"));
        assert!(!h.first_xxh128.contains_key("a.mov"));
    }

    #[test]
    fn the_reference_tools_output_reads() {
        let d = tempfile::tempdir().unwrap();
        let folder = d.path().join("ascmhl");
        std::fs::create_dir(&folder).unwrap();
        for f in ["0001_A002R2EC_2020-01-16_091500Z.mhl", "ascmhl_chain.xml"] {
            std::fs::copy(format!("{FIXTURES}/{f}"), folder.join(f)).unwrap();
        }
        let h = read(d.path()).unwrap().unwrap();
        assert_eq!(h.entries.len(), 1);
        assert!(h.recorded.contains("Clips/A002C006_141024_R2EC.mov"));
        // The ASC's sample records xxh64: another format since #178, so the path only.
        assert!(
            !h.first_xxh128
                .contains_key("Clips/A002C006_141024_R2EC.mov")
        );
    }

    #[test]
    fn damage_is_named() {
        let d = tempfile::tempdir().unwrap();
        let w = append(
            d.path(),
            None,
            &[],
            &g(vec![rec("a", 1, Action::Original)]),
            at(),
        )
        .unwrap();
        let name = w
            .manifest
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(&w.manifest).unwrap();
        std::fs::write(&w.manifest, text.replace("<hostname>h<", "<hostname>x<")).unwrap();
        assert_eq!(read(d.path()), Err(Damage::Altered(name.clone())));
        std::fs::remove_file(&w.manifest).unwrap();
        assert_eq!(read(d.path()), Err(Damage::Missing(name)));
        std::fs::write(&w.chain, "<nope").unwrap();
        assert_eq!(read(d.path()), Err(Damage::ChainUnreadable));
        std::fs::remove_file(&w.chain).unwrap();
        assert_eq!(read(d.path()), Err(Damage::NoChain));
    }

    #[test]
    fn a_chain_with_a_gap_or_a_path_outside_is_damaged() {
        let d = tempfile::tempdir().unwrap();
        let folder = d.path().join("ascmhl");
        std::fs::create_dir(&folder).unwrap();
        let entry = |sequence: u32, file: &str| ChainEntry {
            sequence,
            file: file.into(),
            c4: crate::mhl::c4::c4(b""),
        };
        std::fs::write(
            folder.join("ascmhl_chain.xml"),
            chain_xml(&[entry(2, "x.mhl")]),
        )
        .unwrap();
        assert_eq!(read(d.path()), Err(Damage::Gap));
        std::fs::write(
            folder.join("ascmhl_chain.xml"),
            chain_xml(&[entry(1, "../x.mhl")]),
        )
        .unwrap();
        assert_eq!(read(d.path()), Err(Damage::Unreadable("../x.mhl".into())));
    }

    /// Writes a manifest from `body` (the `<hashes>`/`<references>` part) as the next
    /// generation of `scope`'s history, as another tool would.
    fn hand_written(scope: &Path, body: &str) {
        let before = read(scope).unwrap();
        let folder = scope.join("ascmhl");
        std::fs::create_dir_all(&folder).unwrap();
        let text = manifest_xml(&g(Vec::new()))
            .replace("</processinfo>\n", &format!("</processinfo>\n{body}"));
        let mut entries = before.map_or(Vec::new(), |h| h.entries);
        let name = format!("{:04}_x_2026-10-01_081500Z.mhl", entries.len() + 1);
        std::fs::write(folder.join(&name), &text).unwrap();
        entries.push(ChainEntry {
            sequence: entries.len() as u32 + 1,
            file: name,
            c4: crate::mhl::c4::c4(text.as_bytes()),
        });
        std::fs::write(folder.join("ascmhl_chain.xml"), chain_xml(&entries)).unwrap();
    }

    /// Review #10: an XXH128 added later as `verified` (after md5) is the known-good hash.
    #[test]
    fn a_verified_xxh128_after_another_format_counts() {
        let d = tempfile::tempdir().unwrap();
        hand_written(
            d.path(),
            "  <hashes>\n    <hash>\n      <path size=\"1\">a.mov</path>\n      <md5 action=\"original\">0cc175b9c0f1b6a831c399e269772661</md5>\n    </hash>\n  </hashes>\n",
        );
        hand_written(
            d.path(),
            "  <hashes>\n    <hash>\n      <path size=\"1\">a.mov</path>\n      <md5 action=\"verified\">0cc175b9c0f1b6a831c399e269772661</md5>\n      <xxh128 action=\"verified\">000000000000000000000000000000aa</xxh128>\n    </hash>\n  </hashes>\n",
        );
        let h = read(d.path()).unwrap().unwrap();
        assert_eq!(h.first_xxh128["a.mov"], Hash::from_u128(0xaa));
    }

    /// Review #11, code review (#192): a file renamed by another tool is recorded under its
    /// new name only. ASC MHL says so with a `<previousPath>` element after the hashes.
    #[test]
    fn a_renamed_file_is_recorded_under_its_new_name() {
        let d = tempfile::tempdir().unwrap();
        hand_written(
            d.path(),
            "  <hashes>\n    <hash>\n      <path size=\"1\">old.mov</path>\n      <xxh128 action=\"original\">000000000000000000000000000000bb</xxh128>\n    </hash>\n  </hashes>\n",
        );
        hand_written(
            d.path(),
            "  <hashes>\n    <hash>\n      <path size=\"1\">new.mov</path>\n      <xxh128 action=\"verified\">000000000000000000000000000000bb</xxh128>\n      <previousPath>old.mov</previousPath>\n    </hash>\n  </hashes>\n",
        );
        let h = read(d.path()).unwrap().unwrap();
        assert!(h.recorded.contains("new.mov") && !h.recorded.contains("old.mov"));
        assert_eq!(h.first_xxh128["new.mov"], Hash::from_u128(0xbb));
    }

    /// Review #9: a reference to a nested manifest that's missing or altered is damage.
    #[test]
    fn a_bad_reference_to_a_nested_history_is_damage() {
        let d = tempfile::tempdir().unwrap();
        let child = d.path().join("A001");
        hand_written(
            &child,
            "  <hashes>\n    <hash>\n      <path size=\"1\">a.mov</path>\n      <xxh128 action=\"original\">00000000000000000000000000000001</xxh128>\n    </hash>\n  </hashes>\n",
        );
        let child_manifest = "A001/ascmhl/0001_x_2026-10-01_081500Z.mhl";
        let good = crate::mhl::c4::c4(&std::fs::read(d.path().join(child_manifest)).unwrap());
        hand_written(
            d.path(),
            &format!(
                "  <references>\n    <hashlistreference>\n      <path>{child_manifest}</path>\n      <c4>{good}</c4>\n    </hashlistreference>\n  </references>\n"
            ),
        );
        assert!(read(d.path()).is_ok());
        std::fs::write(d.path().join(child_manifest), b"altered").unwrap();
        assert_eq!(
            read(d.path()),
            Err(Damage::BadReference(child_manifest.to_string()))
        );
    }
}
