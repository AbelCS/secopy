import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { queuedJob, summaryView } from "../test/fake-api";
import QueueSummary from "./QueueSummary.svelte";

describe("QueueSummary", () => {
  test("one row per job, each opening its summary", async () => {
    const calls = { open: [] as number[], done: 0 };
    render(QueueSummary, {
      props: {
        summary: {
          complete: 1, count: 3, millis: 6_130_000,
          results: [
            { job: queuedJob(), result: "complete", reason: null, summary: summaryView() },
            { job: queuedJob({ source: "/Volumes/CARD_B/DCIM" }), result: "failed", reason: "3 files failed.", summary: summaryView({ outcome: "failures", failed: 3 }) },
            { job: queuedJob({ source: "/x" }), result: "notRun", reason: "Not run: the queue stopped.", summary: null },
          ],
        },
        onOpen: (i: number) => calls.open.push(i),
        onDone: () => calls.done++,
      },
    });
    screen.getByRole("heading", { name: "Queue done: 1 of 3 jobs complete" });
    screen.getByText("took 1:42:10");
    screen.getByText("Not run: the queue stopped.");
    const open = screen.getAllByRole("button", { name: "Summary" });
    expect(open).toHaveLength(2); // no summary for a job that didn't run
    await fireEvent.click(open[1]);
    expect(calls.open).toEqual([1]);
  });

  test("a mirror job's row names it", () => {
    render(QueueSummary, {
      props: {
        summary: {
          complete: 1, count: 1, millis: 1000,
          results: [{ job: queuedJob({ kind: "mirror", name: "Footage" }), result: "complete", reason: null, summary: summaryView() }],
        },
        onOpen: () => {},
        onDone: () => {},
      },
    });
    screen.getByText("Mirror · Footage");
  });
});
