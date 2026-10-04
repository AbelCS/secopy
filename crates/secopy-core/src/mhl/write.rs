//! Writing a generation: the manifest first (a new file), then the chain (replaced whole), so
//! a failure never leaves a history pointing at nothing (spec "Writing safely").

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, SecondsFormat, Utc};

use super::{Action, CHAIN, ChainEntry, FOLDER, Generation};

/// Characters XML 1.0 allows: a name with others can't be listed.
pub fn xml_can_hold(s: &str) -> bool {
    s.chars().all(|c| {
        matches!(c, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}'
            | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\r', "&#13;")
}

fn date(t: DateTime<Local>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// The manifest's text: records sorted by path.
pub fn manifest_xml(g: &Generation) -> String {
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<hashlist version=\"2.0\" xmlns=\"urn:ASC:MHL:v2.0\">\n  <creatorinfo>\n");
    let _ = writeln!(x, "    <creationdate>{}</creationdate>", date(g.created));
    let _ = writeln!(x, "    <hostname>{}</hostname>", esc(&g.hostname));
    let _ = writeln!(
        x,
        "    <tool version=\"{}\">Secopy</tool>",
        esc(&g.tool_version)
    );
    x.push_str("  </creatorinfo>\n  <processinfo>\n    <process>transfer</process>\n");
    if !g.ignore.is_empty() {
        x.push_str("    <ignore>\n");
        for p in &g.ignore {
            let _ = writeln!(x, "      <pattern>{}</pattern>", esc(p));
        }
        x.push_str("    </ignore>\n");
    }
    x.push_str("  </processinfo>\n");
    let mut records: Vec<_> = g.records.iter().collect();
    records.sort_by(|a, b| a.rel.cmp(&b.rel));
    if !records.is_empty() {
        x.push_str("  <hashes>\n");
        for r in records {
            let modified = r
                .modified
                .map(|m| {
                    format!(
                        " lastmodificationdate=\"{}\"",
                        date(DateTime::<Local>::from(m))
                    )
                })
                .unwrap_or_default();
            let action = match r.action {
                Action::Original => "original",
                Action::Verified => "verified",
                Action::Failed => "failed",
            };
            let _ = write!(
                x,
                "    <hash>\n      <path size=\"{}\"{modified}>{}</path>\n      <xxh128 action=\"{action}\">{}</xxh128>\n    </hash>\n",
                r.size,
                esc(&r.rel),
                r.xxh128.to_hex()
            );
        }
        x.push_str("  </hashes>\n");
    }
    if !g.references.is_empty() {
        x.push_str("  <references>\n");
        for r in &g.references {
            let _ = write!(
                x,
                "    <hashlistreference>\n      <path>{}</path>\n      <c4>{}</c4>\n    </hashlistreference>\n",
                esc(&r.path),
                r.c4
            );
        }
        x.push_str("  </references>\n");
    }
    x.push_str("</hashlist>\n");
    x
}

/// The chain file's text.
pub fn chain_xml(entries: &[ChainEntry]) -> String {
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ascmhldirectory xmlns=\"urn:ASC:MHL:DIRECTORY:v2.0\">\n",
    );
    for e in entries {
        let _ = write!(
            x,
            "  <hashlist sequencenr=\"{}\">\n    <path>{}</path>\n    <c4>{}</c4>\n  </hashlist>\n",
            e.sequence,
            esc(&e.file),
            e.c4
        );
    }
    x.push_str("</ascmhldirectory>\n");
    x
}

/// `0001_<folder>_2026-10-01_081500Z.mhl`: the number, the scope's name, and the time the
/// job started, in UTC.
pub fn manifest_name(sequence: u32, folder: &str, started: DateTime<Utc>) -> String {
    format!(
        "{sequence:04}_{folder}_{}.mhl",
        started.format("%Y-%m-%d_%H%M%SZ")
    )
}

/// What `append` wrote, so `revert` can take it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub manifest: PathBuf,
    pub c4: String,
    pub chain: PathBuf,
    /// The chain as it was (`None`: there was none).
    pub chain_before: Option<Vec<u8>>,
    /// The chain as this call wrote it: `revert` only takes back its own.
    pub chain_after: Vec<u8>,
    pub created_folder: bool,
}

/// Appends `g` to the history at `scope`, whose chain was `chain_before` (its bytes, `None`
/// for a new history) with `entries_before`. Refuses (`AlreadyExists`) when the chain on disk
/// isn't `chain_before` any more: something else wrote to the history.
pub fn append(
    scope: &Path,
    chain_before: Option<&[u8]>,
    entries_before: &[ChainEntry],
    g: &Generation,
    started: DateTime<Utc>,
) -> io::Result<Written> {
    append_then_sync(
        scope,
        chain_before,
        entries_before,
        g,
        started,
        crate::os::full_barrier,
    )
}

