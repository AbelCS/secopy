//! Keep copying in the menu bar (#80): closing the window during a job hides it, and a menu
//! bar icon shows the progress and brings it back. The decisions and the words are here, pure;
//! the icon and the window are handled below them.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{ActivationPolicy, AppHandle, Manager};

use crate::commands::AppState;
use crate::dto::{JobOutcome, JobPhase, ProgressView, QueueEvent};
use crate::lock;

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

const TRAY: &str = "secopy-menubar";
const PAUSE: &str = "menubar-pause";
const OPEN: &str = "menubar-open";
const QUIT: &str = "menubar-quit";
const ICON: &[u8] = include_bytes!("../icons/menubar.png");

/// The menu bar icon's state (managed by the app).
#[derive(Default)]
pub struct MenuBar {
    /// The window is hidden and the icon shows: the only thing progress events check first.
    hidden: AtomicBool,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    last: Option<Instant>,
    lines: Vec<String>,
    queue: Option<(u32, u32)>,
    paused: bool,
    finished: bool,
}

/// Makes the icon, then hides the window: never the window without the icon.
pub(crate) fn hide_with(
    make_icon: impl FnOnce() -> Result<(), String>,
    hide_window: impl FnOnce(),
) -> bool {
    if make_icon().is_err() {
        return false;
    }
    hide_window();
    true
}

pub fn is_hidden(app: &AppHandle) -> bool {
    app.state::<MenuBar>().hidden.load(SeqCst)
}

/// Hides the window behind a menu bar icon; `false` (and nothing changed) if the icon can't be made.
pub fn hide(app: &AppHandle) -> bool {
    let status = running_status(app, None);
    let hid = hide_with(
        || make_icon(app, &status).map_err(|e| e.to_string()),
        || {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
            let _ = app.set_activation_policy(ActivationPolicy::Accessory);
        },
    );
    if hid {
        let bar = app.state::<MenuBar>();
        let mut inner = lock(&bar.inner);
        // The queue's place carries over: it was set while the window showed.
        *inner = Inner {
            last: Some(Instant::now()),
            lines: lines(&status),
            queue: inner.queue,
            ..Inner::default()
        };
        drop(inner);
        bar.hidden.store(true, SeqCst);
        // The job may have ended while the icon was made: its end found nothing to update.
        // Nothing runs any more, so the window comes back and closes as with nothing running.
        if !app.state::<AppState>().busy() {
            show(app);
            return false;
        }
    }
    hid
}

