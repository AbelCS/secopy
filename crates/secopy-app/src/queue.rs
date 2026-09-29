//! The job queue (plan 6, RFD §5.7): jobs saved as set up, run one after another.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::dto::{ConflictPolicy, ExtensionKey};

pub const QUEUE: &str = "queue.json";

/// A copy as set up on New copy (spec Q2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyJob {
    pub sources: Vec<PathBuf>,
    pub include_folder: bool,
    /// `None` = every file type.
    pub extensions: Option<Vec<ExtensionKey>>,
    pub destination: PathBuf,
    pub conflicts: ConflictPolicy,
    pub verify: bool,
}

/// A queued job; kinds this version doesn't know are kept as they were written.
#[derive(Debug, Clone, PartialEq)]
pub enum QueuedJob {
    Copy(CopyJob),
    /// A mirror preset, planned at its turn (plan 7).
    Mirror {
        preset: String,
    },
    /// A directory checked against its checksum files, planned at its turn (plan 8).
    Check {
        directory: PathBuf,
    },
    Unknown(serde_json::Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub job: QueuedJob,
    /// Why it failed the last time the queue ran. Queues saved before #84 hold an English
    /// sentence: it loads as `errors.legacy`, shown as it was.
    pub last_error: Option<crate::message::Message>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum OnFailure {
    #[default]
    Continue,
    Stop,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Queue {
    pub on_failure: OnFailure,
    pub jobs: Vec<Entry>,
}

impl Queue {
    pub fn add(&mut self, job: CopyJob) {
        self.jobs.push(Entry {
            job: QueuedJob::Copy(job),
            last_error: None,
        });
    }

    pub fn add_check(&mut self, directory: PathBuf) {
        self.jobs.push(Entry {
            job: QueuedJob::Check { directory },
            last_error: None,
        });
    }

    pub fn add_mirror(&mut self, preset: &str) {
        self.jobs.push(Entry {
            job: QueuedJob::Mirror {
                preset: preset.to_string(),
            },
            last_error: None,
        });
    }

    pub fn remove(&mut self, index: usize) -> bool {
        (index < self.jobs.len())
            .then(|| self.jobs.remove(index))
            .is_some()
    }

    pub fn move_job(&mut self, from: usize, to: usize) -> bool {
        if from >= self.jobs.len() || to >= self.jobs.len() {
            return false;
        }
        let entry = self.jobs.remove(from);
        self.jobs.insert(to, entry);
        true
    }

    pub fn clear(&mut self) {
        self.jobs.clear();
    }
}

use crate::dto::SessionView;
use crate::session::{Change, Ready, Session, scan_source};

/// `job` as New copy would build it now: scanned and checked at its turn (spec Q3).
pub fn prepare(job: &CopyJob) -> Result<Ready, crate::message::Message> {
    let mut s = Session::new();
    let mut view = apply(&mut s, Change::Pick(job.sources.clone()));
    if view.pick_problem.is_none() && !job.include_folder {
        view = apply(&mut s, Change::IncludeFolder(false));
    }
    if let Some(problem) = view.pick_problem {
        return Err(problem);
    }
    s.set_filter(job.extensions.clone());
    s.set_policy(job.conflicts);
    let view = s.set_destination(Some(job.destination.clone()));
    s.ready().ok_or_else(|| why_not(&view))
}

/// Applies `change` and runs the scan it needs, like the commands do.
fn apply(s: &mut Session, change: Change) -> SessionView {
    match s.begin(change) {
        Ok(pending) => {
            let scanned = scan_source(&pending.source);
            s.finish_scan(pending, scanned)
        }
        Err(view) => *view,
    }
}

fn why_not(view: &SessionView) -> crate::message::Message {
    view.destination
        .as_ref()
        .and_then(|d| d.blocker.clone())
        .or_else(|| view.plan.as_ref().and_then(|p| p.blocker.clone()))
        .unwrap_or_else(|| crate::msg!("queue.reason.nothingToCopy"))
}

impl Serialize for Entry {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let mut value = match &self.job {
            QueuedJob::Copy(job) => {
                let mut v = serde_json::to_value(job).map_err(S::Error::custom)?;
                v["kind"] = "copy".into();
                v
            }
            QueuedJob::Mirror { preset } => {
                serde_json::json!({ "kind": "mirror", "preset": preset })
            }
            QueuedJob::Check { directory } => {
                serde_json::json!({ "kind": "check", "directory": directory.to_string_lossy() })
            }
            QueuedJob::Unknown(v) => v.clone(),
        };
        if let Some(object) = value.as_object_mut() {
            let last_error = serde_json::to_value(&self.last_error).map_err(S::Error::custom)?;
            object.insert("lastError".into(), last_error);
        }
        value.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        let last_error = match value.get("lastError") {
            Some(serde_json::Value::String(text)) => {
                Some(crate::msg!("errors.legacy", text = text.as_str()))
            }
            Some(m @ serde_json::Value::Object(_)) => serde_json::from_value(m.clone()).ok(),
            _ => None,
        };
        let job = match value.get("kind").and_then(|k| k.as_str()) {
            Some("copy") => serde_json::from_value::<CopyJob>(value.clone())
                .map(QueuedJob::Copy)
                .unwrap_or(QueuedJob::Unknown(value)),
            Some("mirror") => match value.get("preset").and_then(|p| p.as_str()) {
                Some(preset) => QueuedJob::Mirror {
                    preset: preset.to_string(),
                },
                None => QueuedJob::Unknown(value),
            },
            Some("check") => match value.get("directory").and_then(|d| d.as_str()) {
                Some(directory) => QueuedJob::Check {
                    directory: PathBuf::from(directory),
                },
                None => QueuedJob::Unknown(value),
            },
            _ => QueuedJob::Unknown(value),
        };
        Ok(Self { job, last_error })
    }
}

#[cfg(test)]
mod tests {
    /// #84: a job's last error is saved as a message; an older queue's English sentence
    /// still loads, shown as it was.
    #[test]
    fn last_errors_are_messages_and_old_sentences_still_load() {
        let old: Entry = serde_json::from_value(serde_json::json!({
            "kind": "check", "directory": "/Volumes/A", "lastError": "The destination drive is full."
        }))
        .unwrap();
        assert_eq!(
            old.last_error,
            Some(crate::msg!(
                "errors.legacy",
                text = "The destination drive is full."
            ))
        );
        let new = Entry {
            job: QueuedJob::Check {
                directory: PathBuf::from("/Volumes/A"),
            },
            last_error: Some(crate::msg!("queue.reason.filesFailed", count = 2u32)),
        };
        let json = serde_json::to_value(&new).unwrap();
        assert_eq!(json["lastError"]["key"], "queue.reason.filesFailed");
        assert_eq!(serde_json::from_value::<Entry>(json).unwrap(), new);
        for odd in [
            serde_json::Value::Null,
            serde_json::json!(3),
            serde_json::json!({ "x": 1 }),
        ] {
            let e: Entry = serde_json::from_value(
                serde_json::json!({ "kind": "check", "directory": "/A", "lastError": odd }),
            )
            .unwrap();
            assert_eq!(e.last_error, None);
        }
    }

    use super::*;
    use crate::store::Store;

    use crate::session::{Change, Session, scan_source};

    /// Applies `change` and runs its scan, like the commands do.
    fn apply(s: &mut Session, change: Change) {
        if let Ok(pending) = s.begin(change) {
            let scanned = scan_source(&pending.source);
            s.finish_scan(pending, scanned);
        }
    }

    fn card(dir: &std::path::Path) -> (PathBuf, PathBuf) {
        let card = dir.join("CARD");
        std::fs::create_dir_all(card.join("CLIP")).unwrap();
        std::fs::write(card.join("CLIP/a.mp4"), b"a").unwrap();
        std::fs::write(card.join("CLIP/a.xml"), b"x").unwrap();
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).unwrap();
        (card.join("CLIP"), dest)
    }

    #[test]
    fn new_copy_is_saved_as_set_up() {
        let dir = tempfile::tempdir().unwrap();
        let (clip, dest) = card(dir.path());
        let mut s = Session::new();
        assert_eq!(s.copy_job(true), None, "nothing set up");
        apply(&mut s, Change::Pick(vec![clip.clone()]));
        apply(&mut s, Change::IncludeFolder(false));
        s.set_filter(Some(vec![Some("mp4".into())]));
        s.set_policy(ConflictPolicy::Skip);
        s.set_destination(Some(dest.clone()));
        assert_eq!(
            s.copy_job(false),
            Some(CopyJob {
                sources: vec![clip],
                include_folder: false,
                extensions: Some(vec![Some("mp4".into())]),
                destination: dest,
                conflicts: ConflictPolicy::Skip,
                verify: false,
            })
        );
    }

    /// Review focus 1: the job copies what's there at its turn.
    #[test]
    fn prepare_scans_again_at_its_turn() {
        let dir = tempfile::tempdir().unwrap();
        let (clip, dest) = card(dir.path());
        let job = CopyJob {
            sources: vec![clip.clone()],
            include_folder: true,
            extensions: None,
            destination: dest,
            conflicts: ConflictPolicy::KeepBoth,
            verify: true,
        };
        assert_eq!(prepare(&job).unwrap().plan.files.len(), 2);
        std::fs::write(clip.join("b.mp4"), b"b").unwrap();
        assert_eq!(prepare(&job).unwrap().plan.files.len(), 3);
        let only_mp4 = CopyJob {
            extensions: Some(vec![Some("mp4".into())]),
            include_folder: false,
            ..job
        };
        let ready = prepare(&only_mp4).unwrap();
        assert_eq!(ready.plan.files.len(), 2);
        assert_eq!(ready.copy_root, only_mp4.destination, "contents only");
    }

    #[test]
    fn a_job_that_cant_start_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let (clip, dest) = card(dir.path());
        let gone = CopyJob {
            sources: vec![dir.path().join("gone")],
            include_folder: true,
            extensions: None,
            destination: dest.clone(),
            conflicts: ConflictPolicy::KeepBoth,
            verify: true,
        };
        assert!(
            prepare(&gone)
                .err()
                .unwrap()
                .ends_with("isn't there any more.")
        );
        let no_dest = CopyJob {
            sources: vec![clip.clone()],
            destination: dir.path().join("no-dest"),
            ..gone.clone()
        };
        assert!(!prepare(&no_dest).err().unwrap().is_empty());
        let nothing = CopyJob {
            sources: vec![clip],
            extensions: Some(vec![Some("wav".into())]),
            destination: dest,
            ..gone
        };
        assert_eq!(prepare(&nothing).err().unwrap(), "Nothing to copy.");
    }

