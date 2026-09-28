import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
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
  test("a mirror: its title, then what it archives", () => {
    const { api } = fakeApi();
    const { rerender } = render(JobProgress, {
      props: { progress: progressView(), title: "Mirroring" },
      context: apiContext(api),
    });
    screen.getByRole("heading", { name: "Mirroring" });
    void rerender({ progress: progressView({ phase: "removing", removing: 5, archiving: true }), title: "Mirroring" });
    return waitFor(() => screen.getByText("Archiving 5 files"));
  });

  test("phase, bars, the whole job's percent and file counts", () => {
    show(progressView({ copiedBytes: 148_200_000_000, verifiedBytes: 141_000_000_000, filesDone: 902 }));
    screen.getByRole("heading", { name: "Copying & verifying" });
    expect(screen.getAllByRole("progressbar", { name: /Copied|Verified/ })).toHaveLength(2);
    screen.getByText("148.2 GB of 212.4 GB");
    // Copying and verifying are half of the job each.
    within(screen.getByRole("region", { name: "Progress" })).getByText("68.1 %");
    screen.getByText("902 / 1,284 files");
  });

  test("time left and speed wait for real numbers instead of showing dashes", async () => {
    const { rerender } = show(progressView());
    screen.getByText("Estimating…");
    expect(screen.queryByText(/—|ETA/)).toBeNull();
    await rerender({ progress: progressView({ elapsedMs: 1000, copiedBytes: 1_000_000_000 }) });
    screen.getByText("7:04");
    screen.getByTitle("Time left");
    screen.getByText("1.0 GB of 212.4 GB · 1.0 GB/s");
    expect(screen.queryByText("Estimating…")).toBeNull();
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
    screen.getByText("5.1 GB of 8.4 GB");
    screen.getByRole("progressbar", { name: "A001C014.mov" });
    screen.getByText("+ 12 small files");
    screen.getByText("18.0 MB of 41.0 MB");
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

  test("a failed file's reason takes the checksum's place, so it can be read", async () => {
    const { api } = fakeApi();
    api.finishedPage.mockResolvedValue([row(0, { status: "failed", hash: null, reason: "Cancelled" })]);
    render(JobProgress, { props: { progress: progressView({ filesDone: 1, filesFailed: 1 }) }, context: apiContext(api) });
    const item = await screen.findByRole("listitem");
    await waitFor(() => within(item).getByText("✗ Failed — Cancelled"));
    const status = within(item).getByText("✗ Failed — Cancelled");
    expect(status.classList.contains("wide")).toBe(true);
    expect(within(item).queryByText("—")).toBeNull();
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

  test("the phase is announced, and the finished list can be scrolled from the keyboard", () => {
    const { container } = show(progressView());
    expect(container.querySelector('[aria-live="polite"]')?.textContent).toContain("Copying & verifying");
    expect(container.querySelector(".viewport")?.getAttribute("tabindex")).toBe("0");
  });

  test("Space scrolls the focused file list, and Shift+Space doesn't pause", async () => {
    const { api, container } = show(progressView());
    await fireEvent.keyDown(container.querySelector(".viewport")!, { key: " " });
    await fireEvent.keyDown(window, { key: " ", shiftKey: true });
    expect(api.pauseJob).not.toHaveBeenCalled();
  });

  test("the file list has column headers, and fits a short list", () => {
    const { container } = show(progressView({ filesDone: 4 }));
    const finished = within(screen.getByRole("region", { name: "Finished" }));
    for (const name of ["File", "Size", "Time", "Speed", "Checksum", "Status"]) finished.getByText(name);
    expect((container.querySelector(".viewport") as HTMLElement).style.height).toBe("112px");
  });

  test("verifying a file is a new row, so its bar never runs backwards", async () => {
    const file = { id: 7, name: "C0007.MP4", path: "CLIP/C0007.MP4", size: 8_000_000_000 };
    const { rerender } = show(progressView({ active: [{ ...file, verifying: false, bytesDone: 8_000_000_000 }] }));
    const copying = screen.getByRole("progressbar", { name: "C0007.MP4" });
    await rerender({ progress: progressView({ active: [{ ...file, verifying: true, bytesDone: 800_000_000 }] }) });
    const verifying = screen.getByRole("progressbar", { name: "C0007.MP4" });
    expect(verifying).not.toBe(copying);
    expect(verifying.getAttribute("aria-valuenow")).toBe("10");
  });

  test("in a queue: Job n of m, and Cancel stops the queue", async () => {
    const { api } = fakeApi();
    render(JobProgress, { props: { progress: progressView(), queue: { index: 1, count: 3 } }, context: apiContext(api) });
    screen.getByText("Job 2 of 3");
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalledWith(expect.any(String), "Stop copying and stop the queue?"));
  });
});
