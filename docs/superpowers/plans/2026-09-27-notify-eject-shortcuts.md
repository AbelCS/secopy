# Notifications, Eject, Shortcuts and Accessibility (M2, plan 3b-2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A notification when a copy ends in the background, an Eject button on the summary, a File menu with shortcuts plus Space / Esc, an accessibility pass, and no wait at the end of a job (release 0.4.0).

**Architecture:** The engine's progress ticker waits on a channel instead of sleeping. The app gains `tauri-plugin-notification`, an ejectable-drive check (DiskArbitration) with `diskutil eject`, and a File menu whose items the UI enables. The UI sends notifications, shows Eject, handles the shortcuts, and is checked by axe-core and a token-contrast test.

**Tech Stack:** Rust (secopy-core, secopy-app, Tauri 2.12, core-foundation, DiskArbitration), tauri-plugin-notification 2.4.0 / @tauri-apps/plugin-notification ~2.4.0, Svelte 5, Vitest, axe-core 4.13.

**Spec:** `docs/superpowers/specs/2026-09-27-notify-eject-shortcuts-design.md` (issue #20).

## Global Constraints

- Eject is a button on the summary, never automatic; it uses `diskutil eject <mount point>` and only for the copy's source or destination drive.
- A drive is ejectable when DiskArbitration says it is not internal, or its media is removable or ejectable; the root volume `/` never is.
- The notification only when the window isn't in front, and only with the setting **"Notify when a copy finishes"** on (default on, `notifyWhenDone`).
- File menu items: Choose Source… ⌘O, Choose Destination… ⌘D, Start Copy ⌘↩, Cancel Copy ⌘.; enabled only when they apply. Space = Pause/Resume on the progress screen, never while a field or button has focus. Esc = Back on Settings and Profiles, and closes Save as new….
- Text tokens meet WCAG AA 4.5:1 on every surface token; `text-faint` (disabled only) is exempt.
- UI follows `docs/design/design-system.md` (components in `ui/src/lib/ui/`, tokens only).
- Commits: Conventional Commits with `Refs: #20`; author and committer `3268106+AbelCS@users.noreply.github.com`.
- After a DTO or command change: `SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app --lib ui_bindings`.
- Checks: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cd ui && npx vitest run && npm run check`.

## Review Focus

1. **Retry failed after Eject:** the source is gone, so Retry must end in a clear "no source" state, not a hang. Pinned in Task 3 (`retry_after_the_source_is_gone_has_nothing_to_start`).
2. **Space while typing:** never pauses a copy when focus is in a field. Pinned in Task 4 (`Space pauses and resumes, but not while typing`).
3. **Start Copy from the menu when Start is disabled:** nothing starts. Pinned in Task 4 (`the File menu only starts a copy Start would start`).
4. **Notification permission denied:** no error, no retry loop. Pinned in Task 2 (`a refused notification permission is left alone`).
5. **Eject refused** (card in use): the reason is shown and the button can be used again. Pinned in Task 3 (`a refused eject says why and can be tried again`).

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `crates/secopy-core/src/job/mod.rs`, `runner.rs` | ticker wakes when the job ends | 1 |
| `crates/secopy-app/src/store.rs` | `notify_when_done` setting | 2 |
| `ui/src/lib/summaryText.ts` (new) | summary figures and notification text | 2 |
| `ui/src/lib/api.ts` | notify, windowFocused, eject, menu | 2, 3, 4 |
| `crates/secopy-app/src/volumes.rs` | ejectable drive of a path, eject | 3 |
| `crates/secopy-app/src/jobs.rs`, `dto.rs`, `commands.rs` | summary drives, `eject`, `set_menu_state` | 3, 4 |
| `crates/secopy-app/src/lib.rs` | notification plugin, File menu | 2, 4 |
| `ui/src/components/Summary.svelte` | Eject, Safe to eject | 3 |
| `ui/src/App.svelte`, `Setup.svelte`, `JobProgress.svelte`, `SettingsScreen.svelte`, `ProfilesScreen.svelte`, `ProfileBar.svelte` | notification, menu, Space, Esc | 2, 4 |
| `ui/src/lib/ui/*`, `ui/src/app.css` | focus, live phase, contrast | 5 |
| `ui/src/a11y.test.ts`, `ui/src/tokens.test.ts` (new) | axe-core, contrast | 5 |
| docs, checklist | 3b-2 manual checks | 6 |

---

### Task 1: The job ends without waiting for the progress interval

**Files:** Modify `crates/secopy-core/src/job/mod.rs`, `crates/secopy-core/src/job/runner.rs`; Test `crates/secopy-core/tests/job.rs`

**Interfaces:** Produces nothing new; `run_job` keeps its signature.

- [ ] **Step 1: Write the failing test** (in `tests/job.rs`; add `use std::time::{Duration, Instant};` if missing)

```rust
#[test]
fn a_job_ends_without_waiting_for_the_progress_interval() {
    let f = fixture();
    let mut o = opts(false);
    o.progress_interval = Duration::from_secs(2);
    let started = Instant::now();
    run(&plan(&f.src, &f.dest), &o);
    assert!(started.elapsed() < Duration::from_millis(1500), "took {:?}", started.elapsed());
}
```

- [ ] **Step 2: Run it** — `cargo test -p secopy-core --test job a_job_ends_without` → FAIL ("took 2.0…s").

- [ ] **Step 3: Implement.** In `run_job`, replace the `finished` flag and the sleeping ticker:

```rust
    std::thread::scope(|s| {
        // Dropping the sender ends the ticker at once: when the workers are done, and also
        // when joining one panics below (otherwise the scope would wait for the ticker, and
        // the panic would become a hang).
        let (stop_ticker, stop) = mpsc::channel::<()>();
        let ticker = {
            let runner = &runner;
            s.spawn(move || {
                while let Err(mpsc::RecvTimeoutError::Timeout) = stop.recv_timeout(opts.progress_interval) {
                    runner.emit_progress();
                }
            })
        };
```

  Remove `let finished = AtomicBool::new(false);` and `let stop_ticker = SetOnDrop(&finished);`; keep `drop(stop_ticker);` before `ticker.join()`. Remove `SetOnDrop` from `runner.rs` and its import if nothing else uses it.

- [ ] **Step 4: Run** `cargo test -p secopy-core` → all pass; clippy clean.

- [ ] **Step 5: Commit** `fix(core): end a job without waiting for the progress interval` (Refs: #20).

---

### Task 2: Notify when a copy finishes

**Files:** Modify `Cargo.toml`, `crates/secopy-app/Cargo.toml`, `crates/secopy-app/src/lib.rs`, `crates/secopy-app/capabilities/default.json`, `crates/secopy-app/src/store.rs`, `ui/package.json`, `ui/src/lib/api.ts`, `ui/src/test/fake-api.ts`, `ui/src/App.svelte`, `ui/src/components/Summary.svelte`, `ui/src/components/SettingsScreen.svelte`; Create `ui/src/lib/summaryText.ts`, `ui/src/lib/summaryText.test.ts`

**Interfaces:**
- Produces: `Settings.notify_when_done: bool` (TS `notifyWhenDone`); `summaryStats(s: SummaryView): string[]`; `notificationFor(s: SummaryView): { title: string; body: string }`; `Api.windowFocused(): boolean`; `Api.notify(title, body): Promise<void>`.

- [ ] **Step 1: Failing tests**

`crates/secopy-app/src/store.rs` tests:

```rust
    #[test]
    fn notify_when_done_is_on_unless_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SETTINGS), br#"{"version": 1}"#).unwrap();
        let store = Store::new(dir.path().to_path_buf());
        assert!(store.load::<Settings>(SETTINGS).0.notify_when_done);
        let off = Settings { notify_when_done: false, ..Settings::default() };
        store.save(SETTINGS, &off).unwrap();
        assert!(!store.load::<Settings>(SETTINGS).0.notify_when_done);
    }
```

`ui/src/lib/summaryText.test.ts`:

```ts
import { describe, expect, test } from "vitest";
import { summaryView } from "../test/fake-api";
import { notificationFor, summaryStats } from "./summaryText";

describe("summaryText", () => {
  test("the figures of a summary", () => {
    expect(summaryStats(summaryView({ skippedIdentical: 3 }))).toEqual([
      "1,284 files",
      "212.4 GB written",
      "took 4:12",
      "842.9 MB/s average",
      "3 already at the destination, not checked",
    ]);
  });

  test("a notification says the headline and the main figures", () => {
    expect(notificationFor(summaryView())).toEqual({
      title: "✓ All 1,284 files copied and verified",
      body: "1,284 files · 212.4 GB written · took 4:12",
    });
    expect(notificationFor(summaryView({ outcome: "failures", failed: 3 })).title).toBe("✗ 3 files failed");
  });
});
```

(If the speed figure differs, use the one `formatSpeed` gives for 212.4 GB in 252 s; the test pins the list's order and wording.)

`ui/src/App.test.ts`:

```ts
  test("a finished copy notifies only when the window is in the background", async () => {
    const { api, state } = app();
    api.windowFocused.mockReturnValue(false);
    await fireEvent.click(await startButton());
    state.progress!(progressView({ phase: "done" }));
    await waitFor(() =>
      expect(api.notify).toHaveBeenCalledWith(
        "✓ All 1,284 files copied and verified",
        "1,284 files · 212.4 GB written · took 4:12",
      ),
    );
  });

  test("no notification when the window is in front, or when it's turned off", async () => {
    const { api, state } = fakeApi(readyView());
    state.start = startView({ session: readyView(), settings: settingsView({ notifyWhenDone: false }) });
    render(App, { props: { api } });
    api.windowFocused.mockReturnValue(false);
    await fireEvent.click(await startButton());
    state.progress!(progressView({ phase: "done" }));
    await screen.findByText(/All 1,284 files copied and verified/);
    expect(api.notify).not.toHaveBeenCalled();
  });
```

`ui/src/lib/api.test.ts` (mock the plugin):

```ts
import { vi } from "vitest";
const plugin = vi.hoisted(() => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-notification", () => plugin);

test("a refused notification permission is left alone", async () => {
  plugin.isPermissionGranted.mockResolvedValue(false);
  plugin.requestPermission.mockResolvedValue("denied");
  await expect(tauriApi.notify("t", "b")).resolves.toBeUndefined();
  expect(plugin.sendNotification).not.toHaveBeenCalled();
  expect(plugin.requestPermission).toHaveBeenCalledTimes(1);
});

test("with permission, the notification is sent", async () => {
  plugin.isPermissionGranted.mockResolvedValue(true);
  await tauriApi.notify("t", "b");
  expect(plugin.sendNotification).toHaveBeenCalledWith({ title: "t", body: "b" });
});
```

(Import `tauriApi` from `./api`.) `SettingsScreen.test.ts`: clicking "Notify when a copy finishes" calls `setSettings` with `notifyWhenDone: false`.

- [ ] **Step 2: Run** — cargo test (compile error: no field `notify_when_done`), vitest (missing module / functions) → RED.

- [ ] **Step 3: Implement**

- Rust: `tauri-plugin-notification = "2.4.0"` in the workspace and `tauri-plugin-notification.workspace = true` in the app; `.plugin(tauri_plugin_notification::init())` in `run()`; `"notification:default"` in the capability's permissions.
- `store.rs`: `pub notify_when_done: bool` in `Settings` (default `true`), `#[serde(default = "yes")] notify_when_done: bool` in `SettingsOnDisk`, copied in the `Deserialize` impl. Update the three places that build `Settings` literally (tests, commands test).
- UI: `npm install @tauri-apps/plugin-notification@~2.4.0`.
- `summaryText.ts`:

```ts
// What a summary says in short: its figures, and the notification when a copy ends.
import type { SummaryView } from "./bindings";
import { formatBytes, formatCount, formatDuration, formatSpeed, plural } from "./format";
import { headline } from "./headline";

export function summaryStats(s: SummaryView): string[] {
  const speed = formatSpeed(s.millis > 0 ? (s.bytesWritten * 1000) / s.millis : null);
  const items = [plural(s.files, "file"), `${formatBytes(s.bytesWritten)} written`, `took ${formatDuration(s.millis)}`, `${speed} average`];
  if (s.skippedIdentical > 0) items.push(`${formatCount(s.skippedIdentical)} already at the destination, not checked`);
  if (s.skippedDifferent > 0) items.push(`${plural(s.skippedDifferent, "different file")} left as they were`);
  if (s.notStarted > 0) items.push(`${formatCount(s.notStarted)} not started`);
  return items;
}

export function notificationFor(s: SummaryView): { title: string; body: string } {
  const mark = s.outcome === "complete" ? "✓" : "✗";
  return { title: `${mark} ${headline(s)}`, body: summaryStats(s).slice(0, 3).join(" · ") };
}
```

  `Summary.svelte` uses `summaryStats(summary)` instead of its own list.
- `api.ts`:

```ts
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
…
  /** The window is in front; notifications are only for when it isn't. */
  windowFocused: (): boolean => document.hasFocus(),
  /** A macOS notification. Asks for permission once; does nothing if it's refused. */
  notify: async (title: string, body: string): Promise<void> => {
    let granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
    if (granted) sendNotification({ title, body });
  },
```

  Fakes: `windowFocused: vi.fn(() => true)`, `notify: vi.fn((_t: string, _b: string) => Promise.resolve())`, and `notifyWhenDone: true` in `settingsView()` and `App.svelte`'s initial settings.
- `App.svelte` `finish()`:

```ts
  async function finish() {
    summary = (await run(() => api.jobSummary())) ?? null;
    if (!summary) return;
    screen = "summary";
    if (settings.notifyWhenDone && !api.windowFocused()) {
      const { title, body } = notificationFor(summary);
      void api.notify(title, body).catch(() => {}); // a notification is a courtesy, never an error
    }
  }
```

- `SettingsScreen.svelte`: a fourth `Checkbox` "Notify when a copy finishes" (`notifyWhenDone`) with help "Only when Secopy's window isn't in front. macOS asks for permission the first time."

- [ ] **Step 4: Run** everything (regenerate bindings) → PASS.

- [ ] **Step 5: Commit** `feat(app): notify when a copy finishes in the background` (Refs: #20).

---

### Task 3: Eject the card, and say when the destination is safe to eject

**Files:** Modify `crates/secopy-app/src/volumes.rs`, `jobs.rs`, `dto.rs`, `commands.rs`, `lib.rs` (command list), `ui/src/lib/api.ts`, `ui/src/test/fake-api.ts`, `ui/src/lib/ui/Notice.svelte`, `ui/src/components/Summary.svelte`, `ui/src/components/Summary.test.ts`

**Interfaces:**
- Produces: `volumes::DriveRef { name: String, mount_point: String }` (Serialize, Type, camelCase); `volumes::ejectable_drive(path: &Path) -> Option<DriveRef>`; `volumes::eject(mount_point: &Path) -> Result<(), String>`; `SummaryView.source_drive / destination_drive: Option<DriveRef>`; `AppState::eject(&self, mount_point: &str) -> Result<(), String>`; command `eject(mount_point: String)`; `Api.eject(mountPoint)`; `Notice` prop `announce` (default `true`).

- [ ] **Step 1: Failing tests**

`volumes.rs`:

```rust
    #[test]
    fn external_and_removable_drives_can_be_ejected_the_macs_own_disk_cannot() {
        let facts = |internal, removable, ejectable| DiskFacts { internal, removable, ejectable };
        let card = Path::new("/Volumes/CARD_A");
        assert!(ejectable(card, facts(Some(false), Some(false), Some(false))), "USB drive");
        assert!(ejectable(card, facts(Some(true), Some(true), Some(true))), "SD card in the built-in reader");
        assert!(!ejectable(card, facts(Some(true), Some(false), Some(false))), "internal disk");
        assert!(!ejectable(Path::new("/"), facts(Some(false), Some(true), Some(true))), "never the root");
        assert!(!ejectable(card, DiskFacts::default()), "unknown: don't offer it");
    }
```

`commands.rs` tests:

```rust
    #[test]
    fn eject_only_takes_the_copys_own_drives() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().join("data"));
        assert_eq!(state.eject("/Volumes/CARD_A").unwrap_err(), "There is no finished copy.");
        // After a copy between folders on the Mac's own disk, nothing may be ejected.
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("a.mov"), b"a").unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        state.rescan(Change::Pick(vec![card]));
        state.session.lock().unwrap().set_destination(Some(dest));
        state.start(false, Sink::default()).unwrap();
        state.jobs.wait();
        assert_eq!(
            state.eject("/Volumes/Other").unwrap_err(),
            "Secopy only ejects the copy's source or destination drive."
        );
    }

    #[cfg(unix)]
    #[test]
    fn retry_after_the_source_is_gone_has_nothing_to_start() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARD");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("a.mov"), b"a").unwrap();
        fs::write(card.join("b.mov"), b"b").unwrap();
        fs::set_permissions(card.join("b.mov"), fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(card.join("b.mov")).is_ok() {
            return; // running as root
        }
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        let state = AppState::new(dir.path().join("data"));
        state.rescan(Change::Pick(vec![card.clone()]));
        state.session.lock().unwrap().set_destination(Some(dest));
        state.start(false, Sink::default()).unwrap();
        state.jobs.wait();
        fs::set_permissions(card.join("b.mov"), fs::Permissions::from_mode(0o644)).unwrap();
        fs::remove_dir_all(&card).unwrap(); // the card was ejected
        let (source, selection) = state.jobs.retry().unwrap();
        state.session.lock().unwrap().install_retry(source, selection);
        let started = state.start(false, Sink::default());
        state.jobs.wait();
        let summary = state.jobs.summary().unwrap();
        assert!(
            started.is_err() || summary.outcome != JobOutcome::Complete,
            "a retry without its card must not report a complete copy"
        );
    }
