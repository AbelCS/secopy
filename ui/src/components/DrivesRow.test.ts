import { render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, test, vi } from "vitest";
import { apiContext } from "../lib/api";
import type { DriveView } from "../lib/bindings";
import { drive, fakeApi } from "../test/fake-api";
import DrivesRow from "./DrivesRow.svelte";

function show() {
  const { api } = fakeApi();
  let answer: (drives: DriveView[]) => void = () => {};
  api.listDrives.mockImplementation(() => new Promise((resolve) => (answer = resolve)));
  render(DrivesRow, { props: { source: null, onPick: () => {} }, context: apiContext(api) });
  return { api, answer: (drives: DriveView[]) => answer(drives) };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("DrivesRow", () => {
  test("says it is looking until the first answer, not that nothing is connected", async () => {
    const { answer } = show();
    screen.getByText("Looking for drives…");
    expect(screen.queryByText("No cards or drives connected.")).toBeNull();
    answer([]);
    await screen.findByText("No cards or drives connected.");
  });

  test("doesn't ask again while a slow answer is on its way", async () => {
    vi.useFakeTimers();
    const { api, answer } = show();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(api.listDrives).toHaveBeenCalledTimes(1);
    answer([drive()]);
    await vi.advanceTimersByTimeAsync(2_000);
    expect(api.listDrives).toHaveBeenCalledTimes(2);
  });
});
