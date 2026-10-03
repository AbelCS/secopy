import { expect, test } from "vitest";
import { summaryView } from "../test/fake-api";
import { raw } from "../test/fake-api";
import { headline } from "./headline";

test("each outcome has a clear status line", () => {
  expect(headline(summaryView())).toBe("1,284 files copied and verified");
  expect(headline(summaryView({ verify: false, verified: 0, copied: 1 }))).toBe("1 file copied");
  expect(headline(summaryView({ outcome: "failures", failed: 3 }))).toBe("3 files failed");
  expect(headline(summaryView({ outcome: "cancelled" }))).toBe("Cancelled");
  expect(
    headline(summaryView({ outcome: "stopped", stoppedBecause: raw("The destination drive is full") })),
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
  ).toBe("2 files couldn’t be removed");
  const note = { key: "format.raw", args: { text: "x" } };
  expect(
    headline(summaryView({ outcome: "failures", failed: 0, mirror: mirror({ archiveNotDeleted: note }) })),
  ).toBe("Archived files couldn’t all be deleted");
  expect(
    headline(summaryView({ outcome: "failures", failed: 0, mirror: mirror({ archiveNotCleaned: note }) })),
  ).toBe("Old archived files couldn’t all be removed");
});

test("a cancel that removed the copied files says so", () => {
  const undone = { removed: 3, notRestored: 0, failed: 0 };
  expect(headline(summaryView({ outcome: "cancelled", undone }))).toBe("Cancelled: the destination is back as it was");
  expect(headline(summaryView({ outcome: "cancelled", undone: { ...undone, notRestored: 1 } }))).toBe(
    "Cancelled: not everything could be put back",
  );
});

test("what couldn't be read, and a checksum file that wasn't written, aren't a success", () => {
  expect(headline(summaryView({ outcome: "failures", failed: 0, unread: 2 }))).toBe("2 items couldn’t be read");
  expect(headline(summaryView({ outcome: "failures", failed: 0, unread: 1 }))).toBe("1 item couldn’t be read");
  expect(headline(summaryView({ outcome: "failures", failed: 0, checksumError: raw("Permission denied") }))).toBe(
    "The checksum file couldn’t be written",
  );
});

test("a destination that couldn't confirm the copy is saved isn't a success", () => {
  expect(headline(summaryView({ outcome: "failures", failed: 0, durabilityError: raw("Input/output error") }))).toBe(
    "The destination couldn’t confirm the files are saved",
  );
});

test("files Skip left out are never hidden behind \"All\" or \"everything was already there\"", () => {
  expect(headline(summaryView({ verified: 2, skippedDifferent: 3 }))).toBe(
    "2 files copied and verified; 3 different files left unchanged",
  );
  expect(headline(summaryView({ verified: 0, skippedDifferent: 1 }))).toBe(
    "Nothing copied: 1 different file left unchanged",
  );
});

test("empty directories that couldn't be created aren't a success", () => {
  expect(headline(summaryView({ outcome: "failures", failed: 0, dirErrors: 2 }))).toBe(
    "2 empty directories couldn’t be created",
  );
});

test("a check says intact, or what it found", () => {
  const check = { intact: 1284, changed: 0, missing: 0, failed: 0, notChecked: 12, checksumFiles: 3, problems: [] };
  expect(headline(summaryView({ check }))).toBe("1,284 files intact");
  expect(headline(summaryView({ outcome: "failures", check: { ...check, intact: 1280, changed: 3, missing: 1 } }))).toBe(
    "3 files changed · 1 missing",
  );
});

test("a check that stopped says so, not what it read so far", () => {
  const check = { intact: 3, changed: 0, missing: 0, failed: 0, notChecked: 0, checksumFiles: 1, problems: [] };
  expect(headline(summaryView({ outcome: "stopped", stoppedBecause: raw("Secopy hit an internal error"), check }))).toBe(
    "Stopped: Secopy hit an internal error",
  );
  expect(headline(summaryView({ outcome: "stopped", stoppedBecause: null, check }))).toBe(
    "Stopped: verifying couldn’t continue",
  );
});

test("a check counts every checksum file problem, also past the ones listed", () => {
  const check = { intact: 5, changed: 0, missing: 0, failed: 0, notChecked: 0, checksumFiles: 1, problems: Array(1000).fill(raw("a.xxh128:1: bad line")) };
  expect(headline(summaryView({ outcome: "failures", check: { ...check, moreProblems: 234 } }))).toBe(
    "1,234 checksum file problems",
  );
  expect(headline(summaryView({ outcome: "failures", check: { ...check, moreProblems: null } }))).toBe(
    "1,000 checksum file problems",
  );
});

test("an ASC MHL that failed isn't a success (#154)", () => {
  expect(headline(summaryView({ outcome: "failures", failed: 0, mhlFailed: 2 }))).toBe(
    "2 files don’t match their ASC MHL history",
  );
  expect(headline(summaryView({ outcome: "failures", failed: 0, mhlError: raw("x") }))).toBe(
    "ASC MHL couldn’t be written",
  );
});
