import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { raw } from "../test/fake-api";
import { apiContext } from "../lib/api";
import type { QueueView } from "../lib/bindings";
import { fakeApi, queuedJob, queueView } from "../test/fake-api";
import QueueScreen from "./QueueScreen.svelte";
import { helpOf } from "../test/hint";

function show(queue: QueueView = queueView({ jobs: [queuedJob(), queuedJob({ source: raw("/Volumes/CARD_B/DCIM"), verify: false })] })) {
  const { api } = fakeApi();
  const calls = { queue: [] as QueueView[], run: 0 };
  render(QueueScreen, {
    props: { queue, onQueue: (q: QueueView) => calls.queue.push(q), onRun: () => calls.run++ },
    context: apiContext(api),
  });
  return { api, calls };
}

describe("QueueScreen", () => {
  test("each job shows its mode, source and destination", () => {
    show();
    const rows = within(screen.getByRole("list", { name: "Queued jobs" })).getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    within(rows[0]).getByText("Copy & Verify");
    within(rows[1]).getByText("Copy");
    within(rows[1]).getByText("/Volumes/CARD_B/DCIM");
  });

  test("a mirror job shows its name, origin and destination", () => {
    show(
      queueView({
        jobs: [
          queuedJob({ kind: "mirror", name: "Footage", source: raw("/Volumes/SSD/Footage"), destination: "/Volumes/Media/Footage" }),
        ],
      }),
    );
    const row = screen.getByRole("listitem");
    within(row).getByText("Mirror · Footage");
    within(row).getByText("/Volumes/SSD/Footage");
    expect(within(row).queryByText("Copy & Verify")).toBeNull();
  });

  test("a check job shows Verify and its directory", () => {
    show(queueView({ jobs: [queuedJob({ kind: "check", source: raw("/Volumes/Backup/Day01"), destination: "" })] }));
    const row = screen.getByRole("listitem");
    within(row).getByText("Verify");
    within(row).getByText("/Volumes/Backup/Day01");
    expect(within(row).queryByText("→")).toBeNull();
  });

  test("jobs move and are removed", async () => {
    const { api } = show();
    await fireEvent.click(screen.getByRole("button", { name: "Move job 1 down" }));
    expect(api.moveInQueue).toHaveBeenCalledWith(0, 1);
    await fireEvent.click(screen.getByRole("button", { name: "Remove job 2" }));
    expect(api.removeFromQueue).toHaveBeenCalledWith(1);
    expect(screen.getByRole("button", { name: "Move job 1 up" })).toHaveProperty("disabled", true);
  });

  test("a queued mirror whose preset was deleted says so", () => {
    show(queueView({ jobs: [queuedJob({ kind: "mirror", supported: false, name: null, lastError: raw("The mirror preset no longer exists.") })] }));
    screen.getByText("A mirror that was deleted");
    expect(screen.queryByText("A job for a newer Secopy")).toBeNull();
  });

  test("a move that fails leaves the focus where it was", async () => {
    const { api } = show();
    api.moveInQueue.mockRejectedValueOnce(new Error("The queue is running."));
    const down = screen.getByRole("button", { name: "Move job 1 down" });
    down.focus();
    await fireEvent.click(down);
    await screen.findByText("The queue is running.");
    expect(document.activeElement).toBe(down);
  });

  test("for VoiceOver: a list, and focus follows a moved job", async () => {
    show();
    expect(screen.getByRole("list")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Move job 1 down" }));
    // Job 1 is now job 2, the last: its Move down is off, so Move up takes the focus.
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Move job 2 up" })),
    );
  });

  test("Clear asks first", async () => {
    const { api } = show();
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "Clear…" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(api.clearQueue).not.toHaveBeenCalled();
  });

  test("the failure choice is saved, and Start runs the queue", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByLabelText("Stop the queue"));
    expect(api.setQueueOnFailure).toHaveBeenCalledWith("stop");
    await fireEvent.click(screen.getByRole("button", { name: "Start" }));
    expect(calls.run).toBe(1);
  });

  test("a failed job says why; a newer job says so", () => {
    show(queueView({ jobs: [queuedJob({ lastError: raw("CARD_A isn't connected.") }), queuedJob({ kind: "unknown", supported: false, lastError: raw("Needs a newer Secopy.") })] }));
    screen.getByText("CARD_A isn't connected.");
    screen.getByText("Needs a newer Secopy.");
  });

  test("an empty queue explains how to add jobs, and Run is off", () => {
    show(queueView());
    screen.getByText(/Add to queue/);
    expect(screen.getByRole("button", { name: "Start" })).toHaveProperty("disabled", true);
  });
});

describe("QueueScreen: help on the buttons", () => {
  test("Start says how many jobs run; Clear… that it removes them all and asks first", () => {
    show();
    expect(helpOf(screen.getByRole("button", { name: "Start" }))).toBe("Runs the 2 jobs one after another.");
    expect(helpOf(screen.getByRole("button", { name: "Clear…" }))).toBe(
      "Removes every job from the queue, after asking.",
    );
  });

  test("one job", () => {
    show(queueView({ jobs: [queuedJob()] }));
    expect(helpOf(screen.getByRole("button", { name: "Start" }))).toBe("Runs the one job in the queue.");
  });

  test("an empty queue: both off, no help", () => {
    show(queueView());
    expect(helpOf(screen.getByRole("button", { name: "Start" }))).toBeNull();
    expect(helpOf(screen.getByRole("button", { name: "Clear…" }))).toBeNull();
  });
});

describe("QueueScreen double clicks (#117)", () => {
  test("a second click while a change is on its way does nothing", async () => {
    const { api } = show();
    api.removeFromQueue.mockReturnValue(new Promise(() => {})); // still on its way
    const remove = screen.getByRole("button", { name: "Remove job 1" });
    await fireEvent.click(remove);
    await fireEvent.click(remove);
    expect(api.removeFromQueue).toHaveBeenCalledTimes(1);
  });

  test("the failure policy isn't ignored while a removal is on its way", async () => {
    const { api } = show();
    api.removeFromQueue.mockReturnValue(new Promise(() => {}));
    await fireEvent.click(screen.getByRole("button", { name: "Remove job 1" }));
    await fireEvent.click(screen.getByRole("radio", { name: "Stop the queue" }));
    expect(api.setQueueOnFailure).toHaveBeenCalledWith("stop");
  });
});
