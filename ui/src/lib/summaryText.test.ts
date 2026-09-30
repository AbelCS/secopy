import { describe, expect, test } from "vitest";
import { queuedJob, raw, summaryView } from "../test/fake-api";
import type { QueueResult, QueueSummaryView } from "./bindings";
import { notificationFor, queueNotification, summaryStats } from "./summaryText";

describe("summaryText", () => {
  test("the figures of a summary", () => {
    expect(summaryStats(summaryView({ skippedIdentical: 3 }))).toEqual([
      "1,284 files",
      "212.4 GB written",
      "took 4:12",
      "842.9 MB/s average",
      "3 already at the destination, not checked",
    ]);
  });

  test("one different file is left as it was", () => {
    expect(summaryStats(summaryView({ skippedDifferent: 1 })).map(String).join(" | ")).toContain(
      "1 different file left as it was",
    );
  });

  test("a notification says the headline and the main figures", () => {
    expect(notificationFor(summaryView())).toEqual({
      title: "✓ All 1,284 files copied and verified",
      body: "1,284 files · 212.4 GB written · took 4:12",
    });
    expect(notificationFor(summaryView({ outcome: "failures", failed: 3 })).title).toBe("✗ 3 files failed");
  });

  test("a report that couldn't be saved is in the notification too (#116)", () => {
    const n = notificationFor(summaryView({ reportErrors: [raw("disk full")] }));
    expect(n.body).toBe("Report not saved · 1,284 files · 212.4 GB written");
  });

  test("the queue's notification says every job finished only when each is complete (#116)", () => {
    const queue = (results: QueueResult[], saveError: QueueSummaryView["saveError"] = null): QueueSummaryView => ({
      complete: results.filter((r) => r === "complete").length,
      count: results.length,
      millis: 1000,
      results: results.map((result) => ({ job: queuedJob(), result, reason: null, summary: null })),
      saveError,
    });
    expect(queueNotification(queue(["complete", "complete"])).body).toBe("Every job finished.");
    expect(queueNotification(queue(["complete", "cancelled", "notRun"])).body).toBe("1 cancelled · 1 not run");
    expect(queueNotification(queue(["failed", "notRun", "notRun"])).body).toBe("1 job failed · 2 not run");
    expect(queueNotification(queue(["complete"], raw("Couldn't save the queue."))).body).toBe(
      "Couldn't save the queue.",
    );
    expect(queueNotification(queue(["failed", "complete"], raw("Couldn't save the queue."))).body).toBe(
      "1 job failed · Couldn't save the queue.",
    );
  });
});