/// `append`, with how the folder is synced after the chain is replaced.
fn append_then_sync(
    scope: &Path,
    chain_before: Option<&[u8]>,
    entries_before: &[ChainEntry],
    g: &Generation,
    started: DateTime<Utc>,
    sync_dir: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<Written> {
    let folder = scope.join(FOLDER);
    let chain = folder.join(CHAIN);
    if read_chain(&chain)?.as_deref() != chain_before {
        return Err(changed());
    }
    let created_folder = match fs::create_dir(&folder) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && folder.is_dir() => false,
        Err(e) => return Err(e),
    };
    let sequence = entries_before.len() as u32 + 1;
    let name = scope
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let manifest = folder.join(manifest_name(sequence, &name, started));
    let text = manifest_xml(g);
    let c4 = super::c4::c4(text.as_bytes());
    let mut entries = entries_before.to_vec();
    entries.push(ChainEntry {
        sequence,
        file: manifest_name(sequence, &name, started),
        c4: c4.clone(),
    });
    let chain_text = chain_xml(&entries);
    let written = Written {
        manifest: manifest.clone(),
        c4,
        chain: chain.clone(),
        chain_before: chain_before.map(<[u8]>::to_vec),
        chain_after: chain_text.clone().into_bytes(),
        created_folder,
    };
    // Never through a link, never over a file: a taken name is an error, and isn't removed.
    let mut f = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest)
    {
        Ok(f) => f,
        Err(e) => {
            if created_folder {
                let _ = fs::remove_dir(&folder);
            }
            return Err(e);
        }
    };
    let before_chain = (|| {
        f.write_all(text.as_bytes())?;
        crate::os::sync_durable(&f)?;
        // Once more right before the chain is replaced: the window for another tool to
        // append unseen is as small as it can be without a lock the standard doesn't have.
        if read_chain(&chain)?.as_deref() != chain_before {
            return Err(changed());
        }
        Ok(())
    })();
    if let Err(e) = before_chain {
        // The chain is untouched: only this call's manifest (and folder) go.
        let _ = fs::remove_file(&manifest);
        if created_folder {
            let _ = fs::remove_dir(&folder);
        }
        return Err(e);
    }
    // As in a copy job, only a device error fails the sync, not a file system that can't.
    let synced = |()| match sync_dir(&folder) {
        Err(e) if crate::os::is_device_error(&e) => Err(e),
        _ => Ok(()),
    };
    match replace(&chain, chain_text.as_bytes()).and_then(synced) {
        Ok(()) => Ok(written),
        Err(e) => {
            let _ = revert(&written);
            Err(e)
        }
    }
}

fn changed() -> io::Error {
    io::Error::new(io::ErrorKind::AlreadyExists, "the ASC MHL history changed")
}

