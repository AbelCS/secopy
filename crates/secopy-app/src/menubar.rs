//! Keep copying in the menu bar (#80): closing the window during a job hides it, and a menu
//! bar icon shows the progress; clicking it opens a small panel with the job, a progress bar,
//! Pause and Open Secopy. The decisions and the words are here, pure; the icon, the panel and
//! the window are handled below them.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    ActivationPolicy, AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::commands::AppState;
use crate::dto::{JobOutcome, JobPhase, ProgressView, QueueEvent, QueueResult};
use crate::lock;

/// A job the icon follows: what it does, from where to where, its place in a queue, and its
/// last progress.
#[derive(Debug, Clone, PartialEq)]
pub struct Running {
    /// "Copying & verifying", "Mirroring Footage", "Verifying".
    pub heading: String,
    pub from: Option<String>,
    pub to: Option<String>,
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

/// What the panel under the icon shows.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PanelView {
    pub heading: String,
    pub from: Option<String>,
    pub to: Option<String>,
    /// Work done, 0–1; `None` before there is anything to count.
    pub fraction: Option<f64>,
    /// "42%", or "…".
    pub percent: String,
    /// "44 of 106 files".
    pub files: String,
    /// "850.0 MB/s"; `None` until there is a speed.
    pub speed: Option<String>,
    /// "3:12 left".
    pub left: Option<String>,
    pub paused: bool,
    /// A mirror archiving or deleting, or Cancel putting the destination back.
    pub removing: bool,
    /// How it ended, once it has.
    pub ended: Option<Ended>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Ended {
    /// Complete: every file done. Anything else is not.
    pub ok: bool,
    pub text: String,
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

/// How a job ended, in words.
fn ended_text(outcome: JobOutcome, why: Option<&str>) -> String {
    match outcome {
        JobOutcome::Complete => "Finished: every file done".into(),
        JobOutcome::Failures => "Finished with problems".into(),
        JobOutcome::Cancelled => "Cancelled".into(),
        JobOutcome::Stopped => match why {
            Some(why) => format!("Stopped: {why}"),
            None => "Stopped".into(),
        },
    }
}

/// The panel for `s`.
pub fn panel(s: &Status) -> PanelView {
    match s {
        Status::Finished { outcome, why } => PanelView {
            heading: "Secopy".into(),
            from: None,
            to: None,
            fraction: None,
            percent: String::new(),
            files: String::new(),
            speed: None,
            left: None,
            paused: false,
            removing: false,
            ended: Some(Ended {
                ok: *outcome == JobOutcome::Complete,
                text: ended_text(*outcome, why.as_deref()),
            }),
        },
        Status::Running(r) => {
            let v = &r.view;
            let (done, total) = work(r);
            let (speed, left) = if done > 0 && v.elapsed_ms > 0 {
                let speed = done * 1000 / v.elapsed_ms;
                let left_ms = (total - done.min(total)) * 1000 / speed.max(1);
                (
                    Some(format!("{}/s", bytes(speed))),
                    Some(format!("{} left", duration(left_ms))),
                )
            } else {
                (None, None)
            };
            let heading = match r.queue {
                Some((n, m)) if !r.heading.starts_with("Checking") => {
                    format!("Job {n} of {m} · {}", r.heading)
                }
                _ => r.heading.clone(),
            };
            PanelView {
                heading,
                from: r.from.clone(),
                to: r.to.clone(),
                fraction: (total > 0).then(|| (done as f64 / total as f64).min(1.0)),
                percent: percent(r).map_or_else(|| "…".to_string(), |p| format!("{p}%")),
                files: format!("{} of {} files", v.files_done, v.total_files),
                speed,
                left,
                paused: v.paused,
                removing: v.phase == JobPhase::Removing,
                ended: None,
            }
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

/// While a queue checks its next job: its place, nothing else yet.
pub fn checking(queue: (u32, u32)) -> Status {
    Status::Running(Running {
        heading: format!("Checking job {} of {}", queue.0, queue.1),
        from: None,
        to: None,
        queue: Some(queue),
        check: false,
        view: ProgressView::default(),
    })
}

/// What the icon shows first, when the window hides: between a queue's jobs, that it checks
/// the next one; otherwise the job's last real figures (`…` only before there are any).
pub fn opening(queue: Option<(u32, u32)>, checking_next: bool, job: Option<Running>) -> Status {
    match queue {
        Some(place) if checking_next => checking(place),
        _ => Status::Running(job.unwrap_or(Running {
            heading: "Secopy".into(),
            from: None,
            to: None,
            queue,
            check: false,
            view: ProgressView::default(),
        })),
    }
}

/// Whether to update the icon now: at most once a second, and always at the end.
pub fn due(last: Option<Instant>, now: Instant, end: bool) -> bool {
    end || last.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(1))
}

/// A queue ended: cancelled when the user stopped it; complete only when every job was, and
/// there was one.
pub fn queue_outcome(complete: u32, count: u32, cancelled: bool) -> JobOutcome {
    if cancelled {
        JobOutcome::Cancelled
    } else if count > 0 && complete == count {
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

/// A rectangle on screen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Room kept between the panel and the screen's edges and the icon.
const MARGIN: f64 = 8.0;
const GAP: f64 = 4.0;

/// The panel's top-left corner: centred under the icon, kept on the screen.
pub fn panel_position(icon: &Area, panel: (f64, f64), screen: &Area) -> (f64, f64) {
    let x = icon.x + icon.w / 2.0 - panel.0 / 2.0;
    let x = x
        .min(screen.x + screen.w - panel.0 - MARGIN)
        .max(screen.x + MARGIN);
    (x, icon.y + icon.h + GAP)
}

/// Sends to the window, then does `follow` (the menu bar's part); a panic there can't keep
/// the window from its progress, nor its Done.
pub fn deliver(send: impl FnOnce(), follow: impl FnOnce()) {
    send();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(follow));
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

const TRAY: &str = "secopy-menubar";
/// The panel's window, and the event that sends it what to show.
pub const PANEL: &str = "menubar";
pub const PANEL_VIEW: &str = "menubar-view";
const PANEL_SIZE: (f64, f64) = (340.0, 190.0);
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
    /// What the panel shows now, for when it opens.
    view: Option<PanelView>,
    queue: Option<(u32, u32)>,
    /// The queue checks its next job: no job's figures are current.
    checking_next: bool,
    finished: bool,
}

pub fn is_hidden(app: &AppHandle) -> bool {
    app.state::<MenuBar>().hidden.load(SeqCst)
}

/// The panel's window, made hidden at start so it opens at once.
pub fn make_panel(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, PANEL, WebviewUrl::App("index.html?panel".into()))
        .title("Secopy")
        .inner_size(PANEL_SIZE.0, PANEL_SIZE.1)
        .resizable(false)
        .decorations(false)
        // See-through, so the panel's own rounded corners show, with the native shadow.
        .transparent(true)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    Ok(())
}

/// The panel's content now (it asks when it opens).
pub fn panel_view(app: &AppHandle) -> Option<PanelView> {
    lock(&app.state::<MenuBar>().inner).view.clone()
}

fn hide_panel(app: &AppHandle) {
    if let Some(p) = app.get_webview_window(PANEL) {
        let _ = p.hide();
    }
}

/// Opens the panel under the icon, or closes it if it's open. Without a panel, opens Secopy.
fn toggle_panel(app: &AppHandle, icon: Area) {
    let Some(p) = app.get_webview_window(PANEL) else {
        show(app);
        return;
    };
    if p.is_visible().unwrap_or(false) {
        let _ = p.hide();
        return;
    }
    let size = p
        .outer_size()
        .map_or((PANEL_SIZE.0 * 2.0, PANEL_SIZE.1 * 2.0), |s| {
            (f64::from(s.width), f64::from(s.height))
        });
    let screen = app
        .monitor_from_point(icon.x, icon.y)
        .ok()
        .flatten()
        .map(|m| Area {
            x: f64::from(m.position().x),
            y: f64::from(m.position().y),
            w: f64::from(m.size().width),
            h: f64::from(m.size().height),
        })
        .unwrap_or(Area {
            x: 0.0,
            y: 0.0,
            w: 1e6,
            h: 1e6,
        });
    let (x, y) = panel_position(&icon, size, &screen);
    let _ = p.set_position(PhysicalPosition::new(x, y));
    if let Some(view) = panel_view(app) {
        let _ = app.emit_to(PANEL, PANEL_VIEW, view);
    }
    let _ = p.show();
    let _ = p.set_focus();
}

fn make_icon(app: &AppHandle, status: &Status) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY)
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .title(title(status))
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                let scale = app
                    .get_webview_window(PANEL)
                    .and_then(|p| p.scale_factor().ok())
                    .unwrap_or(1.0);
                let pos = rect.position.to_physical::<f64>(scale);
                let size = rect.size.to_physical::<f64>(scale);
                toggle_panel(
                    app,
                    Area {
                        x: pos.x,
                        y: pos.y,
                        w: size.width,
                        h: size.height,
                    },
                );
            }
        })
        .build(app)?;
    Ok(())
}

