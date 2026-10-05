import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { raw } from "../test/fake-api";
import en from "../locales/en.json";
import { setLocale } from "../lib/i18n";
import { apiContext } from "../lib/api";
import type { FinishedRow, ProgressView } from "../lib/bindings";
import { fakeApi, progressView } from "../test/fake-api";
import JobProgress from "./JobProgress.svelte";
import { helpOf, hintOf } from "../test/hint";

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
    hash: "06b05ab6733a618578af5f94892f3950",
    status: "verified",
    reason: null,
    ...over,
  };
}

describe("JobProgress", () => {
  test("a mirror: its title, then what it archives", () => {
    const { api } = fakeApi();
    const { rerender } = render(JobProgress, {
      props: { progress: progressView(), mirror: true },
      context: apiContext(api),
    });
    screen.getByRole("heading", { name: "Mirroring" });
    void rerender({ progress: progressView({ phase: "removing", removing: 5, archiving: true }), mirror: true });
    return waitFor(() => screen.getByText("Archiving 5 files"));
  });

  test("phase, bars, the whole job's percent and file counts", () => {
    show(progressView({ copiedBytes: 148_200_000_000, verifiedBytes: 141_000_000_000, filesDone: 902 }));
    screen.getByRole("heading", { name: "Copying & verifying" });
    expect(screen.getAllByRole("progressbar", { name: /Copied|Verified/ })).toHaveLength(2);
    screen.getByText("148.2 GB of 212.4 GB");
    // Copying and verifying are half of the job each.
    within(screen.getByRole("region", { name: "Progress" })).getByText("68.1%");
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
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    const dialog = screen.getByRole("dialog", { name: "Cancel this job?" });
    within(dialog).getByText("Files already copied stay; the file in progress is removed.");
  });

  test("Cancel asks in a dialog: Continue keeps copying", async () => {
    const { api } = show(progressView());
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    const dialog = screen.getByRole("dialog", { name: "Cancel this job?" });
    within(dialog).getByText(/the file in progress is removed/);
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Continue" }));
    await fireEvent.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.cancelJob).not.toHaveBeenCalled();
  });

  test("Stop keeps the files already copied unless asked to remove them", async () => {
    const { api } = show(progressView());
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    let dialog = screen.getByRole("dialog");
    const remove = within(dialog).getByRole("checkbox", { name: "Also remove the files already copied" });
    expect(remove).toHaveProperty("checked", false);
    await fireEvent.click(within(dialog).getByRole("button", { name: "Cancel job" }));
    expect(api.cancelJob).toHaveBeenLastCalledWith(false);
    expect(screen.queryByRole("dialog")).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    dialog = screen.getByRole("dialog");
    await fireEvent.click(within(dialog).getByRole("checkbox", { name: "Also remove the files already copied" }));
    within(dialog).getByText("The file in progress and the files already copied are removed.");
    await fireEvent.click(within(dialog).getByRole("button", { name: "Cancel job" }));
    expect(api.cancelJob).toHaveBeenLastCalledWith(true);
  });

  test("Esc closes the dialog and keeps copying", async () => {
    const { api } = show(progressView());
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.cancelJob).not.toHaveBeenCalled();
  });

  test("while a cancelled job removes what it copied, it says so, with Pause and Cancel off", () => {
    const { api } = fakeApi();
    render(JobProgress, {
      props: { progress: progressView({ phase: "removing", undoing: true }) },
      context: apiContext(api),
    });
    screen.getByText("Putting the destination back as it was…");
    expect(screen.queryByText(/Deleting/)).toBeNull();
    expect(screen.getByRole("button", { name: "Pause" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Cancel…" })).toHaveProperty("disabled", true);
  });

  test("while a mirror archives, Pause and Cancel are off", async () => {
    const { api } = fakeApi();
    const { component } = render(JobProgress, {
      props: { progress: progressView({ phase: "removing", removing: 3, archiving: true }), mirror: true },
      context: apiContext(api),
    });
    expect(screen.getByRole("button", { name: "Pause" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Cancel…" })).toHaveProperty("disabled", true);
    component.cancel(); // ⌘. from the menu
    await Promise.resolve();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("the question closes when a mirror starts removing", async () => {
    const { api } = fakeApi();
    const { rerender } = render(JobProgress, { props: { progress: progressView() }, context: apiContext(api) });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    screen.getByRole("dialog");
    await rerender({ progress: progressView({ phase: "removing", removing: 2, archiving: true }) });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("while a queue job is checked, nothing copied can be removed", async () => {
    const { api } = fakeApi();
    render(JobProgress, {
      props: { progress: progressView(), checking: true, queue: { index: 0, count: 2 } },
      context: apiContext(api),
    });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    expect(screen.queryByRole("checkbox", { name: "Also remove the files already copied" })).toBeNull();
    expect(screen.queryByText(/elapsed/)).toBeNull();
  });

  test("a check: Verifying, one Verified bar, and nothing to remove on Cancel", async () => {
    const { api } = fakeApi();
    render(JobProgress, {
      props: { progress: progressView({ verifiedBytes: 50, totalBytes: 200, copiedBytes: 0 }), check: true },
      context: apiContext(api),
    });
    screen.getByRole("heading", { name: "Verifying" });
    expect(screen.getAllByRole("progressbar", { name: /Copied|Verified|Checked/ }).map((b) => b.getAttribute("aria-label"))).toEqual([
      "Verified",
    ]);
    within(screen.getByRole("region", { name: "Progress" })).getByText("25.0%");
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    screen.getByRole("dialog", { name: "Cancel this job?" });
    expect(screen.queryByRole("checkbox", { name: "Also remove the files already copied" })).toBeNull();
  });

  test("a mirror asks Stop mirroring?", async () => {
    const { api } = fakeApi();
    render(JobProgress, { props: { progress: progressView(), mirror: true }, context: apiContext(api) });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    screen.getByRole("dialog", { name: "Cancel this job?" });
  });

  test("#172 review: Verify's small files are read, not copied", () => {
    const { api } = fakeApi();
    render(JobProgress, {
      props: { progress: progressView({ smallFiles: { done: 1, total: 4 } }), check: true },
      context: apiContext(api),
    });
    expect(hintOf(screen.getByText("Small files"))).toBe("Files under 8 MB, read several at once and counted together.");
  });

  test("big files get a row, small ones one steady row counted in files", () => {
    show(
      progressView({
        active: [
          { id: 1, name: "A001C014.mov", path: "DCIM/A001C014.mov", verifying: false, size: 8_400_000_000, bytesDone: 5_100_000_000 },
        ],
        smallFiles: { done: 1234, total: 5000 },
      }),
    );
    screen.getByText("A001C014.mov");
    screen.getByText("5.1 GB of 8.4 GB");
    screen.getByRole("progressbar", { name: "A001C014.mov" });
    expect(hintOf(screen.getByText("Small files"))).toMatch(/under 8 MB/);
    screen.getByText("1,234 of 5,000");
    expect(screen.getByRole("progressbar", { name: "Small files" }).getAttribute("aria-valuenow")).toBe("25");
  });

  test("recording ASC MHL shows its own bar (#154)", () => {
    show(progressView({ recording: { bytes: 1_000_000_000, total: 4_000_000_000 } }));
    screen.getByText("Recording ASC MHL");
    screen.getByText("1.0 GB of 4.0 GB");
    expect(screen.getByRole("progressbar", { name: "Recording ASC MHL" }).getAttribute("aria-valuenow")).toBe("25");
  });

  test("a fatal error shows a banner", () => {
    show(progressView({ phase: "done", fatal: raw("Source not found.") }));
    expect(screen.getByRole("alert").textContent).toContain("Stopped: Source not found.");
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

  test("a page that failed to load is asked for again, not left blank (#117)", async () => {
    const { api } = fakeApi();
    api.finishedPage.mockRejectedValueOnce(new Error("busy"));
    api.finishedPage.mockImplementation((offset: number, limit: number) =>
      Promise.resolve(Array.from({ length: limit }, (_, i) => row(offset + i))),
    );
    const { container } = render(JobProgress, {
      props: { progress: progressView({ filesDone: 1000 }) },
      context: apiContext(api),
    });
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledTimes(1));
    await new Promise((r) => setTimeout(r, 0)); // the failure is in
    const viewport = container.querySelector(".viewport") as HTMLElement;
    viewport.scrollTop = 28 * 3; // still the first page
    await fireEvent.scroll(viewport);
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledTimes(2));
    await screen.findByText("DCIM/C0.mov");
  });

  test("a failed page is asked for again by itself, a few times (#117)", async () => {
    const { api } = fakeApi();
    api.finishedPage.mockRejectedValueOnce(new Error("busy"));
    api.finishedPage.mockImplementation((offset: number, limit: number) =>
      Promise.resolve(Array.from({ length: Math.min(limit, 3) }, (_, i) => row(offset + i))),
    );
    render(JobProgress, {
      props: { progress: progressView({ phase: "done", filesDone: 3 }) },
      context: apiContext(api),
    });
    await screen.findByText("DCIM/C0.mov", undefined, { timeout: 3000 });
    expect(api.finishedPage).toHaveBeenCalledTimes(2);
  });

  test("the Status column holds a short word; a failure's reason is on hover", async () => {
    const { api } = fakeApi();
    api.finishedPage.mockResolvedValue([
      row(0, { status: "failed", hash: null, reason: raw("The source file changed while it was copied") }),
      row(1, { status: "cancelled", hash: null, reason: null }),
    ]);
    render(JobProgress, { props: { progress: progressView({ filesDone: 2, filesFailed: 1 }) }, context: apiContext(api) });
    await waitFor(() => expect(screen.getAllByRole("listitem")[1].textContent).toContain("Cancelled"));
    const [failed, cancelled] = screen.getAllByRole("listitem");
    const status = within(failed).getByText("✗ Failed");
    expect(status.getAttribute("title")).toBe("The source file changed while it was copied");
    within(failed).getByText("—"); // no checksum, in the Checksum column
    within(cancelled).getByText("Cancelled");
  });

  test("Failed only goes back to the top", async () => {
    const { container } = show(progressView({ filesDone: 10_000, filesFailed: 2 }));
    const viewport = container.querySelector(".viewport") as HTMLElement;
    viewport.scrollTop = 5000 * 28;
    await fireEvent.scroll(viewport);
    await fireEvent.click(screen.getByLabelText("Failed only"));
    expect(viewport.scrollTop).toBe(0);
  });

  test("#178: a finished row's XXH128 is whole in its tooltip (a narrow window shortens it)", async () => {
    const { api } = fakeApi();
    api.finishedPage.mockResolvedValue([row(0)]);
    render(JobProgress, { props: { progress: progressView({ filesDone: 1 }) }, context: apiContext(api) });
    const hash = await screen.findByText("06b05ab6733a618578af5f94892f3950");
    expect(hash.getAttribute("title")).toBe("06b05ab6733a618578af5f94892f3950");
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

  test("a file's bar is as wide as its progress in a language with a decimal comma", () => {
    setLocale("de", en);
    try {
      const file = { id: 7, name: "C0007.MP4", path: "CLIP/C0007.MP4", size: 8_000_000_000, verifying: false };
      show(progressView({ active: [{ ...file, bytesDone: 2_020_000_000 }] }));
      const fill = screen.getByRole("progressbar", { name: "C0007.MP4" }).querySelector(".fill") as HTMLElement;
      expect(fill.style.width).toBe("25.25%");
    } finally {
      setLocale("en");
    }
  });

  // #208: the screen says which job runs, under its title.
  const studio = { name: "Studio", source: "/Users/beli/Studio", destination: "/Volumes/Media/Mirror/Studio" };

  test("the job is named under the title: a mirror's name, origin and destination (#208)", () => {
    const { api } = fakeApi();
    render(JobProgress, { props: { progress: progressView(), mirror: true, job: studio }, context: apiContext(api) });
    const job = screen.getByRole("group", { name: "Job" });
    expect(job.textContent?.replace(/\s+/g, " ").trim()).toBe("Studio · /Users/beli/Studio → /Volumes/Media/Mirror/Studio");
    expect(within(job).getByText("/Users/beli/Studio").closest("[title]")?.getAttribute("title")).toBe("/Users/beli/Studio");
  });

  test("a verify names its directory, with no arrow (#208)", () => {
    const { api } = fakeApi();
    const job = { name: null, source: "/Volumes/Backup/Day01", destination: null };
    render(JobProgress, { props: { progress: progressView(), check: true, job }, context: apiContext(api) });
    expect(screen.getByRole("group", { name: "Job" }).textContent?.trim()).toBe("/Volumes/Backup/Day01");
  });

  test("in a queue: Job n of m, and Cancel stops the queue", async () => {
    const { api } = fakeApi();
    render(JobProgress, { props: { progress: progressView(), queue: { index: 1, count: 3 }, job: studio }, context: apiContext(api) });
    // #208: the job's place starts the line that names it, not the action bar.
    expect(screen.getByRole("group", { name: "Job" }).textContent?.replace(/\s+/g, " ").trim()).toMatch(/^Job 2 of 3 · Studio · /);
    expect(within(screen.getByRole("group", { name: "Actions" })).queryByText(/Job 2 of 3/)).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    screen.getByRole("dialog", { name: "Cancel this job and stop the queue?" });
    expect(api.confirm).not.toHaveBeenCalled();
  });

  test("in a queue, Cancel names what the job does", async () => {
    const { api } = fakeApi();
    const { rerender } = render(JobProgress, {
      props: { progress: progressView(), queue: { index: 0, count: 2 }, check: true },
      context: apiContext(api),
    });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    screen.getByRole("dialog", { name: "Cancel this job and stop the queue?" });
    await fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    await rerender({ check: false, mirror: true });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    screen.getByRole("dialog", { name: "Cancel this job and stop the queue?" });
  });

  test("while a queue job is checked, Cancel stops the queue and nothing of the job was written", async () => {
    const { api } = fakeApi();
    render(JobProgress, {
      props: { progress: progressView(), checking: true, queue: { index: 0, count: 2 } },
      context: apiContext(api),
    });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    const dialog = screen.getByRole("dialog", { name: "Stop the queue?" });
    within(dialog).getByText("This job hasn’t started yet: nothing was written for it.");
  });

  test("a mirror's Cancel says files deleted in the origin stay, and no checksum file", async () => {
    const { api } = fakeApi();
    render(JobProgress, { props: { progress: progressView(), mirror: true }, context: apiContext(api) });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    const dialog = screen.getByRole("dialog", { name: "Cancel this job?" });
    within(dialog).getByText(
      "Files already copied stay; the file in progress is removed. Files deleted in the origin are left in the destination.",
    );
  });
});

describe("JobProgress: help on the buttons", () => {
  test("Pause, Resume and Cancel say what they do and their keys", async () => {
    const { rerender } = show(progressView());
    expect(helpOf(screen.getByRole("button", { name: "Pause" }))).toBe(
      "Stops reading and writing until you resume (Space).",
    );
    expect(helpOf(screen.getByRole("button", { name: "Cancel…" }))).toBe("Asks first, then cancels the job (⌘.).");
    await rerender({ progress: progressView({ paused: true }) });
    expect(helpOf(screen.getByRole("button", { name: "Resume" }))).toBe("Carries on from where it paused (Space).");
  });

  test("a check only reads; a queue job's Cancel stops the queue too", () => {
    const { api } = fakeApi();
    render(JobProgress, { props: { progress: progressView(), check: true, queue: { index: 0, count: 2 } }, context: apiContext(api) });
    expect(helpOf(screen.getByRole("button", { name: "Pause" }))).toBe("Stops reading until you resume (Space).");
    expect(helpOf(screen.getByRole("button", { name: "Cancel…" }))).toBe(
      "Asks first, then cancels this job and stops the queue (⌘.).",
    );
  });

  test("done: the buttons are off and give no help", () => {
    show(progressView({ phase: "done" }));
    expect(helpOf(screen.getByRole("button", { name: "Pause" }))).toBeNull();
    expect(helpOf(screen.getByRole("button", { name: "Cancel…" }))).toBeNull();
  });
});

describe("JobProgress before its first figures (#117)", () => {
  test("a total not known yet shows nothing done, not 100%", () => {
    const { container } = show(progressView({ totalBytes: 0, copiedBytes: 0, verifiedBytes: 0, phase: "copying" }));
    expect(screen.getByText(/^0(\.0)?%$/)).toBeTruthy();
    for (const bar of container.querySelectorAll("[role=progressbar]")) {
      expect(bar.getAttribute("aria-valuenow")).toBe("0");
    }
  });
});
