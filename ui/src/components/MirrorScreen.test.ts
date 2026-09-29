import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import { AppError } from "../lib/message";
import type { MirrorPreset, MirrorPreviewView, QueueView } from "../lib/bindings";
import { fakeApi, mirrorPreset } from "../test/fake-api";
import MirrorScreen from "./MirrorScreen.svelte";
import { helpOf } from "../test/hint";

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

  test("a cancelled preview shows no error; a failed one says why", async () => {
    const { api } = show();
    api.previewMirror.mockRejectedValueOnce(new AppError({ key: "errors.mirror.previewCancelled", args: {} }));
    await fireEvent.click(screen.getByRole("button", { name: "Preview…" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Preview…" })).toHaveProperty("disabled", false));
    expect(screen.queryByText("Cancelled.")).toBeNull();
    api.previewMirror.mockRejectedValueOnce(new AppError({ key: "errors.mirror.same", args: {} }));
    await fireEvent.click(screen.getByRole("button", { name: "Preview…" }));
    await screen.findByText("The origin and the destination are the same directory.");
  });

  /** Switches the saved archive-mode preset to Delete and presses Save. */
  async function switchToDelete() {
    await fireEvent.click(screen.getByRole("radio", { name: "Delete them" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
  }

  test("switching to Delete with an empty archive just saves", async () => {
    const { api } = show();
    await switchToDelete();
    await waitFor(() => expect(api.editMirrorPreset).toHaveBeenCalled());
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.deleteMirrorArchive).not.toHaveBeenCalled();
  });

  test("switching to Delete asks about the archive; Delete them now deletes it, then saves", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue({ state: "files", files: 124, bytes: 38_200_000_000 });
    api.deleteMirrorArchive.mockResolvedValue({ removed: 124, notDeleted: null });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    within(dialog).getByText("The archive holds 124 files (38.2 GB) from earlier runs.");
    await fireEvent.click(within(dialog).getByRole("button", { name: "Delete them now" }));
    await waitFor(() => expect(api.editMirrorPreset).toHaveBeenCalled());
    expect(api.deleteMirrorArchive).toHaveBeenCalledWith("m1");
    expect(api.deleteMirrorArchive.mock.invocationCallOrder[0]).toBeLessThan(api.editMirrorPreset.mock.invocationCallOrder[0]);
    await screen.findByText("Deleted 124 archived files.");
  });

  test("Keep them for the preset's days saves and deletes nothing; Cancel doesn't save", async () => {
    const { api } = show([mirrorPreset({ deleted: { mode: "archive", days: 7 } })]);
    api.mirrorArchive.mockResolvedValue({ state: "files", files: 1, bytes: 5 });
    await switchToDelete();
    let dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    await fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(api.editMirrorPreset).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    await fireEvent.click(within(dialog).getByRole("button", { name: "Keep them 7 days" }));
    await waitFor(() => expect(api.editMirrorPreset).toHaveBeenCalled());
    expect(api.deleteMirrorArchive).not.toHaveBeenCalled();
  });

  test("Keep says the days being saved, when they changed too", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue({ state: "files", files: 1, bytes: 5 });
    await fireEvent.input(screen.getByRole("spinbutton", { name: "Days to keep" }), { target: { value: "7" } });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    within(dialog).getByRole("button", { name: "Keep them 7 days" });
  });

  test("a new destination in the same save doesn't ask about the old one's archive", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue({ state: "files", files: 1, bytes: 5 });
    await fireEvent.input(screen.getByRole("textbox", { name: "Destination" }), { target: { value: "/Volumes/Other/Footage" } });
    await switchToDelete();
    await waitFor(() => expect(api.editMirrorPreset).toHaveBeenCalled());
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("a destination that isn't connected can have its archive deleted at the next run", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue({ state: "unavailable" });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    within(dialog).getByText("The destination isn't connected, so its archive can't be checked.");
    expect(within(dialog).queryByRole("button", { name: "Delete them now" })).toBeNull();
    await fireEvent.click(within(dialog).getByRole("button", { name: "Delete it at the next run" }));
    await waitFor(() => expect(api.clearMirrorArchiveNextRun).toHaveBeenCalledWith("m1"));
    expect(api.editMirrorPreset).toHaveBeenCalled();
  });

  test("while a job runs, the archive is deleted at the next run or kept", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue({ state: "busy" });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    within(dialog).getByText("A job is running, so the archive can't be deleted now.");
    within(dialog).getByRole("button", { name: "Delete it at the next run" });
    within(dialog).getByRole("button", { name: "Keep archived files 30 days" });
  });

  test("the days say when archived files go; shortening them says what the next run removes", async () => {
    show();
    screen.getByText(/Each run first removes archived files older than this/);
    await fireEvent.input(screen.getByRole("spinbutton", { name: "Days to keep" }), { target: { value: "7" } });
    await screen.findByText("At the next run, files archived more than 7 days ago are removed.");
  });

  test("a problem with the origin is shown under Origin", async () => {
    const { api } = show();
    api.editMirrorPreset.mockRejectedValueOnce(new AppError({ key: "errors.field.origin.notFull", args: {} }));
    await fireEvent.input(screen.getByRole("textbox", { name: "Origin" }), { target: { value: "Footage" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    const error = await screen.findByText("The origin must be a full path, like /Volumes/SSD/Footage.");
    expect(screen.getByRole("textbox", { name: "Origin" }).getAttribute("aria-describedby")).toBe(error.id);
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

describe("MirrorScreen: help on Add to queue", () => {
  test("says the mirror is worked out again when it runs; Preview… has none", () => {
    show();
    expect(helpOf(screen.getByRole("button", { name: "Add to queue" }))).toBe(
      "Adds this mirror to the Queue; what to copy and remove is worked out again when it runs.",
    );
    expect(helpOf(screen.getByRole("button", { name: "Preview…" }))).toBeNull();
  });

  test("Export… saves the selected mirror under its name", async () => {
    const { api } = show();
    api.pickExportPath.mockResolvedValue("/Users/me/Footage.secopy");
    api.exportMirrorPreset.mockResolvedValue("Exported “Footage”.");
    await fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    expect(api.pickExportPath).toHaveBeenCalledWith(`${mirrorPreset().name}.secopy`);
    await waitFor(() => expect(api.exportMirrorPreset).toHaveBeenCalledWith(mirrorPreset().id, "/Users/me/Footage.secopy"));
    await screen.findByText("Exported “Footage”.");
  });

  test("Export… with unsaved changes asks first; Keep editing exports nothing", async () => {
    const { api } = show();
    api.confirm.mockResolvedValue(false);
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "Other" } });
    await fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(api.pickExportPath).not.toHaveBeenCalled();
  });
});
