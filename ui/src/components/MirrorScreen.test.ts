import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { MirrorPreset, MirrorPreviewView, QueueView } from "../lib/bindings";
import { fakeApi, mirrorPreset } from "../test/fake-api";
import MirrorScreen from "./MirrorScreen.svelte";

function show(presets: MirrorPreset[] = [mirrorPreset()]) {
  const { api } = fakeApi();
  const calls = { presets: [] as MirrorPreset[][], preview: [] as MirrorPreviewView[], queue: [] as QueueView[] };
  render(MirrorScreen, {
    props: {
      presets,
      onPresets: (p: MirrorPreset[]) => calls.presets.push(p),
      onPreview: (v: MirrorPreviewView) => calls.preview.push(v),
      onQueue: (q: QueueView) => calls.queue.push(q),
    },
    context: apiContext(api),
  });
  return { api, calls };
}

describe("MirrorScreen", () => {
  test("a preset shows its origin, destination and what happens to deleted files", () => {
    show();
    expect(screen.getByRole("textbox", { name: "Origin" })).toHaveProperty("value", "/Volumes/SSD/Footage");
    expect(screen.getByRole("textbox", { name: "Destination" })).toHaveProperty("value", "/Volumes/Media/Footage");
    expect(screen.getByLabelText("Archive them")).toHaveProperty("checked", true);
    expect(screen.getByRole("spinbutton", { name: "Days to keep" })).toHaveProperty("value", "30");
  });

  test("Checking says copies are always verified, and what the deep check adds", () => {
    show();
    screen.getByText(/always verified after copying/);
    const deep = screen.getByRole("checkbox", { name: "Also compare unchanged files byte for byte" });
    expect(deep).toHaveProperty("checked", false);
    screen.getByText(/same size and date are normally left alone/);
  });

  test("a long Preview… says how far it is and can be cancelled", async () => {
    const { api } = show();
    let report: ((c: { done: number; total: number }) => void) | undefined;
    api.previewMirror.mockImplementation((_id: string, onCompared: (c: { done: number; total: number }) => void) => {
      report = onCompared;
      return new Promise(() => {});
    });
    await fireEvent.click(screen.getByRole("button", { name: "Preview…" }));
    expect(screen.getByRole("button", { name: "Preview…" })).toHaveProperty("disabled", true);
    report!({ done: 120, total: 2410 });
    await screen.findByText("Comparing contents: 120 of 2,410 files");
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(api.cancelMirrorPreview).toHaveBeenCalled();
  });

  test("Preview… previews the saved preset; edits must be saved first", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByRole("button", { name: "Preview…" }));
    await waitFor(() => expect(api.previewMirror).toHaveBeenCalledWith("m1", expect.any(Function)));
    expect(calls.preview).toHaveLength(1);
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "Other" } });
    expect(screen.queryByRole("button", { name: "Preview…" })).toBeNull();
    screen.getByRole("button", { name: "Save" });
  });

  test("Delete permanently asks what it means", async () => {
    show();
    await fireEvent.click(screen.getByLabelText("Delete them"));
    screen.getByText(/can't be undone/);
    expect(screen.queryByRole("spinbutton", { name: "Days to keep" })).toBeNull();
  });

  test("Add to queue queues the preset", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByRole("button", { name: "Add to queue" }));
    await waitFor(() => expect(api.addMirrorToQueue).toHaveBeenCalledWith("m1"));
    expect(calls.queue).toHaveLength(1);
  });

  test("leaving unsaved changes asks once, however often the list is clicked", async () => {
    const { api } = show([mirrorPreset(), mirrorPreset({ id: "m2", name: "Audio → NAS" })]);
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "Other" } });
    let answer: ((discard: boolean) => void) | undefined;
    api.confirm.mockImplementationOnce(() => new Promise((r) => (answer = r)));
    await fireEvent.click(screen.getByRole("button", { name: "Audio → NAS" }));
    await fireEvent.click(screen.getByRole("button", { name: "Audio → NAS" }));
    expect(api.confirm).toHaveBeenCalledTimes(1);
    answer!(true);
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "Audio → NAS"));
  });

  test("with no presets it explains mirrors", async () => {
    show([]);
    screen.getByText(/keeps a copy of a directory identical/);
    await fireEvent.click(screen.getByRole("button", { name: "+ New mirror" }));
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "");
  });
});
