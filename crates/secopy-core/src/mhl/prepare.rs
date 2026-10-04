//! Before a copy starts: which histories it writes to, which files already in the
//! destination it reads, and what stops it (spec "Existing histories", "Blockers").

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::ignore::{Ignore, merged};
use super::read::{Damage, History, read};
use super::write::xml_can_hold;
use super::{CHAIN, FOLDER};
use crate::plan::{Action, Plan};

pub struct MhlInputs {
    /// Where the files go: `Plan::dest` joined with the scan's root directory, if any.
    pub copy_root: PathBuf,
    /// The picked directory (the source of `copy_root`); `None` for files picked one by one.
    pub source_dir: Option<PathBuf>,
    /// The names the copy leaves out (#158): the histories leave them out too.
    pub ignore: crate::ignore::Patterns,
}

/// One history the job writes a generation to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopePlan {
    /// The folder it covers, in the destination.
    pub scope: PathBuf,
    /// The destination's history there, as read now.
    pub dest: Option<History>,
    /// The source's history for the same folder, copied along with the files.
    pub source: Option<History>,
}

impl ScopePlan {
    /// The history this generation continues: the destination's, else the one arriving.
    pub fn continues(&self) -> Option<&History> {
        self.dest.as_ref().or(self.source.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhlPlan {
    /// Deepest first; the copy root last.
    pub scopes: Vec<ScopePlan>,
    /// Files already in the destination that no history records: read and recorded.
    pub to_read: Vec<PathBuf>,
    pub to_read_bytes: u64,
    /// The patterns Secopy adds to each generation's ignore list.
    pub patterns: Vec<String>,
}

impl MhlPlan {
    /// The copy root's generation number: 1 for a new history.
    pub fn generation(&self) -> u32 {
        self.scopes
            .last()
            .and_then(ScopePlan::continues)
            .map_or(0, |h| h.entries.len() as u32)
            + 1
    }

    /// The deepest scope holding `path` (a destination path).
    pub fn scope_of(&self, path: &Path) -> Option<&ScopePlan> {
        self.scopes.iter().find(|s| path.starts_with(&s.scope))
    }
}

/// Why Start is refused (spec "Blockers").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MhlBlocker {
    /// The source and the destination each have a different history for this folder.
    TwoHistories {
        scope: PathBuf,
    },
    /// Overwrite would replace a file the destination's history lists.
    OverwritesRecorded {
        path: PathBuf,
    },
    Damaged {
        scope: PathBuf,
        damage: Damage,
    },
    /// The source's history lists files this copy doesn't bring (or its own files aren't
    /// copied).
    LeavesOut {
        scope: PathBuf,
    },
    /// A name XML can't hold (or not UTF-8), or a folder name a manifest's name can't carry.
    Unlistable {
        path: PathBuf,
    },
    /// A file the source's history lists would land under another name (Keep both) or not
    /// at all (Skip): the history would describe the file already there.
    Conflicts {
        path: PathBuf,
    },
    /// A folder in the destination that can't be read: what's in it can't be listed.
    Unreadable {
        path: PathBuf,
    },
}

/// `path` relative to `base`, `/`-separated; `None` outside it or not UTF-8.
pub fn rel_to(path: &Path, base: &Path) -> Option<String> {
    let rel = path.strip_prefix(base).ok()?;
    let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
    Some(parts?.join("/"))
}

/// With the setting on: the scopes, what to read, or why Start is refused. Marks the source's
/// history files `SkipIdentical` in `plan` when the destination already has that history (the
/// same, or longer).
pub fn prepare(plan: &mut Plan, inputs: &MhlInputs) -> Result<MhlPlan, Vec<MhlBlocker>> {
    let root = &inputs.copy_root;
    let patterns = super::ignore::secopy_patterns(&inputs.ignore);
    let mut blockers = Vec::new();
    let mut scopes: BTreeMap<PathBuf, ScopePlan> = BTreeMap::new();
    let scope = |scopes: &mut BTreeMap<PathBuf, ScopePlan>, at: &Path| {
        scopes.entry(at.to_path_buf()).or_insert_with(|| ScopePlan {
            scope: at.to_path_buf(),
            dest: None,
            source: None,
        });
    };
    scope(&mut scopes, root);

    // The source's histories: where they land, and whether this copy brings them whole.
    let mut source_scopes: Vec<(PathBuf, PathBuf)> = Vec::new();
    if let Some(src) = &inputs.source_dir {
        // Every history in the source, also one a file-type filter leaves out of the copy.
        let mut found: Vec<PathBuf> = WalkDir::new(src)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_dir() && e.file_name() == FOLDER)
            .filter_map(|e| e.path().parent().map(Path::to_path_buf))
            .collect();
        found.sort();
        found.dedup();
        for at in found {
            match read(&at) {
                Ok(None) => {}
                Err(damage) => blockers.push(MhlBlocker::Damaged { scope: at, damage }),
                Ok(Some(h)) => {
                    let Ok(sub) = at.strip_prefix(src) else {
                        continue;
                    };
                    let landed = root.join(sub);
                    scope(&mut scopes, &landed);
                    if leaves_out(plan, &at, &h) {
                        blockers.push(MhlBlocker::LeavesOut {
                            scope: landed.clone(),
                        });
                    }
                    blockers.extend(
                        lands_elsewhere(plan, &at, &h)
                            .into_iter()
                            .map(|path| MhlBlocker::Conflicts { path }),
                    );
                    // Appended to once copied: the checksum file would list it as it was.
                    let own = at.join(FOLDER);
                    for f in plan.files.iter_mut() {
                        if f.entry.source.starts_with(&own) {
                            f.in_checksum_file = false;
                        }
                    }
                    source_scopes.push((at.clone(), landed.clone()));
                    scopes.get_mut(&landed).expect("added").source = Some(h);
                }
            }
        }
    }

    // The destination: its histories, and the files already there.
    let mut existing: Vec<(PathBuf, u64)> = Vec::new();
    if root.is_dir() {
        // A folder the copy ignores isn't entered: what's in it isn't the copy's (#158).
        let mut walk = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                e.depth() == 0
                    || !e.file_type().is_dir()
                    || e.file_name() == FOLDER
                    || !inputs.ignore.matches(e.file_name())
            });
        while let Some(entry) = walk.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    let path = e.path().unwrap_or(root).to_path_buf();
                    blockers.push(MhlBlocker::Unreadable { path });
                    continue;
                }
            };
            if entry.file_type().is_dir() {
                if entry.file_name() == FOLDER {
                    walk.skip_current_dir();
                    let at = entry.path().parent().expect("inside root").to_path_buf();
                    match read(&at) {
                        Ok(None) => {}
                        Err(damage) => blockers.push(MhlBlocker::Damaged { scope: at, damage }),
                        Ok(Some(h)) => {
                            scope(&mut scopes, &at);
                            scopes.get_mut(&at).expect("added").dest = Some(h);
                        }
                    }
                }
            } else if entry.file_type().is_file() {
                let size = entry.metadata().map_or(0, |m| m.len());
                existing.push((entry.into_path(), size));
            }
        }
    }

    // A folder with both: the same history (the destination's is as long or longer) or two.
    for (src_at, landed) in &source_scopes {
        let s = &scopes[landed];
        let (Some(src), Some(dest)) = (&s.source, &s.dest) else {
            continue;
        };
        if dest.entries.starts_with(&src.entries) {
            let history = src_at.join(FOLDER);
            for f in plan.files.iter_mut() {
                if f.entry.source.starts_with(&history) {
                    f.action = Action::SkipIdentical;
                }
            }
        } else {
            blockers.push(MhlBlocker::TwoHistories {
                scope: landed.clone(),
            });
        }
    }

    let mut ordered: Vec<ScopePlan> = scopes.into_values().collect();
    ordered.sort_by_key(|s| std::cmp::Reverse(s.scope.components().count()));
    // A manifest's name carries its folder's name.
    for s in &ordered {
        let name = s.scope.file_name().and_then(|n| n.to_str());
        if !name.is_some_and(|n| xml_can_hold(n) && !n.contains('\\')) {
            blockers.push(MhlBlocker::Unlistable {
                path: s.scope.clone(),
            });
        }
    }
    let deepest = |path: &Path| ordered.iter().find(|s| path.starts_with(&s.scope));

    let mut written: HashSet<PathBuf> = HashSet::new();
    for f in &plan.files {
        let at = plan.dest.join(f.final_rel());
        if !f.final_rel().to_str().is_some_and(xml_can_hold) {
            blockers.push(MhlBlocker::Unlistable { path: at.clone() });
        }
        if f.action == Action::Overwrite
            && let Some(s) = deepest(&at)
            && let Some(h) = &s.dest
            && rel_to(&at, &s.scope).is_some_and(|rel| h.recorded.contains(&rel))
        {
            blockers.push(MhlBlocker::OverwritesRecorded { path: at.clone() });
        }
        if f.action.writes() {
            written.insert(at);
        }
    }

    let mut to_read = Vec::new();
    let mut to_read_bytes = 0;
    for (path, size) in existing {
        if written.contains(&path) {
            continue;
        }
        let Some(s) = deepest(&path) else { continue };
        let Some(rel) = rel_to(&path, &s.scope).filter(|r| xml_can_hold(r)) else {
            blockers.push(MhlBlocker::Unlistable { path });
            continue;
        };
        let previous = s.continues().map_or(&[][..], |h| h.ignore.as_slice());
        if Ignore::new(&merged(previous, &patterns)).matches(&rel, false) {
            continue;
        }
        let recorded = [&s.dest, &s.source]
            .into_iter()
            .flatten()
            .any(|h| h.recorded.contains(&rel));
        if !recorded {
            to_read_bytes += size;
            to_read.push(path);
        }
    }
    to_read.sort();

    if blockers.is_empty() {
        Ok(MhlPlan {
            scopes: ordered,
            to_read,
            to_read_bytes,
            patterns,
        })
    } else {
        blockers.dedup();
        Err(blockers)
    }
}