```

(Import `crate::dto::JobOutcome` in the tests.) `Summary.test.ts`:

```ts
  test("Eject shows for a card, and says when it's done", async () => {
    const { api } = show(summaryView({ sourceDrive: { name: "CARD_A", mountPoint: "/Volumes/CARD_A" } }));
    await fireEvent.click(screen.getByRole("button", { name: "Eject CARD_A" }));
    expect(api.eject).toHaveBeenCalledWith("/Volumes/CARD_A");
    await screen.findByText("CARD_A was ejected. You can remove it.");
    expect(screen.queryByRole("button", { name: "Eject CARD_A" })).toBeNull();
  });

  test("a refused eject says why and can be tried again", async () => {
    const { api } = show(summaryView({ sourceDrive: { name: "CARD_A", mountPoint: "/Volumes/CARD_A" } }));
    api.eject.mockRejectedValueOnce(new Error("CARD_A is in use by Finder"));
    await fireEvent.click(screen.getByRole("button", { name: "Eject CARD_A" }));
    await screen.findByText("CARD_A is in use by Finder");
    screen.getByRole("button", { name: "Eject CARD_A" });
  });

  test("no Eject for the Mac's own disk; Safe to eject for a removable destination", () => {
    show(summaryView({ destinationDrive: { name: "V001", mountPoint: "/Volumes/V001" } }));
    expect(screen.queryByRole("button", { name: /Eject/ })).toBeNull();
    screen.getByText("Safe to eject V001: everything was written.");
  });