fn read_chain(chain: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(chain) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Writes `path` whole: a temporary name next to it (one nobody has: a taken one is left
/// alone), synced, renamed over it.
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let pid = std::process::id();
    let (tmp, mut f) = (0..100)
        .find_map(|n| {
            let suffix = if n == 0 {
                String::new()
            } else {
                format!("-{n}")
            };
            let tmp = path.with_file_name(format!(".{CHAIN}.{pid}{suffix}.secopy-tmp"));
            match OpenOptions::new().write(true).create_new(true).open(&tmp) {
                Ok(f) => Some(Ok((tmp, f))),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => None,
                Err(e) => Some(Err(e)),
            }
        })
        .unwrap_or_else(|| Err(io::Error::from(io::ErrorKind::AlreadyExists)))?;
    // Only the file this call created is removed on a failure.
    let result = f
        .write_all(bytes)
        .and_then(|()| crate::os::sync_durable(&f))
        .and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Takes back what `append` wrote: the chain as it was, the manifest gone, and the folder if
/// it made it (when it's empty). Only while both are still as it wrote them: what another
/// tool wrote since is never undone (`AlreadyExists`, nothing changed).
pub fn revert(w: &Written) -> io::Result<()> {
    let manifest_ours = match fs::read(&w.manifest) {
        Ok(bytes) => super::c4::c4(&bytes) == w.c4,
        Err(e) if e.kind() == io::ErrorKind::NotFound => true,
        Err(e) => return Err(e),
    };
    let chain_now = read_chain(&w.chain)?;
    let chain_ours = chain_now.as_deref() == Some(w.chain_after.as_slice())
        || chain_now.as_deref() == w.chain_before.as_deref();
    if !manifest_ours || !chain_ours {
        return Err(changed());
    }
    match &w.chain_before {
        Some(bytes) => replace(&w.chain, bytes)?,
        None => match fs::remove_file(&w.chain) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        },
    }
    match fs::remove_file(&w.manifest) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    if w.created_folder
        && let Some(folder) = w.chain.parent()
    {
        let _ = fs::remove_dir(folder);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::Hash;
    use crate::mhl::{Action, Generation, Record};
    use chrono::TimeZone;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ascmhl");

    fn generation() -> Generation {
        Generation {
            created: chrono::Local
                .with_ymd_and_hms(2026, 10, 1, 10, 15, 0)
                .unwrap(),
            hostname: "mac.local".into(),
            tool_version: "0.19.0".into(),
            ignore: crate::mhl::ignore::secopy_patterns(&crate::ignore::Patterns::defaults()),
            records: vec![
                Record {
                    rel: "b & c/<C0002>.MP4".into(),
                    size: 5,
                    modified: None,
                    xxh128: Hash::from_u128(0x26c7827d889f6da3),
                    action: Action::Original,
                },
                Record {
                    rel: "A/C0001.MP4".into(),
                    size: 7,
                    modified: Some(
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_790_000_000),
                    ),
                    xxh128: Hash::from_u128(1),
                    action: Action::Verified,
                },
            ],
            references: Vec::new(),
        }
    }

    /// Runs xmllint against the ASC's schema: Secopy's output is what the standard says.
    fn valid(xml: &str, schema: &str) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.xml");
        std::fs::write(&file, xml).unwrap();
        let out = std::process::Command::new("xmllint")
            .args(["--noout", "--schema", &format!("{FIXTURES}/{schema}")])
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn a_manifest_follows_the_schema() {
        let xml = manifest_xml(&generation());
        valid(&xml, "ASCMHL.xsd");
        assert!(xml.contains("<process>transfer</process>"));
        assert!(xml.contains(r#"<tool version="0.19.0">Secopy</tool>"#));
        assert!(
            xml.contains(r#"<xxh128 action="original">000000000000000026c7827d889f6da3</xxh128>"#)
        );
        assert!(xml.contains("b &amp; c/&lt;C0002&gt;.MP4"));
        assert!(
            xml.contains(r#"<xxh128 action="verified">00000000000000000000000000000001</xxh128>"#)
        );
    }

    #[test]
    fn a_manifest_with_references_follows_the_schema() {
        let mut g = generation();
        g.records.clear();
        g.references.push(crate::mhl::Reference {
            path: "A001/ascmhl/0002_A001_2026-10-01_081500Z.mhl".into(),
            c4: crate::mhl::c4::c4(b"x"),
        });
        valid(&manifest_xml(&g), "ASCMHL.xsd");
    }

    #[test]
    fn records_are_sorted_by_path() {
        let xml = manifest_xml(&generation());
        assert!(xml.find("A/C0001.MP4").unwrap() < xml.find("b &amp; c").unwrap());
    }

    #[test]
    fn a_chain_follows_the_schema() {
        let xml = chain_xml(&[ChainEntry {
            sequence: 1,
            file: "0001_A_2026-10-01_081500Z.mhl".into(),
            c4: crate::mhl::c4::c4(b"x"),
        }]);
        valid(&xml, "ASCMHLDirectory__combined.xsd");
    }

    #[test]
    fn a_manifest_is_named_by_number_folder_and_utc_time() {
        let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap();
        assert_eq!(
            manifest_name(1, "A002R2EC", at),
            "0001_A002R2EC_2026-10-01_081500Z.mhl"
        );
        assert_eq!(
            manifest_name(10_000, "A", at),
            "10000_A_2026-10-01_081500Z.mhl"
        );
    }

    #[test]
    fn appending_writes_the_manifest_then_the_chain_and_revert_undoes_it() {
        let scope = tempfile::tempdir().unwrap();
        let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap();
        let first = append(scope.path(), None, &[], &generation(), at).unwrap();
        assert!(first.created_folder);
        let chain = std::fs::read(&first.chain).unwrap();
        assert_eq!(
            first.c4,
            crate::mhl::c4::c4(&std::fs::read(&first.manifest).unwrap())
        );
        let entries = [ChainEntry {
            sequence: 1,
            file: first.manifest.file_name().unwrap().to_string_lossy().into(),
            c4: first.c4.clone(),
        }];
        let second = append(scope.path(), Some(&chain), &entries, &generation(), at).unwrap();
        assert!(
            second
                .manifest
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("0002_")
        );
        revert(&second).unwrap();
        assert_eq!(std::fs::read(&first.chain).unwrap(), chain);
        assert!(!second.manifest.exists());
        revert(&first).unwrap();
        assert!(!scope.path().join("ascmhl").exists());
    }

    /// Code review (#192): as for the checksum file, a file system that can't sync a
    /// directory (SMB: `ENOTSUP`) isn't a failure; a device error is, and takes it back.
    #[test]
    fn only_a_device_error_syncing_the_folder_fails_the_history() {
        let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap();
        let err = |code| move |_: &Path| Err(io::Error::from_raw_os_error(code));
        let scope = tempfile::tempdir().unwrap();
        let written = append_then_sync(
            scope.path(),
            None,
            &[],
            &generation(),
            at,
            err(libc::ENOTSUP),
        );
        assert!(written.unwrap().manifest.exists());
        let scope = tempfile::tempdir().unwrap();
        let failed = append_then_sync(scope.path(), None, &[], &generation(), at, err(libc::EIO));
        assert_eq!(failed.unwrap_err().raw_os_error(), Some(libc::EIO));
        assert!(!scope.path().join("ascmhl").exists());
    }

    #[test]
    fn a_chain_changed_since_it_was_read_isnt_overwritten() {
        let scope = tempfile::tempdir().unwrap();
        let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap();
        let first = append(scope.path(), None, &[], &generation(), at).unwrap();
        let chain = std::fs::read(&first.chain).unwrap();
        // Read as empty, but a history appeared since.
        let err = append(scope.path(), None, &[], &generation(), at).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&first.chain).unwrap(), chain);
    }

    #[test]
    fn a_taken_manifest_name_is_never_removed() {
        let scope = tempfile::tempdir().unwrap();
        let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap();
        let folder = scope.path().join("ascmhl");
        std::fs::create_dir(&folder).unwrap();
        let name = manifest_name(1, &scope.path().file_name().unwrap().to_string_lossy(), at);
        std::fs::write(folder.join(&name), b"someone else's").unwrap();
        let err = append(scope.path(), None, &[], &generation(), at).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(folder.join(&name)).unwrap(),
            b"someone else's"
        );
        assert!(!folder.join("ascmhl_chain.xml").exists());
    }

    /// Review #1: revert never undoes what another tool wrote after Secopy.
    #[test]
    fn revert_leaves_a_history_someone_else_changed() {
        let scope = tempfile::tempdir().unwrap();
        let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap();
        let mine = append(scope.path(), None, &[], &generation(), at).unwrap();
        let entries = [ChainEntry {
            sequence: 1,
            file: mine.manifest.file_name().unwrap().to_string_lossy().into(),
            c4: mine.c4.clone(),
        }];
        let later = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).unwrap();
        let theirs = append(
            scope.path(),
            Some(&mine.chain_after),
            &entries,
            &generation(),
            later,
        )
        .unwrap();
        assert!(revert(&mine).is_err());
        assert_eq!(std::fs::read(&mine.chain).unwrap(), theirs.chain_after);
        assert!(mine.manifest.exists() && theirs.manifest.exists());
    }

    /// Review #1: a manifest someone replaced isn't removed by revert.
    #[test]
    fn revert_leaves_a_replaced_manifest() {
        let scope = tempfile::tempdir().unwrap();
        let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap();
        let mine = append(scope.path(), None, &[], &generation(), at).unwrap();
        std::fs::write(&mine.manifest, b"someone else's").unwrap();
        assert!(revert(&mine).is_err());
        assert_eq!(std::fs::read(&mine.manifest).unwrap(), b"someone else's");
        assert!(mine.chain.exists());
    }

    /// Review #3: a temporary name already taken isn't removed, and the chain still goes in.
    #[test]
    fn a_taken_temporary_name_isnt_removed() {
        let scope = tempfile::tempdir().unwrap();
        let folder = scope.path().join("ascmhl");
        std::fs::create_dir(&folder).unwrap();
        let taken = folder.join(format!(
            ".ascmhl_chain.xml.{}.secopy-tmp",
            std::process::id()
        ));
        std::fs::write(&taken, b"not mine").unwrap();
        let at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 15, 0).unwrap();
        append(scope.path(), None, &[], &generation(), at).unwrap();
        assert_eq!(std::fs::read(&taken).unwrap(), b"not mine");
        assert!(folder.join("ascmhl_chain.xml").is_file());
    }

    #[test]
    fn names_xml_cant_hold_are_known() {
        assert!(xml_can_hold("C0001 é.MP4"));
        assert!(!xml_can_hold("bad\u{1}.MP4"));
        assert!(!xml_can_hold("bad\u{FFFE}.MP4"));
    }
}
