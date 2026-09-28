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

test("a mirror says what it did", () => {
  const mirror = (over = {}) => ({
    new: 12, updated: 3, removed: 5, archived: true, removalFailures: [], nothingRemoved: null, ...over,
  });
  expect(headline(summaryView({ mirror: mirror() }))).toBe("Mirrored: 12 new, 3 updated, 5 archived");
  expect(headline(summaryView({ mirror: mirror({ updated: 0, archived: false }) }))).toBe(
    "Mirrored: 12 new, 5 deleted",
  );
  expect(headline(summaryView({ mirror: mirror({ new: 0, updated: 0, removed: 0 }) }))).toBe("Already in sync");
  expect(headline(summaryView({ outcome: "failures", failed: 3, mirror: mirror() }))).toBe("3 files failed");
  const row = { id: 0, path: "a", finalPath: "a", size: 0, millis: 0, hash: null, status: "failed" as const, reason: "x" };
  expect(
    headline(summaryView({ outcome: "failures", failed: 0, mirror: mirror({ removalFailures: [row, { ...row, id: 1 }] }) })),
  ).toBe("2 files couldn't be removed");
});

test("a cancel that removed the copied files says so", () => {
  const undone = { removed: 3, restored: 1, notRestored: 0, failed: 0 };
  expect(headline(summaryView({ outcome: "cancelled", undone }))).toBe("Cancelled: the destination is back as it was");
  expect(headline(summaryView({ outcome: "cancelled", undone: { ...undone, notRestored: 1 } }))).toBe(
    "Cancelled: the copied files were removed",
  );
});