```

- [ ] **Step 2: Run** → RED (missing `DiskFacts`, `ejectable`, `eject`, `sourceDrive`…).

- [ ] **Step 3: Implement**

`volumes.rs`: factor the `statfs` call of `mount_info` into `fn mount_of(path) -> Option<(PathBuf /*mounted on*/, String /*mounted from*/, libc::statfs)>` and use it in both places. Add:

```rust
/// A drive the summary can offer to eject.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DriveRef {
    pub name: String,
    pub mount_point: String,
}

/// What DiskArbitration says about a disk; `None` when it doesn't say.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct DiskFacts {
    internal: Option<bool>,
    removable: Option<bool>,
    ejectable: Option<bool>,
}

/// External, or removable media (an SD card in the built-in reader); never the root.
fn ejectable(mount_point: &Path, f: DiskFacts) -> bool {
    mount_point != Path::new("/")
        && (f.internal == Some(false) || f.removable == Some(true) || f.ejectable == Some(true))
}

/// The ejectable drive holding `path`, if any.
pub fn ejectable_drive(path: &Path) -> Option<DriveRef> {
    let (mounted_on, from, _) = mount_of(path)?;
    let facts = disk_arbitration::facts(from.strip_prefix("/dev/")?)?;
    ejectable(&mounted_on, facts).then(|| DriveRef {
        name: mounted_on.file_name().map_or_else(|| show(&mounted_on), |n| n.to_string_lossy().into_owned()),
        mount_point: show(&mounted_on),
    })
}