    fn job(name: &str) -> CopyJob {
        CopyJob {
            sources: vec![PathBuf::from(format!("/Volumes/{name}/DCIM"))],
            include_folder: true,
            extensions: Some(vec![Some("mp4".into())]),
            destination: PathBuf::from("/Volumes/V001/Day01"),
            conflicts: ConflictPolicy::KeepBoth,
            verify: true,
        }
    }

    #[test]
    fn jobs_are_added_moved_removed_and_cleared() {
        let mut q = Queue::default();
        q.add(job("A"));
        q.add(job("B"));
        q.add(job("C"));
        assert!(q.move_job(2, 0));
        let names = |q: &Queue| -> Vec<String> {
            q.jobs
                .iter()
                .map(|e| match &e.job {
                    QueuedJob::Copy(c) => c.sources[0].display().to_string(),
                    QueuedJob::Mirror { .. } | QueuedJob::Check { .. } | QueuedJob::Unknown(_) => {
                        "?".into()
                    }
                })
                .collect()
        };
        assert_eq!(
            names(&q),
            ["/Volumes/C/DCIM", "/Volumes/A/DCIM", "/Volumes/B/DCIM"]
        );
        assert!(!q.move_job(3, 0) && !q.remove(3));
        assert!(q.remove(1));
        assert_eq!(names(&q), ["/Volumes/C/DCIM", "/Volumes/B/DCIM"]);
        q.clear();
        assert!(q.jobs.is_empty());
    }

