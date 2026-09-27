import { expect, test } from "vitest";
import { summaryView } from "../test/fake-api";
import { headline } from "./headline";

test("each outcome has a clear status line", () => {
  expect(headline(summaryView())).toBe("All 1,284 files copied and verified");
  expect(headline(summaryView({ verify: false, verified: 0, copied: 1 }))).toBe("All 1 file copied");
  expect(headline(summaryView({ outcome: "failures", failed: 3 }))).toBe("3 files failed");
  expect(headline(summaryView({ outcome: "cancelled" }))).toBe("Cancelled");
  expect(
    headline(summaryView({ outcome: "stopped", stoppedBecause: "The destination drive is full" })),
  ).toBe("Stopped: The destination drive is full");
  expect(headline(summaryView({ verified: 0, skippedIdentical: 12 }))).toBe(
    "Nothing to copy: everything was already there",
  );
});