/// macOS's own eject: every volume on the drive, and the reason when it can't.
pub fn eject(mount_point: &Path) -> Result<(), String> {
    let out = std::process::Command::new("diskutil")
        .arg("eject")
        .arg(mount_point)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(if out.stderr.is_empty() { &out.stdout } else { &out.stderr });
    Err(text.trim().to_string())
}
```

In `disk_arbitration`, factor `fn description(bsd_name) -> Option<CFDictionary<CFString, CFType>>` out of `is_disk_image`, declare `kDADiskDescriptionDeviceInternalKey`, `kDADiskDescriptionMediaRemovableKey`, `kDADiskDescriptionMediaEjectableKey`, and add:

```rust
    pub(super) fn facts(bsd_name: &str) -> Option<super::DiskFacts> {
        let d = description(bsd_name)?;
        // SAFETY: the keys are DiskArbitration's constant strings.
        let flag = |key: CFStringRef| unsafe {
            d.find(&CFString::wrap_under_get_rule(key))
                .and_then(|v| v.downcast::<CFBoolean>())
                .map(bool::from)
        };
        Some(super::DiskFacts {
            internal: flag(unsafe { kDADiskDescriptionDeviceInternalKey }),
            removable: flag(unsafe { kDADiskDescriptionMediaRemovableKey }),
            ejectable: flag(unsafe { kDADiskDescriptionMediaEjectableKey }),
        })
    }