/// Hides the window behind a menu bar icon; `false` (and nothing changed) if the icon can't
/// be made.
pub fn hide(app: &AppHandle) -> bool {
    let status = opening_status(app);
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
            view: Some(panel(&status)),
            queue: inner.queue,
            checking_next: inner.checking_next,
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

/// Shows the window again, back in the Dock; the icon and the panel go.
pub fn show(app: &AppHandle) {
    let bar = app.state::<MenuBar>();
    bar.hidden.store(false, SeqCst);
    let mut inner = lock(&bar.inner);
    // A queue still running keeps its place for the next time the window hides.
    *inner = Inner {
        queue: inner.queue,
        checking_next: inner.checking_next,
        ..Inner::default()
    };
    drop(inner);
    hide_panel(app);
    let _ = app.set_activation_policy(ActivationPolicy::Regular);
    let _ = app.remove_tray_by_id(TRAY);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// [`opening`] with what the app knows now.
fn opening_status(app: &AppHandle) -> Status {
    let state = app.state::<AppState>();
    let bar = app.state::<MenuBar>();
    let (queue, checking_next) = {
        let inner = lock(&bar.inner);
        (inner.queue, inner.checking_next)
    };
    let job = state
        .jobs
        .progress_view()
        .map(|view| running(&state, queue, view));
    opening(queue, checking_next, job)
}

fn running(state: &AppState, queue: Option<(u32, u32)>, view: ProgressView) -> Running {
    let (heading, from, to) = state
        .jobs
        .describe()
        .unwrap_or_else(|| ("Secopy".into(), String::new(), None));
    Running {
        heading,
        from: Some(from).filter(|f| !f.is_empty()),
        to,
        queue,
        check: state.jobs.is_check(),
        view,
    }
}

/// Never waits: the title is changed on the main thread, which may itself be waiting for this
/// job (quitting stops the job and waits for it); the panel gets an event.
fn update(app: &AppHandle, status: &Status) {
    let view = panel(status);
    lock(&app.state::<MenuBar>().inner).view = Some(view.clone());
    let _ = app.emit_to(PANEL, PANEL_VIEW, view);
    let (handle, text) = (app.clone(), title(status));
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = handle.tray_by_id(TRAY) {
            let _ = tray.set_title(Some(text));
        }
    });
}

