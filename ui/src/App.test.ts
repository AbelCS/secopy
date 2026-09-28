import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import App from "./App.svelte";
import {
  fakeApi,
  profile,
  progressView,
  readyView,
  sessionView,
  settingsView,
  startView,
  summaryView,
  queuedJob,
  queueView,
  mirrorPreview,
} from "./test/fake-api";

function app(view = readyView()) {
  const { api, state } = fakeApi(view);
  render(App, { props: { api } });
  return { api, state };
}

const startButton = () => screen.findByRole("button", { name: "Start copy" });

describe("App", () => {
  test("the main screen is New copy", () => {
    app(sessionView());
    expect(screen.getByRole("heading", { level: 1, name: "New copy" })).toBeTruthy();
  });

  test("a job goes from setup to progress to the summary", async () => {
    const { api, state } = app();
    await fireEvent.click(await startButton());
    expect(api.startJob).toHaveBeenCalledWith(true, expect.any(Function));
    await screen.findByRole("heading", { name: "Copying & verifying" });
    state.progress!(progressView({ copiedBytes: 1000, elapsedMs: 500 }));
    state.progress!(progressView({ phase: "done" }));
    await screen.findByText(/All 1,284 files copied and verified/);
  });

  test("a job that can't start goes back to setup with the reason", async () => {
    const { api } = app();
    api.startJob.mockRejectedValueOnce(new Error("Nothing to copy, or something blocks the copy."));
    await fireEvent.click(await startButton());
    await screen.findByText("Nothing to copy, or something blocks the copy.");
    await startButton();
  });

  test("Retry failed goes back to setup with the failed files", async () => {
    const { api, state } = app();
    api.jobSummary.mockResolvedValue(summaryView({ outcome: "failures", failed: 2, verified: 1282 }));
    await fireEvent.click(await startButton());
    state.progress!(progressView({ phase: "done" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Retry failed" }));
    await waitFor(() => expect(api.retryFailed).toHaveBeenCalled());
    await startButton();
  });

  test("New copy clears the source and keeps the destination", async () => {
    const { api, state } = app();
    await fireEvent.click(await startButton());
    state.progress!(progressView({ phase: "done" }));
    await fireEvent.click(await screen.findByRole("button", { name: "New copy" }));
    await waitFor(() => expect(api.clearSource).toHaveBeenCalled());
  });

  test("closing during a copy asks, and stays open unless confirmed", async () => {
    const { api, state } = app();
    await waitFor(() => expect(state.close).not.toBeNull());
    api.jobRunning.mockResolvedValue(true);
    api.confirm.mockResolvedValueOnce(false);
    let prevented = false;
    await state.close!(() => (prevented = true));
    expect(prevented).toBe(true);
    api.confirm.mockResolvedValueOnce(true);
    prevented = false;
    await state.close!(() => (prevented = true));
    expect(prevented).toBe(false);
  });

  test("closing when idle doesn't ask", async () => {
    const { api, state } = app();
    await waitFor(() => expect(state.close).not.toBeNull());
    await state.close!(() => {});
    expect(api.confirm).not.toHaveBeenCalled();
  });
  test("the start-up load fills the window", async () => {
    const { api, state } = fakeApi(readyView());
    state.start = startView({ session: readyView(), verify: false, profiles: [profile()] });
    render(App, { props: { api } });
    await screen.findByRole("button", { name: "Start copy" });
    expect(screen.getByRole("radio", { name: "Copy" })).toHaveProperty("checked", true);
    screen.getByRole("option", { name: "Sony FX3" });
  });

  test("the kinds of job are tabs at the top; the Queue and Settings sit apart on the right", async () => {
    const { state } = app();
    await startButton();
    const tabs = screen.getByRole("navigation", { name: "Sections" });
    for (const name of ["Copy", "Mirror", "Verify"]) within(tabs).getByRole("button", { name });
    expect(within(tabs).queryByRole("button", { name: /^Queue/ })).toBeNull();
    const inBar = () => screen.getByRole("button", { name: "Settings" }).closest(".tabbar");
    expect(inBar()).not.toBeNull();
    expect(screen.getByRole("button", { name: /^Queue/ }).closest(".tabbar")).not.toBeNull();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-queue");
    await screen.findByRole("heading", { level: 1, name: "Queue" });
    expect(inBar()).not.toBeNull();
  });

  test("Settings opens from the gear and from the menu, and Cancel goes back", async () => {
    const { state } = app();
    await fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    await screen.findByRole("heading", { name: "Settings" });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await startButton();
    await waitFor(() => expect(state.openSettings).not.toBeNull());
    state.openSettings!();
    await screen.findByRole("heading", { name: "Settings" });
  });

  test("the menu doesn't open Settings during a copy", async () => {
    const { state } = app();
    await fireEvent.click(await startButton());
    await screen.findByRole("heading", { name: "Copying & verifying" });
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
    state.openSettings!();
    expect(screen.queryByRole("heading", { name: "Settings" })).toBeNull();
  });

  test("a saved file that couldn't be read is shown once", async () => {
    const { api, state } = fakeApi(readyView());
    state.start = startView({ session: readyView(), warnings: ["settings.json couldn't be read (…)."] });
    render(App, { props: { api } });
    await screen.findByText("settings.json couldn't be read (…).");
    await fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("settings.json couldn't be read (…).")).toBeNull();
  });

  test("the mode is remembered", async () => {
    const { api } = app();
    await fireEvent.click(await screen.findByRole("radio", { name: "Copy" }));
    await waitFor(() => expect(api.setMode).toHaveBeenCalledWith(false));
  });

  test("with the checksum file off, quitting during a copy doesn't mention one", async () => {
    const { api, state } = fakeApi(readyView());
    state.start = startView({ session: readyView(), settings: settingsView({ writeChecksumFile: false }) });
    render(App, { props: { api } });
    await waitFor(() => expect(state.close).not.toBeNull());
    await screen.findByRole("button", { name: "Start copy" });
    api.jobRunning.mockResolvedValue(true);
    await state.close!(() => {});
    expect(api.confirm).toHaveBeenCalledWith(
      "Files already copied stay; the file in progress is removed.",
      "Stop copying and quit?",
      "Stop copying",
      "Keep copying",
    );
  });

  test("quitting during a check says verifying, and that nothing was changed", async () => {
    const { api, state } = app();
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-verify");
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(await screen.findByRole("button", { name: "Choose…" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Start verify" }));
    await screen.findByRole("heading", { level: 1, name: "Verifying" });
    api.jobRunning.mockResolvedValue(true);
    await state.close!(() => {});
    expect(api.confirm).toHaveBeenCalledWith(
      "Nothing was changed: a check only reads files.",
      "Stop verifying and quit?",
      "Stop verifying",
      "Keep verifying",
    );
  });

  test("quitting during a mirror says mirroring, and what a stopped mirror leaves", async () => {
    const { api, state } = app();
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-mirror");
    await fireEvent.click(await screen.findByRole("button", { name: "Preview…" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run mirror" }));
    await screen.findByRole("heading", { level: 1, name: "Mirroring" });
    api.jobRunning.mockResolvedValue(true);
    await state.close!(() => {});
    expect(api.confirm).toHaveBeenCalledWith(
      "Files already copied stay; the file in progress is removed. Files deleted in the origin are left in the destination.",
      "Stop mirroring and quit?",
      "Stop mirroring",
      "Keep mirroring",
    );
  });

  test("quitting during a queued check says verifying; while a job is checked, the queue", async () => {
    const { api, state } = fakeApi(readyView());
    state.queue = queueView({ jobs: [queuedJob({ kind: "check" })] });
    render(App, { props: { api } });
    await fireEvent.click(await screen.findByRole("button", { name: /^Queue/ }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run queue" }));
    await waitFor(() => expect(state.queueEvent).not.toBeNull());
    api.jobRunning.mockResolvedValue(true);
    state.queueEvent!({ type: "jobChecking", index: 0, count: 1 });
    await screen.findByRole("heading", { level: 1, name: "Checking…" });
    await state.close!(() => {});
    expect(api.confirm).toHaveBeenLastCalledWith(
      "This job hasn't started yet: nothing was written for it.",
      "Stop the queue and quit?",
      "Stop the queue",
      "Continue",
    );
    state.queueEvent!({ type: "jobStarted", index: 0, count: 1, job: queuedJob({ kind: "check" }) });
    await screen.findByRole("heading", { level: 1, name: "Verifying" });
    await state.close!(() => {});
    expect(api.confirm).toHaveBeenLastCalledWith(
      "Nothing was changed: a check only reads files.",
      "Stop verifying and quit?",
      "Stop verifying",
      "Keep verifying",
    );
  });

  test("Manage profiles… opens the Profiles screen, not Settings", async () => {
    const { api, state } = fakeApi(readyView());
    state.start = startView({ session: readyView(), profiles: [profile()] });
    render(App, { props: { api } });
    await screen.findByRole("option", { name: "Sony FX3" });
    await fireEvent.change(screen.getByRole("combobox", { name: "Profile" }), { target: { value: "manage" } });
    await screen.findByRole("heading", { name: "Profiles" });
    expect(screen.queryByRole("heading", { name: "Settings" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await screen.findByRole("button", { name: "Start copy" });
  });
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
    await screen.findByRole("dialog", { name: "Stop copying?" });
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

  test("⌘. is off while a mirror archives or deletes, as Cancel is", async () => {
    const { api, state } = app();
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-mirror");
    await fireEvent.click(await screen.findByRole("button", { name: "Preview…" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run mirror" }));
    await waitFor(() => expect(api.setMenuState).toHaveBeenLastCalledWith(false, false, true));
    state.progress!(progressView({ phase: "removing", removing: 3, archiving: true }));
    await waitFor(() => expect(api.setMenuState).toHaveBeenLastCalledWith(false, false, false));
  });

  test("the last profile is loaded again at start", async () => {
    const { api, state } = fakeApi(sessionView());
    state.start = startView({ session: sessionView(), profiles: [profile()], lastProfile: "fx3" });
    render(App, { props: { api } });
    await waitFor(() => expect(api.selectProfile).toHaveBeenCalledWith("fx3"));
  });

  test("the tabs switch between Copy and Queue, and hide while copying", async () => {
    const { state } = app();
    await startButton();
    await fireEvent.click(screen.getByRole("button", { name: /^Queue/ }));
    await screen.findByRole("heading", { level: 1, name: "Queue" });
    state.menu!("show-copy");
    await screen.findByRole("heading", { level: 1, name: "New copy" });
    await fireEvent.click(await startButton());
    await screen.findByRole("heading", { name: "Copying & verifying" });
    expect(screen.queryByRole("navigation", { name: "Sections" })).toBeNull();
  });

  test("a queue run: progress per job, one notification, then the queue summary", async () => {
    const { api, state } = fakeApi(readyView());
    state.queue = queueView({ jobs: [queuedJob(), queuedJob()] });
    api.windowFocused.mockReturnValue(false);
    render(App, { props: { api } });
    await fireEvent.click(await screen.findByRole("button", { name: /^Queue/ }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run queue" }));
    await waitFor(() => expect(state.queueEvent).not.toBeNull());
    state.queueEvent!({ type: "jobChecking", index: 0, count: 2 });
    state.queueEvent!({ type: "jobStarted", index: 0, count: 2, job: queuedJob() });
    state.queueEvent!({ type: "progress", view: progressView() });
    await screen.findByText(/^Job 1 of 2 · /);
    state.queueEvent!({ type: "done", summary: { complete: 2, count: 2, millis: 1000, results: [
      { job: queuedJob(), result: "complete", reason: null, summary: summaryView() },
      { job: queuedJob(), result: "complete", reason: null, summary: summaryView() },
    ], saveError: null } });
    await screen.findByRole("heading", { name: "Queue done: 2 of 2 jobs complete" });
    expect(api.notify).toHaveBeenCalledTimes(1);
    expect(api.notify).toHaveBeenCalledWith("Queue done: 2 of 2 jobs complete", expect.any(String));
  });

  test("each queue job gets a fresh Copying screen: its own files, speed and time", async () => {
    const { api, state } = fakeApi(readyView());
    state.queue = queueView({ jobs: [queuedJob(), queuedJob()] });
    api.finishedPage.mockResolvedValue([
      { id: 0, path: "A.mov", finalPath: "A.mov", size: 1, millis: 1, hash: "h", status: "verified", reason: null },
    ]);
    render(App, { props: { api } });
    await fireEvent.click(await screen.findByRole("button", { name: /^Queue/ }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run queue" }));
    await waitFor(() => expect(state.queueEvent).not.toBeNull());
    state.queueEvent!({ type: "jobChecking", index: 0, count: 2 });
    state.queueEvent!({ type: "jobStarted", index: 0, count: 2, job: queuedJob() });
    state.queueEvent!({ type: "progress", view: progressView({ filesDone: 1 }) });
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledTimes(1));
    state.queueEvent!({ type: "jobChecking", index: 1, count: 2 });
    state.queueEvent!({ type: "jobStarted", index: 1, count: 2, job: queuedJob() });
    await screen.findByText(/^Job 2 of 2 · /);
    state.queueEvent!({ type: "progress", view: progressView({ filesDone: 1 }) });
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledTimes(2), { timeout: 500 });
  });

  test("a queue job is checked first, then shows its own kind", async () => {
    const { api, state } = fakeApi(readyView());
    state.queue = queueView({ jobs: [queuedJob(), queuedJob({ kind: "mirror", name: "Footage" })] });
    render(App, { props: { api } });
    await fireEvent.click(await screen.findByRole("button", { name: /^Queue/ }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run queue" }));
    await waitFor(() => expect(state.queueEvent).not.toBeNull());
    state.queueEvent!({ type: "jobChecking", index: 1, count: 2 });
    await screen.findByRole("heading", { level: 1, name: "Checking…" });
    expect(screen.getByRole("button", { name: "Pause" })).toHaveProperty("disabled", true);
    state.queueEvent!({ type: "jobStarted", index: 1, count: 2, job: queuedJob({ kind: "mirror", name: "Footage" }) });
    await screen.findByRole("heading", { level: 1, name: "Mirroring" });
  });

  test("a queued mirror's deep check says how far it is", async () => {
    const { api, state } = fakeApi(readyView());
    state.queue = queueView({ jobs: [queuedJob({ kind: "mirror", name: "Footage" })] });
    render(App, { props: { api } });
    await fireEvent.click(await screen.findByRole("button", { name: /^Queue/ }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run queue" }));
    await waitFor(() => expect(state.queueEvent).not.toBeNull());
    state.queueEvent!({ type: "jobChecking", index: 0, count: 1 });
    state.queueEvent!({ type: "compared", index: 0, done: 1, total: 3 });
    await screen.findByText("Comparing contents: 1 of 3 files");
  });

  test("a preview that ends after you left Mirror doesn't pull you back", async () => {
    const { api, state } = app();
    let finish: ((v: ReturnType<typeof mirrorPreview>) => void) | undefined;
    api.previewMirror.mockImplementation(() => new Promise((r) => (finish = r)));
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-mirror");
    await fireEvent.click(await screen.findByRole("button", { name: "Preview…" }));
    state.menu!("show-queue");
    await screen.findByRole("heading", { level: 1, name: "Queue" });
    finish!(mirrorPreview());
    await new Promise((r) => setTimeout(r, 20));
    screen.getByRole("heading", { level: 1, name: "Queue" });
  });

  test("after a queue run, Copy opens New copy, not an older summary", async () => {
    const { api, state } = fakeApi(readyView());
    state.queue = queueView({ jobs: [queuedJob()] });
    render(App, { props: { api } });
    await fireEvent.click(await startButton());
    state.progress!(progressView({ phase: "done" }));
    await screen.findByRole("heading", { level: 1, name: "Summary" });
    state.menu!("show-queue");
    await fireEvent.click(await screen.findByRole("button", { name: "Run queue" }));
    await waitFor(() => expect(state.queueEvent).not.toBeNull());
    state.queueEvent!({ type: "done", summary: { complete: 1, count: 1, millis: 1000, results: [
      { job: queuedJob(), result: "complete", reason: null, summary: summaryView() },
    ], saveError: null } });
    await screen.findByRole("heading", { name: "Queue done: 1 of 1 job complete" });
    state.menu!("show-copy");
    await screen.findByRole("heading", { level: 1, name: "New copy" });
  });

  test("the Verify tab and ⌘3 open Verify; Queue is ⌘4", async () => {
    const { state } = app();
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-verify");
    await screen.findByRole("heading", { level: 1, name: "Verify" });
    state.menu!("show-queue");
    await screen.findByRole("heading", { level: 1, name: "Queue" });
  });

  test("a check runs as Verifying and ends in its summary on the Verify tab", async () => {
    const { api, state } = app();
    api.jobSummary.mockResolvedValue(summaryView({ check: { intact: 2, changed: 0, missing: 0, failed: 0, notChecked: 0, checksumFiles: 1, problems: [] } }));
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-verify");
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(await screen.findByRole("button", { name: "Choose…" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Start verify" }));
    await screen.findByRole("heading", { level: 1, name: "Verifying" });
    state.progress!(progressView({ phase: "done" }));
    await screen.findByText("All 2 files intact");
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    await screen.findByRole("heading", { level: 1, name: "Verify" });
  });

  test("the Mirror tab and ⌘2 open Mirror", async () => {
    const { state } = app();
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-mirror");
    await screen.findByRole("heading", { level: 1, name: "Mirror" });
    screen.getByRole("button", { name: "Mirror" });
  });

  test("a mirror: Preview…, Run mirror, its summary, and Done back to Mirror", async () => {
    const { api, state } = app();
    const mirror = { new: 2, updated: 1, removed: 1, archived: true, removalFailures: [], nothingRemoved: null };
    api.jobSummary.mockResolvedValue(summaryView({ mirror }));
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-mirror");
    await fireEvent.click(await screen.findByRole("button", { name: "Preview…" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run mirror" }));
    await screen.findByRole("heading", { level: 1, name: "Mirroring" });
    expect(api.runMirror).toHaveBeenCalledWith("m1", expect.any(Function));
    state.progress!(progressView({ phase: "done" }));
    await screen.findByText("Mirrored: 2 new, 1 updated, 1 archived");
    // The Copy section doesn't show the mirror's summary.
    state.menu!("show-copy");
    await screen.findByRole("heading", { level: 1, name: "New copy" });
    state.menu!("show-mirror");
    await screen.findByText("Mirrored: 2 new, 1 updated, 1 archived");
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    await screen.findByRole("heading", { level: 1, name: "Mirror" });
  });

  test("a copy after a mirror: Mirror no longer shows the mirror's summary", async () => {
    const { api, state } = app();
    const mirror = { new: 2, updated: 1, removed: 1, archived: true, removalFailures: [], nothingRemoved: null };
    api.jobSummary.mockResolvedValueOnce(summaryView({ mirror }));
    await startButton();
    await waitFor(() => expect(state.menu).not.toBeNull());
    state.menu!("show-mirror");
    await fireEvent.click(await screen.findByRole("button", { name: "Preview…" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Run mirror" }));
    state.progress!(progressView({ phase: "done" }));
    await screen.findByText("Mirrored: 2 new, 1 updated, 1 archived");
    state.menu!("show-copy");
    await fireEvent.click(await startButton());
    state.progress!(progressView({ phase: "done" }));
    await screen.findByRole("heading", { level: 1, name: "Summary" });
    state.menu!("show-mirror");
    await screen.findByRole("heading", { level: 1, name: "Mirror" });
    expect(screen.queryByText("Mirrored: 2 new, 1 updated, 1 archived")).toBeNull();
  });
});
