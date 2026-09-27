import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import App from "./App.svelte";
import { fakeApi, progressView, readyView, sessionView, summaryView } from "./test/fake-api";

function app(view = readyView()) {
  const { api, state } = fakeApi(view);
  render(App, { props: { api } });
  return { api, state };
}

const startButton = () => screen.findByRole("button", { name: /^Copy & verify 1,284 files/ });

describe("App", () => {
  test("shows the app name", () => {
    app(sessionView());
    expect(screen.getByRole("heading", { name: "Secopy" })).toBeTruthy();
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
});
