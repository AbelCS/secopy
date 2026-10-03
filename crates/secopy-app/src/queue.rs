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
    /// With Overwrite, the files it replaces as shown when it was queued, relative to the
    /// destination: at its turn it replaces no other (#112).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overwrite: Vec<PathBuf>,
    /// Also ignore (#164), as it was when queued; a job queued before has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
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

/// `job` as New copy would build it now: scanned and checked at its turn (spec Q3), with
/// the settings at its turn (ASC MHL #154, the ignore list #158).
pub fn prepare(
    job: &CopyJob,
    settings: &crate::store::Settings,
) -> Result<Ready, crate::message::Message> {
    let mut s = Session::new();
    s.set_mhl(settings.write_mhl);
    // Nothing is picked yet: these only set the lists for the scan below (#158, #164).
    let _ = s.begin(Change::Ignore(settings.patterns()));
    let _ = s.begin(Change::JobIgnore(job.ignore.clone()));
    // One scan, with the job's choice (#118): not the folder itself, then again without it.
    let view = apply(
        &mut s,
        Change::PickAs {
            paths: job.sources.clone(),
            include_folder: job.include_folder,
        },
    );
    if let Some(problem) = view.pick_problem {
        return Err(problem);
    }
    s.set_filter(job.extensions.clone());
    s.set_destination(Some(job.destination.clone()));
    let view = s.set_policy(job.conflicts);
    let ready = s.ready().ok_or_else(|| why_not(&view))?;
    // Nobody is there to confirm other files, e.g. another card with the same names (#112).
    if overwrites(&ready.plan)
        .iter()
        .any(|rel| !job.overwrite.contains(rel))
    {
        return Err(crate::msg!("queue.reason.overwriteChanged"));
    }
    Ok(ready)
}

/// The files `plan` replaces, relative to its destination.
pub fn overwrites(plan: &secopy_core::plan::Plan) -> Vec<PathBuf> {
    plan.files
        .iter()
        .filter(|f| f.action == secopy_core::plan::Action::Overwrite)
        .map(|f| f.entry.rel.clone())
        .collect()
}

