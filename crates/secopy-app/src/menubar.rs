//! Keep copying in the menu bar (#80): closing the window during a job hides it, and a menu
//! bar icon shows the progress and brings it back. The decisions and the words are here, pure;
//! the icon and the window are handled below them.

use std::time::{Duration, Instant};

use crate::dto::{JobOutcome, JobPhase, ProgressView};

/// A job the icon follows: what it is, its place in a queue, and its last progress.
#[derive(Debug, Clone, PartialEq)]
pub struct Running {
    pub label: String,
    /// (job n, of m) in a queue run.
    pub queue: Option<(u32, u32)>,
    /// A Verify: the work is what was read.
    pub check: bool,
    pub view: ProgressView,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Running(Running),
    /// How it ended; `why` for a stop.
    Finished {
        outcome: JobOutcome,
        why: Option<String>,
    },
}

/// Work done and to do, in bytes, as the progress screen counts it.
fn work(r: &Running) -> (u64, u64) {
    let v = &r.view;
    if r.check {
        (v.verified_bytes, v.total_bytes)
    } else if v.verify {
        (v.copied_bytes + v.verified_bytes, 2 * v.total_bytes)
    } else {
        (v.copied_bytes, v.total_bytes)
    }
}

/// Whole percent done; `None` before there is anything to count.
fn percent(r: &Running) -> Option<u64> {
    let (done, total) = work(r);
    (total > 0).then(|| (done * 100 / total).min(100))
}

/// The text next to the icon.
pub fn title(s: &Status) -> String {
    match s {
        Status::Finished { outcome, .. } => if *outcome == JobOutcome::Complete {
            "✓"
        } else {
            "✗"
        }
        .to_string(),
        Status::Running(r) if r.view.paused => "Paused".into(),
        Status::Running(r) if r.view.phase == JobPhase::Removing => "Removing".into(),
        Status::Running(r) => {
            let p = percent(r).map_or_else(|| "…".to_string(), |p| format!("{p}%"));
            match r.queue {
                Some((n, m)) => format!("{n}/{m} · {p}"),
                None => p,
            }
        }
    }
}

/// The menu's text lines, before its actions.
pub fn lines(s: &Status) -> Vec<String> {
    match s {
        Status::Finished { outcome, why } => vec![match outcome {
            JobOutcome::Complete => "Finished: every file done".to_string(),
            JobOutcome::Failures => "Finished with problems".to_string(),
            JobOutcome::Cancelled => "Cancelled".to_string(),
            JobOutcome::Stopped => match why {
                Some(why) => format!("Stopped: {why}"),
                None => "Stopped".to_string(),
            },
        }],
        Status::Running(r) => {
            let v = &r.view;
            let mut out = vec![r.label.clone()];
            let p = percent(r).map_or_else(|| "…".to_string(), |p| format!("{p}%"));
            out.push(format!("{p} · {} of {} files", v.files_done, v.total_files));
            let (done, total) = work(r);
            if done > 0 && v.elapsed_ms > 0 {
                let speed = done * 1000 / v.elapsed_ms;
                let left_ms = (total - done.min(total)) * 1000 / speed.max(1);
                out.push(format!("{}/s · {} left", bytes(speed), duration(left_ms)));
            }
            out
        }
    }
}

