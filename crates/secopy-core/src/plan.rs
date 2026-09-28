//! Resolves one action per file from the pre-flight and the user's choice for files that
//! differ (FR-16, FR-17).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::FileError;
use crate::fsinfo::FsInfo;
use crate::names::numbered;
use crate::preflight::{Blocker, ConflictKind, Preflight, SourceRoot, clash_key};
use crate::scan::{DirEntry, ScanEntry, ScanProblem, Selection};

/// What to do with files that exist at the destination but differ (FR-17).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DiffersPolicy {
    /// Copy under a new name: `name (1).ext`.
    #[default]
    KeepBoth,
    /// Replace the old file once the new copy is complete (and verified).
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Copy,
    Overwrite,
    /// Copy to this path instead, relative to the destination: the original name
    /// numbered `n`, as in `clip (n).mov`.
    KeepBoth {
        rel: PathBuf,
        n: u32,
    },
    /// Already at the destination: same size and modification time. Not read.
    SkipIdentical,
    SkipDiffers,
    /// A pre-flight problem; the file fails without being read.
    Fail(FileError),
}

impl Action {
    /// True if the job writes this file.
    pub fn writes(&self) -> bool {
        matches!(
            self,
            Action::Copy | Action::Overwrite | Action::KeepBoth { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub entry: ScanEntry,
    pub action: Action,
    /// False if the name can't be written to the checksum file (FR-31).
    pub in_checksum_file: bool,
}

impl PlannedFile {
    /// Where the copy lands, relative to the destination.
    pub fn final_rel(&self) -> &Path {
        match &self.action {
            Action::KeepBoth { rel, .. } => rel,
            _ => &self.entry.rel,
        }
    }
}

/// Everything a job needs, resolved before it starts.
#[derive(Debug, Clone)]
pub struct Plan {
    pub dest: PathBuf,
    /// Same order as `Selection::files`, so ids stay stable.
    pub files: Vec<PlannedFile>,
    pub dirs: Vec<DirEntry>,
    pub fs: FsInfo,
    pub source_roots: Vec<SourceRoot>,
    pub stale_partials: Vec<PathBuf>,
    /// What the scan couldn't read: not copied, so the job can't be complete (#58).
    pub unread: Vec<ScanProblem>,
}

/// Free space kept on top of the bytes to write: 1 % of them, at least 64 MiB.
pub fn space_margin(bytes: u64) -> u64 {
    (bytes / 100).max(64 << 20)
}

impl Plan {
    pub fn resolve(sel: &Selection, pf: &Preflight, policy: DiffersPolicy) -> Plan {
        let problems: HashMap<usize, FileError> = pf
            .file_problems
            .iter()
            .map(|p| (p.id, p.kind.to_error()))
            .collect();
        let conflicts: HashMap<usize, &ConflictKind> =
            pf.conflicts.iter().map(|c| (c.id, &c.kind)).collect();
        let omitted: HashSet<usize> = pf.checksum_omissions.iter().copied().collect();
        let case_sensitive = pf.fs.case_sensitive;
        let mut taken: HashSet<Vec<u8>> = sel
            .files
            .iter()
            .map(|f| clash_key(&f.rel, case_sensitive))
            .collect();
        let files = sel
            .files
            .iter()
            .enumerate()
            .map(|(id, entry)| {
                let action = if let Some(e) = problems.get(&id) {
                    Action::Fail(e.clone())
                } else {
                    match (conflicts.get(&id), policy) {
                        (None, _) => Action::Copy,
                        (Some(ConflictKind::Identical), _) => Action::SkipIdentical,
                        (Some(ConflictKind::Differs { .. }), DiffersPolicy::Skip) => {
                            Action::SkipDiffers
                        }
                        (Some(ConflictKind::Differs { .. }), DiffersPolicy::Overwrite) => {
                            Action::Overwrite
                        }
                        (Some(ConflictKind::Differs { .. }), DiffersPolicy::KeepBoth) => {
                            let (rel, n) =
                                free_name(&pf.dest, &entry.rel, &mut taken, case_sensitive);
                            Action::KeepBoth { rel, n }
                        }
                    }
                };
                PlannedFile {
                    entry: entry.clone(),
                    action,
                    in_checksum_file: !omitted.contains(&id),
                }
            })
            .collect();
        Plan {
            dest: pf.dest.clone(),
            files,
            dirs: sel.dirs.clone(),
            fs: pf.fs.clone(),
            source_roots: pf.source_roots.clone(),
            stale_partials: pf.stale_partials.clone(),
            unread: sel.unread.clone(),
        }
    }

    /// Bytes the job will write.
    pub fn bytes_to_write(&self) -> u64 {
        self.files
            .iter()
            .filter(|f| f.action.writes())
            .map(|f| f.entry.size)
            .sum()
    }

    /// Bytes of every file in the plan, written or not.
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.entry.size).sum()
    }

    /// Problems that stop the job. Empty means Start can be enabled (FR-16).
    pub fn blockers(&self) -> Vec<Blocker> {
        let bytes = self.bytes_to_write();
        let needed = bytes.saturating_add(space_margin(bytes));
        if bytes > 0 && needed > self.fs.free_bytes {
            vec![Blocker::NotEnoughSpace {
                needed,
                free: self.fs.free_bytes,
            }]
        } else {
            vec![]
        }
    }
}

/// `name (1).ext`, `name (2).ext`, …: the first name that is free on disk and not used by
/// another file in the plan.
fn free_name(
    dest: &Path,
    rel: &Path,
    taken: &mut HashSet<Vec<u8>>,
    case_sensitive: bool,
) -> (PathBuf, u32) {
    (1u32..)
        .map(|n| (numbered(rel, n), n))
        .find(|(candidate, _)| {
            let key = clash_key(candidate, case_sensitive);
            let free = !taken.contains(&key) && fs::symlink_metadata(dest.join(candidate)).is_err();
            if free {
                taken.insert(key);
            }
            free
        })
        .expect("some numbered name is free")
}
