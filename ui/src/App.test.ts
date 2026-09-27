import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
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
} from "./test/fake-api";

function app(view = readyView()) {
  const { api, state } = fakeApi(view);
  render(App, { props: { api } });
  return { api, state };
}

const startButton = () => screen.findByRole("button", { name: /^Copy & verify 1,284 files/ });

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
    await screen.findByRole("button", { name: /^Copy 1,284 files/ });
    screen.getByRole("option", { name: "Sony FX3" });
  });

  test("Settings opens from the gear and from the menu, and Done goes back", async () => {
    const { state } = app();
    await fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    await screen.findByRole("heading", { name: "Settings" });
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
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
    await fireEvent.click(await screen.findByLabelText("Copy"));
    await waitFor(() => expect(api.setMode).toHaveBeenCalledWith(false));
  });

  test("with the checksum file off, quitting during a copy doesn't mention one", async () => {
    const { api, state } = fakeApi(readyView());
    state.start = startView({ session: readyView(), settings: settingsView({ writeChecksumFile: false }) });
    render(App, { props: { api } });
    await waitFor(() => expect(state.close).not.toBeNull());
    await screen.findByRole("button", { name: /^Copy & verify 1,284 files/ });
    api.jobRunning.mockResolvedValue(true);
    await state.close!(() => {});
    expect(api.confirm).toHaveBeenCalledWith(
      "Files already copied stay; the file in progress is removed.",
      "Stop copying and quit?",
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
    await screen.findByRole("button", { name: /^Copy & verify 1,284 files/ });
  });
});
