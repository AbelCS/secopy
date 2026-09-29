import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import type { PanelView } from "../lib/bindings";
import { fakeApi } from "../test/fake-api";
import Panel from "./Panel.svelte";

const running = (over: Partial<PanelView> = {}): PanelView => ({
  heading: "Copying & verifying",
  from: "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP",
  to: "/Volumes/V001/Day01/CLIP",
  fraction: 0.42,
  percent: "42%",
  files: "44 of 106 files",
  speed: "850.0 MB/s",
  left: "3:12 left",
  paused: false,
  removing: false,
  ended: null,
  ...over,
});

function show(view: PanelView | null) {
  const { api, state } = fakeApi();
  api.menubarView.mockResolvedValue(view);
  render(Panel, { props: { api } });
  return { api, state };
}

describe("Panel", () => {
  test("a running job: what, from where to where, a bar, files, speed and time left", async () => {
    show(running());
    await screen.findByText("Copying & verifying");
    screen.getByText("42%");
    screen.getByText("/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP");
    screen.getByText("/Volumes/V001/Day01/CLIP");
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("42");
    screen.getByText("44 of 106 files · 850.0 MB/s · 3:12 left");
  });

  test("Pause pauses, and the button says Resume at once", async () => {
    const { api } = show(running());
    await fireEvent.click(await screen.findByRole("button", { name: "Pause" }));
    expect(api.pauseJob).toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Resume" }));
    expect(api.resumeJob).toHaveBeenCalled();
  });

  test("Open Secopy and Quit Secopy… ask the app", async () => {
    const { api } = show(running());
    await fireEvent.click(await screen.findByRole("button", { name: "Open Secopy" }));
    expect(api.openMainWindow).toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Quit Secopy…" }));
    expect(api.quitApp).toHaveBeenCalled();
  });

  test("updates arrive as events", async () => {
    const { state } = show(running());
    await screen.findByText("42%");
    state.panelView!(running({ percent: "43%", fraction: 0.43 }));
    await screen.findByText("43%");
  });

  test("the end: ✓ only for a complete job, with what happened", async () => {
    show(running({ ended: { ok: false, text: "Finished with problems" } }));
    await screen.findByText("Finished with problems");
    expect(screen.queryByRole("button", { name: "Pause" })).toBeNull();
    screen.getByRole("button", { name: "Open Secopy" });
    screen.getByRole("button", { name: "Quit Secopy" });
  });

  test("with nothing to show, it says so", async () => {
    show(null);
    await waitFor(() => screen.getByText("Nothing is running."));
  });

  test("an update from before Pause was pressed doesn't flip the button back", async () => {
    const { state } = show(running());
    await fireEvent.click(await screen.findByRole("button", { name: "Pause" }));
    state.panelView!(running({ paused: false, percent: "43%" }));
    await screen.findByText("43%");
    screen.getByRole("button", { name: "Resume" });
    state.panelView!(running({ paused: true, percent: "44%" }));
    await screen.findByText("44%");
    screen.getByRole("button", { name: "Resume" });
    state.panelView!(running({ paused: false, percent: "45%" }));
    await screen.findByText("45%");
    screen.getByRole("button", { name: "Pause" });
  });
});
