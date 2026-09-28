import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { MirrorPreviewView, QueueView } from "../lib/bindings";
import { fakeApi, mirrorPreview } from "../test/fake-api";
import MirrorPreview from "./MirrorPreview.svelte";
import { helpOf, hintOf } from "../test/hint";

function show(preview: MirrorPreviewView) {
  const { api } = fakeApi();
  const calls = { run: 0, cancel: 0, queue: [] as QueueView[] };
  render(MirrorPreview, {
    props: {
      preview,
      onRun: () => calls.run++,
      onQueue: (q: QueueView) => calls.queue.push(q),
      onCancel: () => calls.cancel++,
    },
    context: apiContext(api),
  });
  return { api, calls };
}

describe("MirrorPreview", () => {
  test("counts, the list by kind, and Start", async () => {
    const { api, calls } = show(
      mirrorPreview({ newFiles: 12, newBytes: 38_200_000_000, changedFiles: 3, removedFiles: 5, unchanged: 2410 }),
    );
    screen.getByText("12 new");
    screen.getByText("3 changed");
    screen.getByText(/5 deleted in the origin/);
    screen.getByText("2,410 unchanged");
    await waitFor(() => expect(api.mirrorPreviewPage).toHaveBeenLastCalledWith(null, 0, expect.any(Number)));
    await fireEvent.click(screen.getByRole("radio", { name: "Deleted" }));
    await waitFor(() => expect(api.mirrorPreviewPage).toHaveBeenLastCalledWith("removed", 0, expect.any(Number)));
    await fireEvent.click(screen.getByRole("button", { name: "Start" }));
    expect(calls.run).toBe(1);
  });

  test("the rows show path, size and why", async () => {
    const { api } = fakeApi();
    api.mirrorPreviewPage.mockResolvedValue([
      { path: "A/new.mov", size: 2_000_000, kind: "new", reason: "New" },
      { path: "B/old.mov", size: 1_000, kind: "removed", reason: "Deleted in the origin" },
    ]);
    render(MirrorPreview, {
      props: { preview: mirrorPreview(), onRun: () => {}, onQueue: () => {}, onCancel: () => {} },
      context: apiContext(api),
    });
    await screen.findByText("A/new.mov");
    screen.getByText("Deleted in the origin");
  });

  test("where archived files go is explained", () => {
    show(mirrorPreview({ removedFiles: 5, archiveDays: 30 }));
    expect(hintOf(screen.getByText("archived, kept 30 days"))).toMatch(/\.secopy-archive/);
  });

  test("files that will fail are counted, and never \"Already in sync\"", () => {
    show(mirrorPreview({ newFiles: 0, changedFiles: 0, removedFiles: 0, failing: 2, unchanged: 10 }));
    screen.getByText(/2 files will fail/);
    expect(screen.queryByText("Already in sync.")).toBeNull();
  });

  test("nothing to do: Already in sync, Run off", () => {
    show(mirrorPreview({ newFiles: 0, changedFiles: 0, removedFiles: 0, unchanged: 10 }));
    screen.getByText("Already in sync.");
    expect(screen.getByRole("button", { name: "Start" })).toHaveProperty("disabled", true);
  });

  test("a tripped guard is shown and Run asks first", async () => {
    const { api, calls } = show(
      mirrorPreview({ removedFiles: 3, guard: "3 of the destination's 3 files would be removed." }),
    );
    screen.getByText("3 of the destination's 3 files would be removed.");
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "Start" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(calls.run).toBe(0);
  });

  test("Add to queue and Cancel", async () => {
    const { api, calls } = show(mirrorPreview());
    await fireEvent.click(screen.getByRole("button", { name: "Add to queue" }));
    await waitFor(() => expect(calls.queue).toHaveLength(1));
    expect(api.addMirrorToQueue).toHaveBeenCalledWith("m1");
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(calls.cancel).toBe(1);
  });
});

describe("MirrorPreview: help on the buttons", () => {
  test("Start says what it copies and what it does with deleted files", () => {
    show(mirrorPreview());
    expect(helpOf(screen.getByRole("button", { name: "Start" }))).toBe(
      "Copies and verifies 2 new and 1 changed files, then archives 1 file gone from the origin.",
    );
    expect(helpOf(screen.getByRole("button", { name: "Add to queue" }))).toBe(
      "Adds this mirror to the Queue; what to copy and remove is worked out again when it runs.",
    );
  });

  test("parts that are 0 are left out; a preset that deletes says so", () => {
    show(mirrorPreview({ newFiles: 1, changedFiles: 0, removedFiles: 4, archiveDays: null }));
    expect(helpOf(screen.getByRole("button", { name: "Start" }))).toBe(
      "Copies and verifies 1 new file, then deletes 4 files gone from the origin.",
    );
  });

  test("only removals", () => {
    show(mirrorPreview({ newFiles: 0, changedFiles: 0, removedFiles: 1 }));
    expect(helpOf(screen.getByRole("button", { name: "Start" }))).toBe("Archives 1 file gone from the origin.");
  });
});
