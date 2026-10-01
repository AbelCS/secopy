//! Undoing a cancelled job (#54): the destination goes back to how it was before it.

use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::{FileStatus, JobReport, Landed};
use crate::plan::{Action, Plan};

/// What `undo` did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Undone {
    /// Files the job created, removed.
    pub removed: u64,
    /// Files the job replaced, put back from the archive.
    pub restored: u64,
    /// Files the job replaced with no old version to put back: the new one stays.
    pub not_restored: u64,
    /// Files that couldn't be removed or put back, with why.
    pub failed: Vec<(PathBuf, String)>,
}

/// Removes what the job wrote: the files it created, its checksum file, its ASC MHL
/// generations and the directories it made; files it replaced come back from `archive` (a
/// mirror's `archive_replaced`). Files that were there before are never removed.
pub fn undo(plan: &Plan, report: &JobReport, archive: Option<&Path>) -> Undone {
    let mut done = Undone::default();
    // The ASC MHL generations first, newest first: the histories as they were (#154).
    for w in report.mhl_written.iter().rev() {
        if let Err(e) = crate::mhl::write::revert(w) {
            done.failed.push((w.chain.clone(), e.to_string()));
        }
    }
    for o in &report.outcomes {
        if !matches!(o.status, FileStatus::Copied | FileStatus::Verified) {
            continue;
        }
        let landed = plan.dest.join(&o.final_rel);
        // Still the copy this job made? A file saved in its place since (another inode, or
        // another size) isn't ours to undo (#115).
        if fs::symlink_metadata(&landed)
            .is_ok_and(|m| m.len() != o.size || o.landed_as.is_some_and(|l| l != Landed::of(&m)))
        {
            done.failed.push((
                o.final_rel.clone(),
                "It changed since it was copied, so it was kept.".into(),
            ));
            continue;
        }
        let old = match plan.files[o.id].action {
            Action::Overwrite => archive.map(|a| a.join(&o.rel)).filter(|p| p.is_file()),
            _ => None,
        };
        let result = match (&plan.files[o.id].action, old) {
            (Action::Overwrite, Some(old)) => {
                fs::rename(&old, &landed).map(|()| done.restored += 1)
            }
            (Action::Overwrite, None) => {
                done.not_restored += 1;
                Ok(())
            }
            _ => fs::remove_file(&landed).map(|()| done.removed += 1),
        };
        if let Err(e) = result {
            done.failed.push((o.final_rel.clone(), e.to_string()));
        }
    }
    // Said when it stays (#134): it would list files that are gone.
    if let Some(checksum) = &report.checksum_file
        && let Err(e) = fs::remove_file(checksum)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        let rel = checksum.strip_prefix(&plan.dest).unwrap_or(checksum);
        done.failed.push((rel.to_path_buf(), e.to_string()));
    }
    // Deepest first; only if empty now.
    let mut dirs: Vec<&PathBuf> = report.created_dirs.iter().collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for d in dirs {
        let _ = fs::remove_dir(d);
    }
    if let Some(archive) = archive {
        // The archive run's directories, emptied by putting the old versions back.
        for e in WalkDir::new(archive)
            .contents_first(true)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_dir())
        {
            let _ = fs::remove_dir(e.path());
        }
        if let Some(parent) = archive.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
    done
}
