import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { SummaryView } from "../lib/bindings";
import { fakeApi, summaryView } from "../test/fake-api";
import Summary from "./Summary.svelte";

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
  test("a report that couldn't be saved says why", () => {
    show(summaryView({ reportFile: null, reportError: "/Users/me/reports: Permission denied" }));
    screen.getByText("The report could not be saved: /Users/me/reports: Permission denied");
  });

  test("a complete job: status, figures and no Retry", () => {
    show(summaryView({ skippedIdentical: 284 }));
    expect(screen.getByRole("status").textContent).toContain("All 1,284 files copied and verified");
    screen.getByText("212.4 GB written");
    screen.getByText("4:12");
    screen.getByText("284 already at the destination, not checked");
    expect(screen.queryByRole("button", { name: "Retry failed" })).toBeNull();
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
    await fireEvent.click(screen.getByRole("button", { name: "Retry failed" }));
    expect(calls).toEqual(["retry"]);
  });

  test("Reveal, Open checksum file and Save report", async () => {
    const { api } = show(summaryView());
    await fireEvent.click(screen.getByRole("button", { name: "Reveal in Finder" }));
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
});
