import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { FinishedRow, ProgressView } from "../lib/bindings";
import { fakeApi, progressView } from "../test/fake-api";
import JobProgress from "./JobProgress.svelte";

function show(progress: ProgressView) {
  const { api } = fakeApi();
  const result = render(JobProgress, { props: { progress }, context: apiContext(api) });
  return { api, ...result };
}

function row(i: number, over: Partial<FinishedRow> = {}): FinishedRow {
  return {
    id: i,
    path: `DCIM/C${i}.mov`,
    finalPath: `DCIM/C${i}.mov`,
    size: 8_100_000_000,
    millis: 6_900,
    hash: "9f3a07c1d4e2c21e",
    status: "verified",
    reason: null,
    ...over,
  };
}

describe("JobProgress", () => {
  test("phase, bars and file counts", () => {
    show(progressView({ copiedBytes: 148_200_000_000, verifiedBytes: 141_000_000_000, filesDone: 902 }));
    screen.getByRole("heading", { name: "Copying & verifying" });
    expect(screen.getAllByRole("progressbar")).toHaveLength(2);
    screen.getByText(/148\.2 GB \/ 212\.4 GB · 69\.8 %/);
    screen.getByText("902 / 1,284 files");
  });

  test("speed and ETA need two updates, then follow the progress", async () => {
    const { rerender } = show(progressView());
    expect(screen.getAllByText(/ETA —/)).toHaveLength(2);
    await rerender({ progress: progressView({ elapsedMs: 1000, copiedBytes: 1_000_000_000 }) });
    screen.getByText(/1\.0 GB\/s \(avg 1\.0 GB\/s\) · ETA 3:31/);
  });

  test("plain Copy shows one bar", () => {
    show(progressView({ verify: false }));
    screen.getByRole("heading", { name: "Copying" });
    expect(screen.getAllByRole("progressbar")).toHaveLength(1);
  });

  test("pause and resume", async () => {
    const { api, rerender } = show(progressView());
    await fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(api.pauseJob).toHaveBeenCalled();
    await rerender({ progress: progressView({ paused: true }) });
    screen.getByRole("heading", { name: "Paused" });
    await fireEvent.click(screen.getByRole("button", { name: "Resume" }));
    expect(api.resumeJob).toHaveBeenCalled();
  });

  test("with the checksum file off, Cancel doesn't mention one", async () => {
    const { api } = fakeApi();
    render(JobProgress, { props: { progress: progressView(), checksumFile: false }, context: apiContext(api) });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() =>
      expect(api.confirm).toHaveBeenCalledWith("Files already copied stay; the file in progress is removed.", "Stop copying?"),
    );
  });

  test("cancel asks first and only stops when confirmed", async () => {
    const { api } = show(progressView());
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalledTimes(1));
    expect(api.cancelJob).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(api.cancelJob).toHaveBeenCalled());
    expect(api.confirm.mock.calls[0][0]).toContain("the file in progress is removed");
  });

  test("big files get a row, small ones are grouped", () => {
    show(
      progressView({
        active: [
          { id: 1, name: "A001C014.mov", path: "DCIM/A001C014.mov", verifying: false, size: 8_400_000_000, bytesDone: 5_100_000_000 },
        ],
        smallFiles: { count: 12, size: 41_000_000, bytesDone: 18_000_000 },
      }),
    );
    screen.getByText("A001C014.mov");
    screen.getByText("+ 12 small files");
  });

  test("a fatal error shows a banner", () => {
    show(progressView({ phase: "done", fatal: "The source is no longer available; was it disconnected?" }));
    expect(screen.getByRole("alert").textContent).toContain("Stopped: The source is no longer available");
  });

  test("the finished list only renders the visible rows and fetches their page", async () => {
    const { api, container } = show(progressView({ filesDone: 10_000 }));
    api.finishedPage.mockImplementation((offset: number, limit: number) =>
      Promise.resolve(Array.from({ length: limit }, (_, i) => row(offset + i))),
    );
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledWith(0, 100, false));
    expect(screen.getAllByRole("listitem").length).toBeLessThan(25);
    const viewport = container.querySelector(".viewport") as HTMLElement;
    viewport.scrollTop = 5000 * 28;
    await fireEvent.scroll(viewport);
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledWith(4900, 100, false));
  });

  test("Failed only goes back to the top", async () => {
    const { container } = show(progressView({ filesDone: 10_000, filesFailed: 2 }));
    const viewport = container.querySelector(".viewport") as HTMLElement;
    viewport.scrollTop = 5000 * 28;
    await fireEvent.scroll(viewport);
    await fireEvent.click(screen.getByLabelText("Failed only"));
    expect(viewport.scrollTop).toBe(0);
  });

  test("a short page is asked for again once per update, not in a loop", async () => {
    const { api, rerender } = show(progressView({ filesDone: 5, elapsedMs: 500 }));
    api.finishedPage.mockImplementation(() => Promise.resolve([row(0), row(1), row(2)]));
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 50));
    expect(api.finishedPage).toHaveBeenCalledTimes(1);
    await rerender({ progress: progressView({ filesDone: 5, elapsedMs: 1000 }) });
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledTimes(2));
  });

  test("Failed only asks for failed rows", async () => {
    const { api } = show(progressView({ filesDone: 10, filesFailed: 2 }));
    await fireEvent.click(screen.getByLabelText("Failed only"));
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledWith(0, 100, true));
  });
});