/// "14.0 MB", like the window's `formatBytes`.
fn bytes(n: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let (mut value, mut unit) = (n as f64, 0);
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// "1:22", "1:02:03", like the window's `formatDuration`.
fn duration(ms: u64) -> String {
    let total = (ms + 500) / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Closing the window hides it: the setting is on, something runs, and it isn't a quit.
pub fn should_hide(keep: bool, busy: bool, quitting: bool) -> bool {
    keep && busy && !quitting
}

/// Whether to update the icon now: at most once a second, and always at the end.
pub fn due(last: Option<Instant>, now: Instant, end: bool) -> bool {
    end || last.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(1))
}

/// A queue ended: complete only when every job was, and there was one.
pub fn queue_outcome(complete: u32, count: u32) -> JobOutcome {
    if count > 0 && complete == count {
        JobOutcome::Complete
    } else {
        JobOutcome::Failures
    }
}

/// This progress is the end of what the icon follows: a job's end, but not inside a queue
/// (the queue goes on).
pub fn ends_here(view: &ProgressView, in_queue: bool) -> bool {
    view.phase == JobPhase::Done && !in_queue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{JobOutcome, JobPhase, ProgressView};
    use std::time::{Duration, Instant};

    fn copying(copied: u64, verified: u64) -> Status {
        Status::Running(Running {
            label: "Copy & Verify · CARD_A → Day01".into(),
            queue: None,
            check: false,
            view: ProgressView {
                phase: JobPhase::Copying,
                elapsed_ms: 60_000,
                verify: true,
                total_files: 106,
                files_done: 44,
                total_bytes: 1_000_000_000,
                copied_bytes: copied,
                verified_bytes: verified,
                ..ProgressView::default()
            },
        })
    }

    #[test]
    fn the_title_is_the_work_done() {
        // Copy & Verify counts both passes: 600 + 240 of 2 × 1000 MB = 42 %.
        assert_eq!(title(&copying(600_000_000, 240_000_000)), "42%");
        let Status::Running(mut r) = copying(600_000_000, 240_000_000) else {
            unreachable!()
        };
        r.queue = Some((2, 3));
        assert_eq!(title(&Status::Running(r.clone())), "2/3 · 42%");
        r.view.paused = true;
        assert_eq!(title(&Status::Running(r.clone())), "Paused");
        r.view.paused = false;
        r.view.phase = JobPhase::Removing;
        assert_eq!(title(&Status::Running(r)), "Removing");
        let before = Status::Running(Running {
            label: "x".into(),
            queue: None,
            check: false,
            view: ProgressView::default(),
        });
        assert_eq!(title(&before), "…");
    }

    #[test]
    fn a_check_counts_what_it_read() {
        let s = Status::Running(Running {
            label: "Verify · /Volumes/Backup".into(),
            queue: None,
            check: true,
            view: ProgressView {
                total_bytes: 200,
                verified_bytes: 50,
                elapsed_ms: 10_000,
                ..ProgressView::default()
            },
        });
        assert_eq!(title(&s), "25%");
    }

    #[test]
    fn the_menu_says_the_job_files_speed_and_time_left() {
        assert_eq!(
            lines(&copying(600_000_000, 240_000_000)),
            [
                "Copy & Verify · CARD_A → Day01",
                "42% · 44 of 106 files",
                "14.0 MB/s · 1:23 left",
            ]
        );
        let early = copying(0, 0);
        assert_eq!(lines(&early).len(), 2, "no speed until there is one");
    }

    #[test]
    fn only_complete_is_a_tick() {
        let end = |outcome, why: Option<&str>| Status::Finished {
            outcome,
            why: why.map(String::from),
        };
        assert_eq!(title(&end(JobOutcome::Complete, None)), "✓");
        for o in [
            JobOutcome::Failures,
            JobOutcome::Cancelled,
            JobOutcome::Stopped,
        ] {
            assert_eq!(title(&end(o, None)), "✗", "{o:?}");
        }
        assert_eq!(
            lines(&end(JobOutcome::Complete, None)),
            ["Finished: every file done"]
        );
        assert_eq!(
            lines(&end(JobOutcome::Failures, None)),
            ["Finished with problems"]
        );
        assert_eq!(lines(&end(JobOutcome::Cancelled, None)), ["Cancelled"]);
        assert_eq!(
            lines(&end(
                JobOutcome::Stopped,
                Some("The destination drive is full")
            )),
            ["Stopped: The destination drive is full"]
        );
    }

    #[test]
    fn a_queue_is_complete_only_when_every_job_is() {
        assert_eq!(queue_outcome(3, 3), JobOutcome::Complete);
        assert_eq!(queue_outcome(2, 3), JobOutcome::Failures);
        assert_eq!(queue_outcome(0, 0), JobOutcome::Failures);
    }

    #[test]
    fn hides_only_when_on_busy_and_not_quitting() {
        assert!(should_hide(true, true, false));
        assert!(
            !should_hide(false, true, false),
            "setting off: today's question"
        );
        assert!(!should_hide(true, false, false), "nothing runs: quit");
        assert!(!should_hide(true, true, true), "a quit is never a hide");
    }

    #[test]
    fn updates_are_throttled_but_the_end_always_shows() {
        let t = Instant::now();
        assert!(due(None, t, false));
        assert!(!due(Some(t), t + Duration::from_millis(500), false));
        assert!(due(Some(t), t + Duration::from_millis(1000), false));
        assert!(due(Some(t), t + Duration::from_millis(10), true));
    }

    #[test]
    fn a_jobs_end_inside_a_queue_is_not_the_end() {
        let done = ProgressView {
            phase: JobPhase::Done,
            ..ProgressView::default()
        };
        assert!(ends_here(&done, false));
        assert!(!ends_here(&done, true));
        assert!(!ends_here(&ProgressView::default(), false));
    }
}