```

(`use core_foundation::boolean::CFBoolean;`.)

`dto.rs` `SummaryView`: `source_drive: Option<DriveRef>` ("the card or drive the copy came from, when it can be ejected") and `destination_drive: Option<DriveRef>`. `jobs.rs` `summary()`:

```rust
            source_drive: source_path(&job.ready.source).and_then(|p| volumes::ejectable_drive(&p)),
            destination_drive: volumes::ejectable_drive(&job.ready.plan.dest),
```

with `fn source_path(s: &Source) -> Option<PathBuf>` (the directory, or the first file). `commands.rs`:

```rust
    /// Eject the finished copy's source or destination drive (never another one).
    pub fn eject(&self, mount_point: &str) -> Result<(), String> {
        let summary = self.jobs.summary().ok_or("There is no finished copy.")?;
        let ours = [summary.source_drive, summary.destination_drive]
            .into_iter()
            .flatten()
            .any(|d| d.mount_point == mount_point);
        if !ours {
            return Err("Secopy only ejects the copy's source or destination drive.".into());
        }
        volumes::eject(Path::new(mount_point))
    }
```

plus the `#[tauri::command] async fn eject(app, mount_point: String) -> Result<(), String>` (blocking) and its entry in `collect_commands!`. Regenerate bindings.

UI: `api.eject: (mountPoint: string) => unwrap(commands.eject(mountPoint))`; fake `eject: vi.fn((_m: string) => Promise.resolve(null))`; `summaryView()` gains `sourceDrive: null, destinationDrive: null`. `Notice.svelte`: prop `announce = true`; `role={announce ? (tone === "danger" ? "alert" : "status") : undefined}`. `Summary.svelte`:

```ts
  let ejected: string | null = $state(null);
  let ejectError: string | null = $state(null);
  async function eject(drive: DriveRef) {
    try {
      await api.eject(drive.mountPoint);
      ejected = `${drive.name} was ejected. You can remove it.`;
      ejectError = null;
    } catch (e) {
      ejectError = e instanceof Error ? e.message : String(e);
    }
  }
```

In the result block: `{#if ejected}<Notice tone="success" announce={false}>{ejected}</Notice>{/if}`, `{#if ejectError}<Notice tone="danger">{ejectError}</Notice>{/if}`, `{#if summary.destinationDrive}<Notice tone="info" announce={false}>Safe to eject {summary.destinationDrive.name}: everything was written.</Notice>{/if}`. In the action bar's `start`, after Retry failed: `{#if summary.sourceDrive && !ejected}<Button onclick={() => eject(summary.sourceDrive!)}>Eject {summary.sourceDrive.name}</Button>{/if}`.

- [ ] **Step 4: Run** everything → PASS.

