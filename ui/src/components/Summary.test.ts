import { fireEvent, render, screen, within, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { FinishedRow, SummaryView } from "../lib/bindings";
import { fakeApi, summaryView } from "../test/fake-api";
import Summary from "./Summary.svelte";
import { helpOf, hintOf } from "../test/hint";

function show(summary: SummaryView) {
  const { api } = fakeApi();
  const calls: string[] = [];
  render(Summary, {
    props: { summary, onRetry: () => calls.push("retry"), onNewCopy: () => calls.push("new") },
    context: apiContext(api),
  });
  return { api, calls };
}

describe("Summary", () => {
  test("a mirror: its headline, why nothing was removed, what couldn't be, and Done", async () => {
    const { api } = fakeApi();
    const calls = { done: 0 };
    const row = { id: 0, path: "B/old.mov", finalPath: "B/old.mov", size: 0, millis: 0, hash: null, status: "failed" as const, reason: "Permission denied" };
    render(Summary, {
      props: {
        summary: summaryView({
          outcome: "failures",
          failed: 0,
          mirror: { new: 0, updated: 0, removed: 0, archived: true, removalFailures: [row], nothingRemoved: null },
        }),
        onDone: () => calls.done++,
      },
      context: apiContext(api),
    });
    screen.getByRole("heading", { name: /1 file couldn't be removed/ });
    within(screen.getByRole("region", { name: "Not removed" })).getByText("Permission denied", { exact: false });
    expect(screen.queryByRole("button", { name: "New copy" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(calls.done).toBe(1);
  });

  test("a cancel that removed files says what it couldn't put back", () => {
    show(summaryView({ outcome: "cancelled", failed: 1, undone: { removed: 5, notRestored: 2, failed: 1 } }));
    // Retrying only the failed files would leave a partial copy: the others were removed.
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    screen.getByRole("heading", { name: /Cancelled: the copied files were removed/ });
    screen.getByText("2 files this job replaced couldn't be brought back: their new versions stay.");
    screen.getByText("1 file couldn't be removed (see the report).");
  });

  test("a mirror cancelled with its copies removed says only that", () => {
    show(
      summaryView({
        outcome: "cancelled",
        undone: { removed: 3, notRestored: 0, failed: 0 },
        mirror: {
          new: 0, updated: 0, removed: 0, archived: true, removalFailures: [],
          nothingRemoved: "Files deleted in the origin were left in the destination: the mirror was cancelled.",
        },
      }),
    );
    screen.getByRole("heading", { name: /Cancelled: the destination is back as it was/ });
    expect(screen.queryByText(/left in the destination/)).toBeNull();
  });

  test("failures from different places (unread items, directories, files) all show", () => {
    const row = { id: 0, finalPath: "x", size: 0, millis: 0, hash: null, status: "failed" as const };
    show(
      summaryView({
        outcome: "failures",
        failed: 1,
        unread: 1,
        dirErrors: 1,
        failures: [
          { ...row, path: "DCIM/locked", reason: "Couldn't be read: permission denied" },
          { ...row, path: "EMPTY", reason: "Empty directory not created: file exists" },
          { ...row, path: "A001.mov", reason: "Hash mismatch" },
        ],
      }),
    );
    expect(within(screen.getByRole("region", { name: "Failed" })).getAllByRole("listitem")).toHaveLength(3);
  });

  test("a check: problems in checksum files, what wasn't checked, no Retry or checksum note", () => {
    show(
      summaryView({
        outcome: "failures",
        failed: 1,
        checksumOff: true,
        check: { intact: 5, changed: 1, missing: 0, failed: 0, notChecked: 12, checksumFiles: 2, problems: ["a.xxh64:3: bad line"] },
      }),
    );
    screen.getByRole("heading", { name: /1 file changed/ });
    within(screen.getByRole("region", { name: "Problems" })).getByText("a.xxh64:3: bad line");
    expect(hintOf(screen.getByText("12 not checked"))).toMatch(/no checksum file lists/);
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    expect(screen.queryByText(/No checksum file/)).toBeNull();
  });

  test("a check with more problems than listed says how many more", () => {
    show(
      summaryView({
        outcome: "failures",
        failed: 0,
        check: { intact: 5, changed: 0, missing: 0, failed: 0, notChecked: 0, checksumFiles: 1, problems: ["a.xxh64:3: bad line"], moreProblems: 1_233 },
      }),
    );
    screen.getByRole("heading", { name: /1,234 checksum file problems/ });
    within(screen.getByRole("region", { name: "Problems" })).getByText("and 1,233 more (see the report)");
  });

  test("not started explains itself", () => {
    show(summaryView({ outcome: "cancelled", notStarted: 150 }));
    expect(hintOf(screen.getByText("150 not started"))).toMatch(/cancelled or stopped/);
  });

  test("a mirror that removed nothing says why", () => {
    show(
      summaryView({
        outcome: "failures",
        failed: 2,
        mirror: { new: 0, updated: 0, removed: 0, archived: true, removalFailures: [], nothingRemoved: "Files deleted in the origin were left in the destination: 2 files failed." },
      }),
    );
    screen.getByText("Files deleted in the origin were left in the destination: 2 files failed.");
  });

  test("every file is listed with its status and checksum", async () => {
    const { api } = fakeApi();
    const rows: FinishedRow[] = ["A001.MP4", "A002.MP4", "A003.MP4"].map((name, id) => ({
      id,
      path: name,
      finalPath: name,
      size: 2_300_000_000,
      millis: 2_000,
      hash: `d78a9dd8afc9649${id}`,
      status: "verified",
      reason: null,
    }));
    api.finishedPage.mockResolvedValue(rows);
    render(Summary, {
      props: { summary: summaryView({ files: 3, finished: 3 }), onRetry: () => {}, onNewCopy: () => {} },
      context: apiContext(api),
    });
    await waitFor(() => expect(api.finishedPage).toHaveBeenCalledWith(0, 100, false));
    await screen.findByText("A002.MP4");
    screen.getByText("d78a9dd8afc96492");
    expect(screen.getAllByText("✓ Verified")).toHaveLength(3);
  });

  test("a report that couldn't be saved says why", () => {
    show(summaryView({ reportFile: null, reportError: "/Users/me/reports: Permission denied" }));
    screen.getByText("The report could not be saved: /Users/me/reports: Permission denied");
  });

  test("a complete job: status, figures and no Retry", () => {
    show(summaryView({ skippedIdentical: 284 }));
    expect(screen.getByRole("status").textContent).toContain("All 1,284 files copied and verified");
    screen.getByText("212.4 GB written");
    screen.getByText("took 4:12");
    screen.getByText("284 already at the destination, not checked");
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
  });

  test("failures are listed with reasons and can be retried", async () => {
    const { calls } = show(
      summaryView({
        outcome: "failures",
        failed: 1,
        verified: 1283,
        failures: [
          {
            id: 7,
            path: "DCIM/A001.mov",
            finalPath: "DCIM/A001.mov",
            size: 10,
            millis: 1,
            hash: null,
            status: "failed",
            reason: "Hash mismatch (source 0000000000000001, copy 0000000000000002)",
          },
        ],
      }),
    );
    screen.getByText(/Hash mismatch/);
    await fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(calls).toEqual(["retry"]);
  });

  test("Show in Finder, Open checksum file and Save report", async () => {
    const { api } = show(summaryView());
    await fireEvent.click(screen.getByRole("button", { name: "Show in Finder" }));
    expect(api.reveal).toHaveBeenCalledWith("/Volumes/RAID/Day01/DCIM");
    await fireEvent.click(screen.getByRole("button", { name: "Open checksum file" }));
    expect(api.openFile).toHaveBeenCalledWith("/Volumes/RAID/Day01/secopy_2026-09-27_140302.xxh64");
    await fireEvent.click(screen.getByRole("button", { name: "Save report…" }));
    await waitFor(() => expect(api.saveReport).toHaveBeenCalledWith("/tmp/report.txt"));
    expect(api.pickReportPath).toHaveBeenCalledWith("r.txt");
  });

  test("a stopped job says why", () => {
    show(summaryView({ outcome: "stopped", stoppedBecause: "The destination drive is full", notStarted: 40 }));
    expect(screen.getByRole("status").textContent).toContain("Stopped: The destination drive is full");
    screen.getByText("40 not started");
  });

  test("a checksum file that couldn't be written is shown", () => {
    show(summaryView({ checksumFile: null, checksumError: "Permission denied" }));
    screen.getByText(/checksum file could not be written: Permission denied/);
    expect(screen.queryByRole("button", { name: "Open checksum file" })).toBeNull();
  });
  test("no checksum file when it's off in Settings", () => {
    show(summaryView({ checksumFile: null, checksumOff: true }));
    screen.getByText("No checksum file (off in Settings)");
    expect(screen.queryByRole("button", { name: "Open checksum file" })).toBeNull();
  });
  test("the bar puts what you'd do next first, and New copy on the right", () => {
    show(
      summaryView({
        failed: 1,
        outcome: "failures",
        checksumFile: "/Volumes/RAID/Day01/secopy.xxh64",
      }),
    );
    const bar = within(screen.getByRole("group", { name: "Actions" }));
    expect(bar.getAllByRole("button").map((b) => b.textContent?.trim())).toEqual([
      "Retry",
      "Show in Finder",
      "Open checksum file",
      "Save report…",
      "New copy",
    ]);
  });
});

describe("Summary: help on Retry", () => {
  test("Retry says how many failed files it sets up again", () => {
    show(summaryView({ outcome: "failures", failed: 3 }));
    expect(helpOf(screen.getByRole("button", { name: "Retry" }))).toBe(
      "Sets up a new copy of just the 3 failed files; press Start to run it.",
    );
  });

  test("one failed file", () => {
    show(summaryView({ outcome: "failures", failed: 1 }));
    expect(helpOf(screen.getByRole("button", { name: "Retry" }))).toBe(
      "Sets up a new copy of just the 1 failed file; press Start to run it.",
    );
  });

  test("the obvious buttons have none", () => {
    show(summaryView({ outcome: "failures", failed: 3 }));
    for (const name of ["Show in Finder", "Save report…", "New copy"])
      expect(helpOf(screen.getByRole("button", { name }))).toBeNull();
  });
});