    #[test]
    fn the_queue_round_trips_through_its_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let mut q = Queue {
            on_failure: OnFailure::Stop,
            ..Queue::default()
        };
        q.add(job("A"));
        q.jobs[0].last_error = Some(crate::msg!("errors.source.notConnected", drive = "A"));
        store.save(QUEUE, &q).unwrap();
        let text = std::fs::read_to_string(dir.path().join(QUEUE)).unwrap();
        assert!(
            text.contains("\"kind\": \"copy\"") && text.contains("\"onFailure\": \"stop\""),
            "{text}"
        );
        assert_eq!(store.load::<Queue>(QUEUE), (q, None));
    }

    /// A job a newer Secopy wrote stays in the file as it is (Review focus 5).
    #[test]
    fn unknown_kinds_are_kept_verbatim() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(QUEUE),
            r#"{"version": 1, "onFailure": "continue", "jobs": [{"kind": "sync", "preset": "p1", "lastError": null}]}"#,
        )
        .unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let (q, warning) = store.load::<Queue>(QUEUE);
        assert_eq!(warning, None);
        assert!(matches!(&q.jobs[0].job, QueuedJob::Unknown(v) if v["preset"] == "p1"));
        store.save(QUEUE, &q).unwrap();
        let (again, _) = store.load::<Queue>(QUEUE);
        assert_eq!(again, q);
    }

    #[test]
    fn a_check_is_saved_and_read_back() {
        let mut q = Queue::default();
        q.add_check(PathBuf::from("/Volumes/Backup/Day01"));
        let json = serde_json::to_string(&q).unwrap();
        assert!(json.contains(r#""kind":"check""#), "{json}");
        let back: Queue = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.jobs[0].job,
            QueuedJob::Check {
                directory: PathBuf::from("/Volumes/Backup/Day01")
            }
        );
    }
}