- [ ] **Step 5: Commit** `feat(app): eject the card from the summary` (Refs: #20).

---

### Task 4: File menu, Space and Esc

**Files:** Modify `crates/secopy-app/src/lib.rs`, `commands.rs`, `ui/src/lib/api.ts`, `ui/src/test/fake-api.ts`, `ui/src/App.svelte`, `Setup.svelte`, `JobProgress.svelte`, `SettingsScreen.svelte`, `ProfilesScreen.svelte`, `ProfileBar.svelte` and their tests

**Interfaces:**
- Produces: event `"menu"` with payload `"choose-source" | "choose-destination" | "start-copy" | "cancel-copy"`; command `set_menu_state(setup: bool, can_start: bool, copying: bool)`; `lib::menu_state(setup, can_start, copying) -> [bool; 4]` (source, destination, start, cancel); `Api.onMenu(handler)`, `Api.setMenuState(setup, canStart, copying)`; `Setup` exports `chooseSource()`, `chooseDestination()`, `startIfReady()` and a bindable `ready`; `JobProgress` exports `cancel()`.

- [ ] **Step 1: Failing tests**

`lib.rs` tests:

```rust
    #[test]
    fn the_file_menu_offers_only_what_applies() {
        use super::menu_state;
        assert_eq!(menu_state(true, false, false), [true, true, false, false]);
        assert_eq!(menu_state(true, true, false), [true, true, true, false]);
        assert_eq!(menu_state(false, false, true), [false, false, false, true]);
        assert_eq!(menu_state(false, true, false), [false, false, false, false], "Start only on New copy");
    }
```

`App.test.ts` (fake: `state.menu` holds the handler):

```ts
  test("File menu items drive New copy", async () => {
    const { api, state } = app();
    await startButton();
    state.menu!("choose-source");
    await waitFor(() => expect(api.pickSource).toHaveBeenCalled());
    state.menu!("choose-destination");
    await waitFor(() => expect(api.pickDestination).toHaveBeenCalled());
    state.menu!("start-copy");
    await waitFor(() => expect(api.startJob).toHaveBeenCalled());
    await screen.findByRole("heading", { name: "Copying & verifying" });
    state.menu!("cancel-copy");
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
  });

  test("the File menu only starts a copy Start would start", async () => {
    const { api, state } = app(sessionView());
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("start-copy");
    await new Promise((r) => setTimeout(r, 20));
    expect(api.startJob).not.toHaveBeenCalled();
  });

  test("the menu is told what applies", async () => {
    const { api } = app();
    await startButton();
    await waitFor(() => expect(api.setMenuState).toHaveBeenLastCalledWith(true, true, false));
    await fireEvent.click(await startButton());
    await waitFor(() => expect(api.setMenuState).toHaveBeenLastCalledWith(false, false, true));
  });
```

`JobProgress.test.ts`:

```ts
  test("Space pauses and resumes, but not while typing", async () => {
    const { api, rerender } = show(progressView());
    await fireEvent.keyDown(window, { key: " " });
    expect(api.pauseJob).toHaveBeenCalledTimes(1);
    const field = document.createElement("input");
    document.body.append(field);
    await fireEvent.keyDown(field, { key: " " });
    expect(api.pauseJob).toHaveBeenCalledTimes(1);
    field.remove();
    await rerender({ progress: progressView({ paused: true }) });
    await fireEvent.keyDown(window, { key: " " });
    expect(api.resumeJob).toHaveBeenCalledTimes(1);
  });
```

`SettingsScreen.test.ts`: `fireEvent.keyDown(window, { key: "Escape" })` → `calls.done === 1`. `ProfilesScreen.test.ts`: after editing the name, Escape → `api.confirm` called; without edits, Escape → `calls.done === 1`. `Setup.test.ts`: open Save as new…, `fireEvent.keyDown(form, { key: "Escape" })` → the "Name" field is gone.

- [ ] **Step 2: Run** → RED.

- [ ] **Step 3: Implement**

`lib.rs`:

```rust
/// File menu items, and the event that tells the UI one was chosen.
const CHOOSE_SOURCE: &str = "choose-source";
const CHOOSE_DESTINATION: &str = "choose-destination";
const START_COPY: &str = "start-copy";
const CANCEL_COPY: &str = "cancel-copy";
pub const MENU_EVENT: &str = "menu";

/// The File menu's items, kept to grey them out.
pub struct FileMenu<R: Runtime> {
    items: [MenuItem<R>; 4],
}

impl<R: Runtime> FileMenu<R> {
    pub fn update(&self, setup: bool, can_start: bool, copying: bool) {
        for (item, on) in self.items.iter().zip(menu_state(setup, can_start, copying)) {
            let _ = item.set_enabled(on);
        }
    }
}

/// Which File items apply: [Choose Source, Choose Destination, Start Copy, Cancel Copy].
fn menu_state(setup: bool, can_start: bool, copying: bool) -> [bool; 4] {
    [setup, setup, setup && can_start, copying]
}
```

In `menu()`, build the items (`MenuItem::with_id(app, CHOOSE_SOURCE, "Choose Source…", true, Some("CmdOrCtrl+O"))`, `"Choose Destination…"` `CmdOrCtrl+D`, `"Start Copy"` `CmdOrCtrl+Enter` (disabled at first), `"Cancel Copy"` `CmdOrCtrl+Period` (disabled)), a `"File"` submenu (source, destination, separator, start, cancel) placed between the app menu and Edit, and `app.manage(FileMenu { items: [...clones] })`. In `on_menu_event`, emit `MENU_EVENT` with the id for those four ids. `commands.rs`:

```rust
/// The UI says which File menu items apply.
#[tauri::command]
#[specta::specta]
pub fn set_menu_state(app: AppHandle, setup: bool, can_start: bool, copying: bool) {
    if let Some(menu) = app.try_state::<crate::FileMenu<tauri::Wry>>() {
        menu.update(setup, can_start, copying);
    }
}
```

(registered in `collect_commands!`). Check the accelerators parse by launching the app once (`npm run tauri dev` or the built app): a wrong string panics at start.

UI: `api.onMenu: (h: (item: string) => void) => listen<string>("menu", (e) => h(e.payload))`, `api.setMenuState: (setup, canStart, copying) => commands.setMenuState(setup, canStart, copying)`; fakes `onMenu` (stores `state.menu`) and `setMenuState: vi.fn(() => Promise.resolve())`.

`Setup.svelte`: `ready = $bindable(false)` prop, `$effect(() => { ready = canStart; })`; `export function chooseSource`, `export function chooseDestination` (the existing functions), and

```ts
  /** Start from the File menu: only what the Start button would start. */
  export function startIfReady() {
    if (canStart) onStart();
  }
```

`JobProgress.svelte`: `export async function cancel()`; and

```svelte
<svelte:window onkeydown={onKey} />
```

```ts
  /** Space pauses and resumes; not while a field or button has focus (Space is theirs). */
  function onKey(e: KeyboardEvent) {
    if (e.key !== " " || e.repeat || progress.phase === "done") return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, select, button, [contenteditable]")) return;
    e.preventDefault();
    void (progress.paused ? api.resumeJob() : api.pauseJob());
  }
```

`App.svelte`: `bind:this` for Setup and JobProgress, `bind:ready={setupReady}`;

```ts
  $effect(() => {
    void api.setMenuState(screen === "setup", screen === "setup" && setupReady, screen === "progress" && progress?.phase !== "done").catch(() => {});
  });
```

and in `onMount`, `const unlistenMenu = api.onMenu(onMenu)` with

```ts
  function onMenu(item: string) {
    if (item === "choose-source" && screen === "setup") void setupScreen?.chooseSource();
    else if (item === "choose-destination" && screen === "setup") void setupScreen?.chooseDestination();
    else if (item === "start-copy" && screen === "setup") setupScreen?.startIfReady();
    else if (item === "cancel-copy" && screen === "progress") void progressScreen?.cancel();
  }
```

`SettingsScreen.svelte`: `<svelte:window onkeydown={(e) => e.key === "Escape" && onDone()} />`. `ProfilesScreen.svelte`: `<svelte:window onkeydown={(e) => e.key === "Escape" && void back()} />`. `ProfileBar.svelte`: the Save as new form gets `onkeydown={(e) => { if (e.key === "Escape") { e.stopPropagation(); savingAs = false; } }}`.

- [ ] **Step 4: Run** everything → PASS.

- [ ] **Step 5: Commit** `feat(app): a File menu with shortcuts; Space pauses, Esc goes back` (Refs: #20).

---

### Task 5: Accessibility pass

**Files:** Modify `ui/src/app.css`, `ui/src/lib/ui/Button.svelte`, `SegmentedControl.svelte`, `ScreenHeader.svelte`, `ui/src/components/JobProgress.svelte`, `FinishedList.svelte`, `ui/package.json`; Create `ui/src/tokens.test.ts`, `ui/src/a11y.test.ts`

**Interfaces:** Produces token `--accent-strong` (filled accent backgrounds with `--on-accent` text).

- [ ] **Step 1: Failing tests**

`ui/src/tokens.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";

const css = readFileSync(new URL("./app.css", import.meta.url), "utf8");
const tokens = Object.fromEntries([...css.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/gi)].map((m) => [m[1], m[2]]));

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const f = (c: number) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}
const contrast = (a: string, b: string) => {
  const [hi, lo] = [luminance(tokens[a]), luminance(tokens[b])].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

describe("colour tokens (WCAG AA, RFD §5.6)", () => {
  const surfaces = ["bg", "surface", "surface-raised"];
  for (const text of ["text", "text-muted", "accent", "success", "warning", "danger"]) {
    for (const surface of surfaces) {
      test(`${text} on ${surface}`, () => expect(contrast(text, surface)).toBeGreaterThanOrEqual(4.5));
    }
  }
  test("on-accent on accent-strong (filled buttons)", () =>
    expect(contrast("on-accent", "accent-strong")).toBeGreaterThanOrEqual(4.5));
});
```

(`--on-accent` must be written as a hex, `#ffffff`, for the parser.) `ui/src/a11y.test.ts` (dev dependency `axe-core@^4.13`):

```ts
import { render } from "@testing-library/svelte";
import axe from "axe-core";
import { describe, expect, test } from "vitest";
import App from "./App.svelte";
import JobProgress from "./components/JobProgress.svelte";
import ProfilesScreen from "./components/ProfilesScreen.svelte";
import SettingsScreen from "./components/SettingsScreen.svelte";
import Summary from "./components/Summary.svelte";
import { apiContext } from "./lib/api";
import { fakeApi, profile, progressView, readyView, settingsView, summaryView } from "./test/fake-api";

async function violations(container: HTMLElement): Promise<string[]> {
  // Colours are checked by tokens.test.ts; the test DOM can't compute them.
  const result = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
  return result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`);
}