/// Applies `change` and runs the scan it needs, like the commands do.
fn apply(s: &mut Session, change: Change) -> SessionView {
    match s.begin(change) {
        Ok(pending) => {
            let scanned = scan_source(&pending.source, &pending.ignore);
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

/// Every key a queued copy has in this version, with the entry's own.
fn only_known_copy_keys(value: &serde_json::Value) -> bool {
    const KNOWN: [&str; 10] = [
        "ignore",
        "kind",
        "lastError",
        "sources",
        "includeFolder",
        "extensions",
        "destination",
        "conflicts",
        "verify",
        "overwrite",
    ];
    value
        .as_object()
        .is_some_and(|o| o.keys().all(|k| KNOWN.contains(&k.as_str())))
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
            // A newer Secopy's message this one can't read whole keeps its key (its words
            // show with their placeholders), rather than being dropped and erased on save.
            Some(m @ serde_json::Value::Object(_)) => {
                serde_json::from_value(m.clone()).ok().or_else(|| {
                    m.get("key")
                        .and_then(|k| k.as_str())
                        .map(|key| crate::message::Message {
                            key: key.to_string(),
                            args: Default::default(),
                        })
                })
            }
            _ => None,
        };
        let job = match value.get("kind").and_then(|k| k.as_str()) {
            // A setting this version doesn't know (a newer Secopy's): kept whole, never run or
            // saved without it (#137).
            Some("copy") if !only_known_copy_keys(&value) => QueuedJob::Unknown(value),
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

    /// Review: a last error from a newer Secopy keeps at least its key; a job a newer
    /// Secopy wrote keeps its last error through a save.
    /// QA review (#137): a copy a newer Secopy queued with a setting this one doesn't know
    /// isn't run without it, nor saved without it: it's kept as it was, for the newer one.
    #[test]
    fn a_copy_with_settings_this_version_doesnt_know_is_kept_whole() {
        let entry = serde_json::json!({
            "kind": "copy", "sources": ["/A"], "includeFolder": true, "extensions": null,
            "destination": "/B", "conflicts": "keepBoth", "verify": true, "lastError": null,
            "throttle": 50
        });
        let read: Entry = serde_json::from_value(entry.clone()).unwrap();
        assert!(matches!(read.job, QueuedJob::Unknown(_)));
        assert_eq!(serde_json::to_value(&read).unwrap(), entry);
        let known = serde_json::json!({
            "kind": "copy", "sources": ["/A"], "includeFolder": true, "extensions": null,
            "destination": "/B", "conflicts": "keepBoth", "verify": true, "lastError": null,
            "overwrite": ["a.mov"]
        });
        let read: Entry = serde_json::from_value(known).unwrap();
        assert!(matches!(read.job, QueuedJob::Copy(_)));
        // Every key this version saves is one it knows: a new CopyJob field goes on the list.
        let saved = serde_json::to_value(&read).unwrap();
        assert!(only_known_copy_keys(&saved), "{saved}");
    }

    #[test]
    fn newer_last_errors_are_kept() {
        let e: Entry = serde_json::from_value(serde_json::json!({
            "kind": "check", "directory": "/A", "lastError": { "key": "errors.future" }
        }))
        .unwrap();
        assert_eq!(
            e.last_error.map(|m| m.key),
            Some("errors.future".to_string())
        );
        let e: Entry = serde_json::from_value(serde_json::json!({
            "kind": "check", "directory": "/A",
            "lastError": { "key": "errors.future", "args": { "x": { "unknown": true } } }
        }))
        .unwrap();
        assert_eq!(
            e.last_error.map(|m| m.key),
            Some("errors.future".to_string())
        );
        let newer = serde_json::json!({
            "kind": "teleport", "where": "Mars",
            "lastError": { "key": "queue.reason.stopped", "args": {} }
        });
        let e: Entry = serde_json::from_value(newer).unwrap();
        let saved = serde_json::to_value(&e).unwrap();
        assert_eq!(saved["where"], "Mars");
        assert_eq!(saved["lastError"]["key"], "queue.reason.stopped");
    }

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
            let scanned = scan_source(&pending.source, &pending.ignore);
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
        s.set_destination(Some(dest.clone()));
        s.set_policy(ConflictPolicy::Skip);
        assert_eq!(
            s.copy_job(false),
            Some(CopyJob {
                ignore: Vec::new(),
                sources: vec![clip],
                include_folder: false,
                extensions: Some(vec![Some("mp4".into())]),
                destination: dest,
                conflicts: ConflictPolicy::Skip,
                verify: false,
                overwrite: vec![],
            })
        );
    }

    /// Review focus 1: the job copies what's there at its turn.
    #[test]
    fn prepare_scans_again_at_its_turn() {
        let dir = tempfile::tempdir().unwrap();
        let (clip, dest) = card(dir.path());
        let job = CopyJob {
            ignore: Vec::new(),
            sources: vec![clip.clone()],
            include_folder: true,
            extensions: None,
            destination: dest,
            conflicts: ConflictPolicy::KeepBoth,
            verify: true,
            overwrite: vec![],
        };
        assert_eq!(
            prepare(&job, &crate::store::Settings::default())
                .unwrap()
                .plan
                .files
                .len(),
            2
        );
        std::fs::write(clip.join("b.mp4"), b"b").unwrap();
        assert_eq!(
            prepare(&job, &crate::store::Settings::default())
                .unwrap()
                .plan
                .files
                .len(),
            3
        );
        let only_mp4 = CopyJob {
            extensions: Some(vec![Some("mp4".into())]),
            include_folder: false,
            ..job
        };
        let ready = prepare(&only_mp4, &crate::store::Settings::default()).unwrap();
        assert_eq!(ready.plan.files.len(), 2);
        assert_eq!(ready.copy_root, only_mp4.destination, "contents only");
    }

    #[test]
    fn a_queued_copy_overwrites_only_the_files_shown() {
        let dir = tempfile::tempdir().unwrap();
        let (clip, dest) = card(dir.path());
        std::fs::create_dir_all(dest.join("CLIP")).unwrap();
        std::fs::write(dest.join("CLIP/a.mp4"), b"older").unwrap();
        let mut s = Session::new();
        apply(
            &mut s,
            Change::PickAs {
                paths: vec![clip.clone()],
                include_folder: true,
            },
        );
        s.set_destination(Some(dest.clone()));
        s.set_policy(ConflictPolicy::Overwrite);
        let job = s.copy_job(true).unwrap();
        assert_eq!(job.overwrite, vec![PathBuf::from("CLIP/a.mp4")]);
        assert!(prepare(&job, &crate::store::Settings::default()).is_ok());
        // At its turn another file differs (another card): nobody saw it, so it doesn't start.
        std::fs::write(dest.join("CLIP/a.xml"), b"older").unwrap();
        assert_eq!(
            prepare(&job, &crate::store::Settings::default())
                .err()
                .map(|m| m.key),
            Some("queue.reason.overwriteChanged".to_string())
        );
        // A job queued before this list existed has none: it doesn't overwrite either.
        let old = CopyJob {
            overwrite: vec![],
            ..job
        };
        assert_eq!(
            prepare(&old, &crate::store::Settings::default())
                .err()
                .map(|m| m.key),
            Some("queue.reason.overwriteChanged".to_string())
        );
    }

    #[test]
    fn a_job_that_cant_start_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let (clip, dest) = card(dir.path());
        let gone = CopyJob {
            ignore: Vec::new(),
            sources: vec![dir.path().join("gone")],
            include_folder: true,
            extensions: None,
            destination: dest.clone(),
            conflicts: ConflictPolicy::KeepBoth,
            verify: true,
            overwrite: vec![],
        };
        assert!(
            prepare(&gone, &crate::store::Settings::default())
                .err()
                .unwrap()
                .ends_with("isn’t there any more.")
        );
        let no_dest = CopyJob {
            sources: vec![clip.clone()],
            destination: dir.path().join("no-dest"),
            ..gone.clone()
        };
        assert!(
            !prepare(&no_dest, &crate::store::Settings::default())
                .err()
                .unwrap()
                .is_empty()
        );
        let nothing = CopyJob {
            sources: vec![clip],
            extensions: Some(vec![Some("wav".into())]),
            destination: dest,
            ..gone
        };
        assert_eq!(
            prepare(&nothing, &crate::store::Settings::default())
                .err()
                .unwrap(),
            "Nothing to copy."
        );
    }

    fn job(name: &str) -> CopyJob {
        CopyJob {
            ignore: Vec::new(),
            sources: vec![PathBuf::from(format!("/Volumes/{name}/DCIM"))],
            include_folder: true,
            extensions: Some(vec![Some("mp4".into())]),
            destination: PathBuf::from("/Volumes/V001/Day01"),
            conflicts: ConflictPolicy::KeepBoth,
            verify: true,
            overwrite: vec![],
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

    /// #158: a queued job at its turn leaves out what the saved list names.
    #[test]
    fn a_queued_job_uses_the_saved_list() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("CARD");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.MP4"), b"a").unwrap();
        std::fs::write(src.join("a.LRF"), b"l").unwrap();
        let dest = dir.path().join("dest");
        std::fs::create_dir_all(&dest).unwrap();
        let job = CopyJob {
            ignore: Vec::new(),
            sources: vec![src],
            include_folder: true,
            extensions: None,
            destination: dest,
            conflicts: ConflictPolicy::KeepBoth,
            verify: false,
            overwrite: vec![],
        };
        let lrf = crate::store::Settings {
            ignore: vec!["*.LRF".into()],
            ..crate::store::Settings::default()
        };
        assert_eq!(
            prepare(&job, &crate::store::Settings::default())
                .unwrap()
                .plan
                .files
                .len(),
            2
        );
        assert_eq!(prepare(&job, &lrf).unwrap().plan.files.len(), 1);
    }

    /// #164: a job queued before has no list of its own.
    #[test]
    fn a_queued_job_without_a_list_has_none() {
        let job: CopyJob = serde_json::from_str(
            r#"{"sources":["/Volumes/A/DCIM"],"includeFolder":true,"extensions":null,"destination":"/Volumes/B","conflicts":"keepBoth","verify":true}"#,
        )
        .unwrap();
        assert!(job.ignore.is_empty());
    }

    /// #164: a queued job keeps its own list, on top of the settings' at its turn.
    #[test]
    fn a_queued_job_keeps_its_list() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("CARD");
        std::fs::create_dir_all(&src).unwrap();
        for f in ["a.MP4", "a.LRF", ".gitkeep"] {
            std::fs::write(src.join(f), b"x").unwrap();
        }
        let dest = dir.path().join("dest");
        std::fs::create_dir_all(&dest).unwrap();
        let job = CopyJob {
            sources: vec![src],
            include_folder: true,
            extensions: None,
            destination: dest,
            conflicts: ConflictPolicy::KeepBoth,
            verify: false,
            overwrite: vec![],
            ignore: vec![".gitkeep".into()],
        };
        let lrf = crate::store::Settings {
            ignore: vec!["*.LRF".into()],
            ..crate::store::Settings::default()
        };
        let files = |s: &crate::store::Settings| prepare(&job, s).unwrap().plan.files.len();
        assert_eq!(files(&crate::store::Settings::default()), 2, "a.MP4, a.LRF");
        assert_eq!(files(&lrf), 1, "a.MP4");
    }

    /// #164 review: a queued copy with its own list still loads as a copy.
    #[test]
    fn a_queued_copy_with_a_list_loads_after_a_restart() {
        let mut job = job("A");
        job.ignore = vec![".gitkeep".into()];
        let mut q = Queue::default();
        q.add(job.clone());
        let text = serde_json::to_string(&q).unwrap();
        let read: Queue = serde_json::from_str(&text).unwrap();
        assert!(matches!(&read.jobs[0].job, QueuedJob::Copy(j) if j.ignore == [".gitkeep"]));
    }
}