/// Quitting: the icon stops following the job, so nothing waits on it.
pub fn forget(app: &AppHandle) {
    app.state::<MenuBar>().hidden.store(false, SeqCst);
    hide_panel(app);
}

/// A job's progress: nothing while the window shows; at most once a second while hidden.
pub fn progress(app: &AppHandle, view: &ProgressView) {
    let bar = app.state::<MenuBar>();
    if !bar.hidden.load(SeqCst) {
        return;
    }
    let (queue, finished, last) = {
        let inner = lock(&bar.inner);
        (inner.queue, inner.finished, inner.last)
    };
    if finished {
        return;
    }
    let end = ends_here(view, queue.is_some());
    let now = Instant::now();
    if !due(last, now, end) {
        return;
    }
    lock(&bar.inner).last = Some(now);
    let state = app.state::<AppState>();
    let status = if end {
        let summary = state.jobs.summary();
        lock(&bar.inner).finished = true;
        Status::Finished {
            outcome: summary.as_ref().map_or(JobOutcome::Stopped, |s| s.outcome),
            why: summary.and_then(|s| s.stopped_because),
        }
    } else {
        Status::Running(running(&state, queue, view.clone()))
    };
    update(app, &status);
}

/// A queue's events: its place, its jobs' progress, and its end.
pub fn queue_event(app: &AppHandle, e: &QueueEvent) {
    let bar = app.state::<MenuBar>();
    match e {
        QueueEvent::JobChecking { index, count } => {
            let place = (index + 1, *count);
            {
                let mut inner = lock(&bar.inner);
                inner.queue = Some(place);
                inner.checking_next = true;
            }
            if bar.hidden.load(SeqCst) {
                update(app, &checking(place));
            }
        }
        QueueEvent::JobStarted { index, count, .. } => {
            let mut inner = lock(&bar.inner);
            inner.queue = Some((index + 1, *count));
            inner.checking_next = false;
        }
        QueueEvent::Progress { view } => progress(app, view),
        QueueEvent::Done { summary } => {
            if bar.hidden.load(SeqCst) {
                lock(&bar.inner).finished = true;
                let cancelled = summary
                    .results
                    .iter()
                    .any(|r| r.result == QueueResult::Cancelled);
                update(
                    app,
                    &Status::Finished {
                        outcome: queue_outcome(summary.complete, summary.count, cancelled),
                        why: None,
                    },
                );
            }
            let mut inner = lock(&bar.inner);
            inner.queue = None;
            inner.checking_next = false;
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
            heading: "Copying & verifying".into(),
            from: Some("/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP".into()),
            to: Some("/Volumes/V001/Day01/CLIP".into()),
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

    fn running(s: Status) -> Running {
        let Status::Running(r) = s else {
            unreachable!()
        };
        r
    }

    fn end(outcome: JobOutcome, why: Option<&str>) -> Status {
        Status::Finished {
            outcome,
            why: why.map(String::from),
        }
    }

    #[test]
    fn the_title_is_the_work_done() {
        // Copy & Verify counts both passes: 600 + 240 of 2 × 1000 MB = 42 %.
        assert_eq!(title(&copying(600_000_000, 240_000_000)), "42%");
        let mut r = running(copying(600_000_000, 240_000_000));
        r.queue = Some((2, 3));
        assert_eq!(title(&Status::Running(r.clone())), "2/3 · 42%");
        r.view.paused = true;
        assert_eq!(title(&Status::Running(r.clone())), "Paused");
        r.view.paused = false;
        r.view.phase = JobPhase::Removing;
        assert_eq!(title(&Status::Running(r)), "Removing");
        assert_eq!(title(&opening(None, false, None)), "…");
    }

    #[test]
    fn a_check_counts_what_it_read() {
        let mut r = running(copying(0, 0));
        r.check = true;
        r.view = ProgressView {
            total_bytes: 200,
            verified_bytes: 50,
            elapsed_ms: 10_000,
            ..ProgressView::default()
        };
        assert_eq!(title(&Status::Running(r)), "25%");
    }

    #[test]
    fn the_panel_shows_the_job_its_progress_speed_and_time_left() {
        let p = panel(&copying(600_000_000, 240_000_000));
        assert_eq!(p.heading, "Copying & verifying");
        assert_eq!(
            p.from.as_deref(),
            Some("/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP")
        );
        assert_eq!(p.to.as_deref(), Some("/Volumes/V001/Day01/CLIP"));
        assert_eq!((p.percent.as_str(), p.fraction), ("42%", Some(0.42)));
        assert_eq!(p.files, "44 of 106 files");
        assert_eq!(p.speed.as_deref(), Some("14.0 MB/s"));
        assert_eq!(p.left.as_deref(), Some("1:23 left"));
        assert!(p.ended.is_none() && !p.paused);
        let early = panel(&copying(0, 0));
        assert_eq!(
            (early.speed, early.left),
            (None, None),
            "no speed until there is one"
        );
    }

    #[test]
    fn in_a_queue_the_panel_says_which_job() {
        let mut r = running(copying(600_000_000, 240_000_000));
        r.queue = Some((2, 3));
        assert_eq!(
            panel(&Status::Running(r)).heading,
            "Job 2 of 3 · Copying & verifying"
        );
        let p = panel(&checking((2, 3)));
        assert_eq!(p.heading, "Checking job 2 of 3");
        assert_eq!(p.fraction, None);
    }

    #[test]
    fn only_complete_is_a_tick() {
        assert_eq!(title(&end(JobOutcome::Complete, None)), "✓");
        for o in [
            JobOutcome::Failures,
            JobOutcome::Cancelled,
            JobOutcome::Stopped,
        ] {
            assert_eq!(title(&end(o, None)), "✗", "{o:?}");
        }
        let ended = |s: &Status| panel(s).ended.map(|e| (e.ok, e.text));
        assert_eq!(
            ended(&end(JobOutcome::Complete, None)),
            Some((true, "Finished: every file done".into()))
        );
        assert_eq!(
            ended(&end(JobOutcome::Failures, None)),
            Some((false, "Finished with problems".into()))
        );
        assert_eq!(
            ended(&end(JobOutcome::Cancelled, None)),
            Some((false, "Cancelled".into()))
        );
        assert_eq!(
            ended(&end(
                JobOutcome::Stopped,
                Some("The destination drive is full")
            )),
            Some((false, "Stopped: The destination drive is full".into()))
        );
    }

    #[test]
    fn a_queue_is_complete_only_when_every_job_is() {
        assert_eq!(queue_outcome(3, 3, false), JobOutcome::Complete);
        assert_eq!(queue_outcome(2, 3, false), JobOutcome::Failures);
        assert_eq!(queue_outcome(0, 0, false), JobOutcome::Failures);
        assert_eq!(queue_outcome(1, 3, true), JobOutcome::Cancelled);
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

    #[test]
    fn the_window_gets_its_progress_even_if_the_menu_bar_fails() {
        let mut sent = false;
        deliver(|| sent = true, || panic!("a menu bar bug"));
        assert!(sent);
    }

    #[test]
    fn the_icon_starts_from_what_is_true_now() {
        let job = running(copying(600_000_000, 240_000_000));
        assert_eq!(title(&opening(None, false, Some(job.clone()))), "42%");
        assert_eq!(
            panel(&opening(Some((2, 3)), true, Some(job))).heading,
            "Checking job 2 of 3"
        );
    }

    /// #80: the panel opens centred under the icon, and never off the screen.
    #[test]
    fn the_panel_sits_under_the_icon_on_screen() {
        let screen = Area {
            x: 0.0,
            y: 0.0,
            w: 1440.0,
            h: 900.0,
        };
        let icon = |x| Area {
            x,
            y: 0.0,
            w: 30.0,
            h: 24.0,
        };
        assert_eq!(
            panel_position(&icon(1000.0), (320.0, 200.0), &screen),
            (855.0, 28.0)
        );
        assert_eq!(
            panel_position(&icon(1420.0), (320.0, 200.0), &screen),
            (1112.0, 28.0)
        );
        assert_eq!(
            panel_position(&icon(0.0), (320.0, 200.0), &screen),
            (8.0, 28.0)
        );
    }
}