describe("accessibility (axe-core)", () => {
  test("New copy", async () => {
    const { api } = fakeApi(readyView());
    const { container } = render(App, { props: { api } });
    await new Promise((r) => setTimeout(r, 20));
    expect(await violations(container)).toEqual([]);
  });

  test("Copying", async () => {
    const { api } = fakeApi();
    const { container } = render(JobProgress, { props: { progress: progressView({ filesDone: 3 }) }, context: apiContext(api) });
    expect(await violations(container)).toEqual([]);
  });

  test("Summary", async () => {
    const { api } = fakeApi();
    const props = { summary: summaryView(), onRetry: () => {}, onNewCopy: () => {}, onSettings: () => {} };
    const { container } = render(Summary, { props, context: apiContext(api) });
    expect(await violations(container)).toEqual([]);
  });

  test("Settings", async () => {
    const { api } = fakeApi();
    const { container } = render(SettingsScreen, { props: { settings: settingsView(), onSettings: () => {}, onDone: () => {} }, context: apiContext(api) });
    expect(await violations(container)).toEqual([]);
  });

  test("Profiles, with and without profiles", async () => {
    for (const profiles of [[profile()], []]) {
      const { api } = fakeApi();
      const { container, unmount } = render(ProfilesScreen, {
        props: { profiles, onProfiles: () => {}, onView: () => {}, onDone: () => {} },
        context: apiContext(api),
      });
      expect(await violations(container)).toEqual([]);
      unmount();
    }
  });
});
```

`ui/src/lib/ui/ui.test.ts`:

```ts
  test("ScreenHeader takes focus, so a new screen is announced by its title", () => {
    render(ScreenHeader, { props: { title: "Summary" } });
    expect(document.activeElement).toBe(screen.getByRole("heading", { name: "Summary" }));
  });