/// Shows the window again, back in the Dock; the icon goes.
pub fn show(app: &AppHandle) {
    let bar = app.state::<MenuBar>();
    bar.hidden.store(false, SeqCst);
    let mut inner = lock(&bar.inner);
    // A queue still running keeps its place for the next time the window hides.
    *inner = Inner {
        queue: inner.queue,
        ..Inner::default()
    };
    drop(inner);
    let _ = app.set_activation_policy(ActivationPolicy::Regular);
    let _ = app.remove_tray_by_id(TRAY);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn running_status(app: &AppHandle, view: Option<&ProgressView>) -> Status {
    let state = app.state::<AppState>();
    let bar = app.state::<MenuBar>();
    Status::Running(Running {
        label: state.jobs.label().unwrap_or_else(|| "Secopy".into()),
        queue: lock(&bar.inner).queue,
        check: state.jobs.is_check(),
        view: view.cloned().unwrap_or_default(),
    })
}

fn menu(app: &AppHandle, status: &Status) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    for line in lines(status) {
        menu.append(&MenuItem::new(app, line, false, None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    match status {
        Status::Running(r) => {
            let pause = if r.view.paused { "Resume" } else { "Pause" };
            menu.append(&MenuItem::with_id(app, PAUSE, pause, true, None::<&str>)?)?;
            menu.append(&MenuItem::with_id(
                app,
                OPEN,
                "Open Secopy",
                true,
                None::<&str>,
            )?)?;
            menu.append(&PredefinedMenuItem::separator(app)?)?;
            menu.append(&MenuItem::with_id(
                app,
                QUIT,
                "Quit Secopy…",
                true,
                None::<&str>,
            )?)?;
        }
        Status::Finished { .. } => {
            menu.append(&MenuItem::with_id(
                app,
                OPEN,
                "Open Secopy",
                true,
                None::<&str>,
            )?)?;
            menu.append(&MenuItem::with_id(
                app,
                QUIT,
                "Quit Secopy",
                true,
                None::<&str>,
            )?)?;
        }
    }
    Ok(menu)
}

fn make_icon(app: &AppHandle, status: &Status) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY)
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .title(title(status))
        .menu(&menu(app, status)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            PAUSE => {
                // The job's real state, not what the menu last showed.
                let state = app.state::<AppState>();
                if state.jobs.is_paused() {
                    state.jobs.resume();
                } else {
                    state.jobs.pause();
                }
                // The next progress shows it at once.
                lock(&app.state::<MenuBar>().inner).last = None;
            }
            OPEN => show(app),
            QUIT => crate::quit(app),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Shows `status` on the icon: the title always, the menu only when its lines changed.
/// Never waits: the tray is changed on the main thread, which may itself be waiting for this
/// job (quitting stops the job and waits for it).
fn update(app: &AppHandle, status: &Status) {
    let bar = app.state::<MenuBar>();
    let new_lines = lines(status);
    let paused = matches!(status, Status::Running(r) if r.view.paused);
    let rebuild = {
        let mut inner = lock(&bar.inner);
        let changed = inner.lines != new_lines || inner.paused != paused;
        inner.lines = new_lines;
        inner.paused = paused;
        changed
    };
    let (handle, status) = (app.clone(), status.clone());
    let _ = app.run_on_main_thread(move || {
        let Some(tray) = handle.tray_by_id(TRAY) else {
            return;
        };
        let _ = tray.set_title(Some(title(&status)));
        if rebuild && let Ok(menu) = menu(&handle, &status) {
            let _ = tray.set_menu(Some(menu));
        }
    });
}

/// While a queue checks its next job: its place, nothing else yet.
pub fn checking(queue: (u32, u32)) -> Status {
    Status::Running(Running {
        label: format!("Checking job {} of {}", queue.0, queue.1),
        queue: Some(queue),
        check: false,
        view: ProgressView::default(),
    })
}

/// Sends to the window, then does `follow` (the menu bar's part); a panic there can't keep
/// the window from its progress, nor its Done.
pub fn deliver(send: impl FnOnce(), follow: impl FnOnce()) {
    send();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(follow));
}

/// Quitting: the icon stops following the job, so nothing waits on it.
pub fn forget(app: &AppHandle) {
    app.state::<MenuBar>().hidden.store(false, SeqCst);
}

/// A job's progress: nothing while the window shows; at most once a second while hidden.
pub fn progress(app: &AppHandle, view: &ProgressView) {
    let bar = app.state::<MenuBar>();
    if !bar.hidden.load(SeqCst) {
        return;
    }
    let (in_queue, finished, last) = {
        let inner = lock(&bar.inner);
        (inner.queue.is_some(), inner.finished, inner.last)
    };
    if finished {
        return;
    }
    let end = ends_here(view, in_queue);
    let now = Instant::now();
    if !due(last, now, end) {
        return;
    }
    lock(&bar.inner).last = Some(now);
    let status = if end {
        let summary = app.state::<AppState>().jobs.summary();
        lock(&bar.inner).finished = true;
        Status::Finished {
            outcome: summary.as_ref().map_or(JobOutcome::Stopped, |s| s.outcome),
            why: summary.and_then(|s| s.stopped_because),
        }
    } else {
        running_status(app, Some(view))
    };
    update(app, &status);
}

/// A queue's events: its place, its jobs' progress, and its end.
pub fn queue_event(app: &AppHandle, e: &QueueEvent) {
    let bar = app.state::<MenuBar>();
    match e {
        QueueEvent::JobChecking { index, count } => {
            let place = (index + 1, *count);
            lock(&bar.inner).queue = Some(place);
            if bar.hidden.load(SeqCst) {
                update(app, &checking(place));
            }
        }
        QueueEvent::JobStarted { index, count, .. } => {
            lock(&bar.inner).queue = Some((index + 1, *count));
        }
        QueueEvent::Progress { view } => progress(app, view),
        QueueEvent::Done { summary } => {
            if bar.hidden.load(SeqCst) {
                lock(&bar.inner).finished = true;
                update(
                    app,
                    &Status::Finished {
                        outcome: queue_outcome(summary.complete, summary.count),
                        why: None,
                    },
                );
            }
            lock(&bar.inner).queue = None;
        }
        QueueEvent::Compared { .. } => {}
    }
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

    #[test]
    fn hides_only_with_an_icon() {
        let mut hidden = false;
        assert!(!hide_with(|| Err("no tray".into()), || hidden = true));
        assert!(!hidden, "no icon: the window stays");
        assert!(hide_with(|| Ok(()), || hidden = true));
        assert!(hidden);
    }

    /// Review: while a queue checks its next job, the icon says so, not the last job's figures.
    #[test]
    fn a_queue_checking_its_next_job_says_so() {
        let s = checking((2, 3));
        assert_eq!(title(&s), "2/3 · …");
        assert_eq!(lines(&s)[0], "Checking job 2 of 3");
    }

    /// Review: the window always gets its progress, even if the menu bar's part fails.
    #[test]
    fn the_window_gets_its_progress_even_if_the_menu_bar_fails() {
        let mut sent = false;
        deliver(|| sent = true, || panic!("a menu bar bug"));
        assert!(sent);
    }
}
