import { describe, expect, test } from "vitest";
import { summaryView } from "../test/fake-api";
import { notificationFor, summaryStats } from "./summaryText";

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
});
