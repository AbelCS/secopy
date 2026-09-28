import { fireEvent, render, screen, within } from "@testing-library/svelte";
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
          saveError: null,
        },
        onOpen: (i: number) => calls.open.push(i),
        onDone: () => calls.done++,
      },
    });
    screen.getByRole("heading", { name: "Queue done: 1 of 3 jobs complete" });
    screen.getByText("took 1:42:10");
    screen.getByText("Not run: the queue stopped.");
    const list = screen.getByRole("list", { name: "Job results" });
    const open = within(list).getAllByRole("button", { name: /^Summary of job \d/ });
    expect(open).toHaveLength(2); // no summary for a job that didn't run
    expect(open[1]).toBe(screen.getByRole("button", { name: /^Summary of job 2, Copy & Verify · \/Volumes\/CARD_B\/DCIM to / }));
    await fireEvent.click(open[1]);
    expect(calls.open).toEqual([1]);
  });

  test("a mirror job's row names it", () => {
    render(QueueSummary, {
      props: {
        summary: {
          complete: 1, count: 1, millis: 1000,
          results: [{ job: queuedJob({ kind: "mirror", name: "Footage" }), result: "complete", reason: null, summary: summaryView() }],
          saveError: null,
        },
        onOpen: () => {},
        onDone: () => {},
      },
    });
    screen.getByText("Mirror · Footage");
  });

  test("a job for a newer Secopy says so, and a queue that couldn't be saved says why", () => {
    render(QueueSummary, {
      props: {
        summary: {
          complete: 0, count: 1, millis: 1000,
          results: [{ job: queuedJob({ kind: "unknown", supported: false, source: "", destination: "" }), result: "failed", reason: "Needs a newer Secopy.", summary: null }],
          saveError: "Couldn't save the queue: permission denied",
        },
        onOpen: () => {},
        onDone: () => {},
      },
    });
    screen.getByText("A job for a newer Secopy");
    expect(screen.queryByText("→")).toBeNull();
    screen.getByText("Couldn't save the queue: permission denied");
    expect(screen.getByRole("list")).toBeTruthy();
  });

  test("a queued mirror whose preset was deleted says so", () => {
    render(QueueSummary, {
      props: {
        summary: {
          complete: 0, count: 1, millis: 1000,
          results: [{ job: queuedJob({ kind: "mirror", supported: false, name: null }), result: "failed", reason: "The mirror preset no longer exists.", summary: null }],
          saveError: null,
        },
        onOpen: () => {},
        onDone: () => {},
      },
    });
    screen.getByText("A mirror that was deleted");
    expect(screen.queryByText("A job for a newer Secopy")).toBeNull();
  });

  test("a check job's row says Verify", () => {
    render(QueueSummary, {
      props: {
        summary: {
          complete: 1, count: 1, millis: 1000,
          results: [{ job: queuedJob({ kind: "check", source: "/Volumes/Backup/Day01", destination: "" }), result: "complete", reason: null, summary: summaryView() }],
          saveError: null,
        },
        onOpen: () => {},
        onDone: () => {},
      },
    });
    screen.getByText("Verify");
    expect(screen.queryByText("→")).toBeNull();
    screen.getByRole("button", { name: "Summary of job 1, Verify · /Volumes/Backup/Day01" });
  });
});