```

`JobProgress.test.ts`: the phase is in a polite live region (`container.querySelector('[aria-live="polite"]')?.textContent` contains "Copying & verifying"); the finished list's viewport has `tabindex="0"`.

- [ ] **Step 2: Run** → RED (on-accent on accent-strong missing; focus; live region; tabindex; and any axe violations found).

- [ ] **Step 3: Implement**
- `app.css`: `--accent-strong: #2f6fe0;` (white on it 4.7:1) and `--on-accent: #ffffff;`. `Button.svelte` `.primary` and `SegmentedControl.svelte` `label.on` use `var(--accent-strong)` for the background (hover: `color-mix(in srgb, var(--accent-strong) 88%, white)`); links, borders, focus rings and progress bars keep `--accent`.
- `ScreenHeader.svelte`: `let heading: HTMLHeadingElement`; `<h1 bind:this={heading} tabindex="-1">`; `onMount(() => heading.focus())`; style `h1:focus { outline: none; }` (a heading isn't a control).
- `JobProgress.svelte`: `<p class="visually-hidden" aria-live="polite">{phase}</p>` inside the content.
- `FinishedList.svelte`: `tabindex="0"` on the viewport.
- Fix each axe violation the new test finds with the smallest change (a missing name, a nested interactive, a landmark), ledgering each.

- [ ] **Step 4: Run** everything → PASS.

- [ ] **Step 5: Commit** `feat(ui): accessibility pass: contrast, focus, announcements, axe checks` (Refs: #20).

---

### Task 6: Docs, checklist, and the manual check

**Files:** Modify `docs/testing/macos-app-checklist.md`, `docs/design/design-system.md` (accent-strong, Notice `announce`), `README.md` (shortcuts), `docs/superpowers/plans/2026-09-26-v1-roadmap.md` (after the check)

- [ ] **Step 1: Checklist** — append:

```markdown
18. **Notification.** Start a copy, switch to another app: when it ends, a notification says
    the result. With "Notify when a copy finishes" off, none. With the window in front, none.
19. **Eject.** After copying from a card, "Eject <card>" ejects it and says so. Open a file
    from the card in QuickTime and try again: the reason is shown, the card stays mounted.
    With a removable destination, "Safe to eject <drive>" shows.
20. **Keyboard.** ⌘O, ⌘D, ⌘↩ from the File menu (greyed out when they don't apply); Space
    pauses and resumes; ⌘. asks to cancel; Esc leaves Settings and Profiles.
21. **VoiceOver.** With VoiceOver on (⌘F5), do a whole copy with the keyboard: every control
    is read with its name, each new screen reads its title, and the end is announced.
```

- [ ] **Step 2: Docs** — design guide: `accent-strong` for filled accent backgrounds; `Notice` `announce={false}` for static notes. README: a "Keyboard" line (⌘O, ⌘D, ⌘↩, ⌘., Space, Esc, ⌘,).

- [ ] **Step 3: Full check** (all commands in Global Constraints) and commit `docs: checklist and guide for notifications, Eject and shortcuts` (Refs: #20).

- [ ] **Step 4: Manual check (the user)** — build the dmg, ask the user to run items 18–21. Fix findings test-first.

- [ ] **Step 5: Roadmap, push, PR (ask first)** — mark 3b-2 `Done (#20)`, push, PR with `Closes #20`, rebase-merge when the user agrees; then the 0.4.0 release PR is the user's call.