/// Where files the source history at `at` lists would land under another name or not at all.
fn lands_elsewhere(plan: &Plan, at: &Path, h: &History) -> Vec<PathBuf> {
    let ignore = Ignore::new(&h.ignore);
    plan.files
        .iter()
        .filter(|f| matches!(f.action, Action::KeepBoth { .. } | Action::SkipDiffers))
        .filter(|f| {
            rel_to(&f.entry.source, at)
                .is_some_and(|rel| h.recorded.contains(&rel) && !ignore.matches(&rel, false))
        })
        .map(|f| plan.dest.join(&f.entry.rel))
        .collect()
}

/// Whether the copy leaves out files the source history at `at` lists, or the history's own
/// files: the destination's copy of it would list files that aren't there.
fn leaves_out(plan: &Plan, at: &Path, h: &History) -> bool {
    let copied: HashSet<&Path> = plan
        .files
        .iter()
        .filter(|f| !matches!(f.action, Action::Fail(_)))
        .map(|f| f.entry.source.as_path())
        .collect();
    let own = std::iter::once(CHAIN.to_string()).chain(h.entries.iter().map(|e| e.file.clone()));
    if own
        .map(|name| at.join(FOLDER).join(name))
        .any(|p| !copied.contains(p.as_path()))
    {
        return true;
    }
    let ignore = Ignore::new(&h.ignore);
    h.recorded.iter().any(|rel| {
        let path = at.join(rel);
        !ignore.matches(rel, false)
            && !fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir())
            && !copied.contains(path.as_path())
    })
}
