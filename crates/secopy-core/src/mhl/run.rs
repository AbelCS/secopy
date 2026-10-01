//! Recording a copy's ASC MHL once its files are durable: the files already there that need
//! it are read, each scope gets its generation (nested ones first), and a failure takes back
//! everything written (spec "Writing safely").

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, Utc};

use super::ignore::merged;
use super::prepare::{ScopePlan, rel_to};
use super::write::{Written, append, revert};
use super::{Action, Generation, MhlJob, Record, Reference};
use crate::control::JobControl;
use crate::error::IoFailure;
use crate::job::{FileOutcome, FileStatus};
use crate::plan::Plan;

/// What `record` did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recorded {
    pub written: Vec<Written>,
    pub error: Option<IoFailure>,
    /// Files that don't match their earlier hash, relative to the copy root.
    pub failed: Vec<PathBuf>,
}

const READ_BUFFER: usize = 1 << 20;

/// Records `outcomes` (and the plan's files to read) in each scope's history. `progress`
/// gets the bytes read so far.
pub fn record(
    plan: &Plan,
    job: &MhlJob,
    outcomes: &[FileOutcome],
    control: &JobControl,
    progress: &dyn Fn(u64),
) -> Recorded {
    let started = Utc::now();
    let mut files: Vec<(PathBuf, u64)> = outcomes
        .iter()
        .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
        .filter_map(|o| Some((plan.dest.join(&o.final_rel), o.hash?)))
        .collect();
    let mut done = 0;
    for path in &job.plan.to_read {
        match crate::verify::hash_from_device(path, READ_BUFFER, &|b| progress(done + b), control) {
            Ok((hash, _)) => {
                done += fs::metadata(path).map_or(0, |m| m.len());
                files.push((path.clone(), hash));
            }
            Err(e) => return failure(std::io::Error::other(e.to_string())),
        }
    }
    let Some(root) = job.plan.scopes.last().map(|s| s.scope.clone()) else {
        return Recorded::default();
    };

    // Each file in its closest history, with what that history said of it before.
    let mut failed = Vec::new();
    let mut by_scope: HashMap<&Path, Vec<Record>> = HashMap::new();
    for (path, xxh64) in files {
        let Some(scope) = job.plan.scope_of(&path) else {
            continue;
        };
        let Some(rel) = rel_to(&path, &scope.scope) else {
            continue;
        };
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => return failure(e),
        };
        let action = match scope.continues().and_then(|h| h.first_xxh64.get(&rel)) {
            None => Action::Original,
            Some(&before) if before == xxh64 => Action::Verified,
            Some(_) => {
                failed.push(path.strip_prefix(&root).unwrap_or(&path).to_path_buf());
                Action::Failed
            }
        };
        by_scope.entry(&scope.scope).or_default().push(Record {
            rel,
            size: meta.len(),
            modified: meta.modified().ok(),
            xxh64,
            action,
        });
    }

    let created = Local::now();
    let host = hostname();
    let mut written: Vec<Written> = Vec::new();
    for scope in &job.plan.scopes {
        let records = by_scope.remove(scope.scope.as_path()).unwrap_or_default();
        let references: Vec<Reference> = written
            .iter()
            .filter(|w| w.manifest.starts_with(&scope.scope))
            .filter_map(|w| {
                Some(Reference {
                    path: rel_to(&w.manifest, &scope.scope)?,
                    c4: w.c4.clone(),
                })
            })
            .collect();
        if records.is_empty() && references.is_empty() {
            continue;
        }
        let g = Generation {
            created,
            hostname: host.clone(),
            tool_version: job.tool_version.clone(),
            ignore: merged(scope.continues().map_or(&[][..], |h| h.ignore.as_slice())),
            records,
            references,
        };
        match write(scope, &g, started) {
            Ok(w) => written.push(w),
            Err(e) => {
                for w in written.iter().rev() {
                    let _ = revert(w);
                }
                return failure(e);
            }
        }
    }
    failed.sort();
    Recorded {
        written,
        error: None,
        failed,
    }
}

fn write(
    scope: &ScopePlan,
    g: &Generation,
    started: chrono::DateTime<Utc>,
) -> std::io::Result<Written> {
    match scope.continues() {
        Some(h) => append(&scope.scope, Some(&h.chain_bytes), &h.entries, g, started),
        None => append(&scope.scope, None, &[], g, started),
    }
}

fn failure(e: std::io::Error) -> Recorded {
    Recorded {
        error: Some(e.into()),
        ..Recorded::default()
    }
}

/// This Mac's name, as the manifest's `hostname`.
fn hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: the buffer is valid for its length; gethostname writes at most that many bytes.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    if rc != 0 {
        return String::new();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}
