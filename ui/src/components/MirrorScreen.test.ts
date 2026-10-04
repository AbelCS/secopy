import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import { AppError } from "../lib/message";
import type { MirrorPreset, MirrorPreviewView, QueueView } from "../lib/bindings";
import { fakeApi, mirrorPreset } from "../test/fake-api";
import MirrorScreen from "./MirrorScreen.svelte";
import { helpOf } from "../test/hint";

function show(presets: MirrorPreset[] = [mirrorPreset()], before?: (api: ReturnType<typeof fakeApi>["api"]) => void) {
  const { api } = fakeApi();
  before?.(api);
  const calls = { presets: [] as MirrorPreset[][], preview: [] as MirrorPreviewView[], queue: [] as QueueView[] };
  const result = render(MirrorScreen, {
    props: {
      presets,
      onPresets: (p: MirrorPreset[]) => calls.presets.push(p),
      onPreview: (v: MirrorPreviewView) => calls.preview.push(v),
      onQueue: (q: QueueView) => calls.queue.push(q),
    },
    context: apiContext(api),
  });
  const rerender = (props: { presets: MirrorPreset[] }) => result.rerender(props);
  return { api, calls, rerender };
}

describe("MirrorScreen", () => {
  test("a preset shows its origin, destination and what happens to deleted files", () => {
    show();
    expect(screen.getByRole("textbox", { name: "Origin" })).toHaveProperty("value", "/Volumes/SSD/Footage");
    expect(screen.getByRole("textbox", { name: "Destination" })).toHaveProperty("value", "/Volumes/Media/Footage");
    expect(screen.getByLabelText("Archive them")).toHaveProperty("checked", true);
    expect(screen.getByRole("spinbutton", { name: "Days to keep" })).toHaveProperty("value", "30");
  });

  test("Comparison: Standard or Paranoid, each saying what it does; Paranoid warns it's very slow", async () => {
    const { api } = show();
    screen.getByText("New and changed files are always verified after copying.");
    const standard = screen.getByRole("radio", { name: "Standard" });
    const paranoid = screen.getByRole("radio", { name: "Paranoid" });
    expect(standard).toHaveProperty("checked", true);
    expect(standard.getAttribute("aria-describedby")).toBeTruthy();
    expect(document.getElementById(standard.getAttribute("aria-describedby")!)?.textContent).toContain(
      "Size and modification date.",
    );
    const said = document.getElementById(paranoid.getAttribute("aria-describedby")!)?.textContent ?? "";
    expect(said).toContain("Compares the checksums of both copies.");
    expect(said).toContain("Very slow: reads all data on both sides.");
    await fireEvent.click(paranoid);
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(api.editMirrorPreset).toHaveBeenCalledWith("m1", expect.objectContaining({ deepCheck: true })),
    );
  });

  test("a mirror's error isn't shown on the next mirror (#199)", async () => {
    const presets = [mirrorPreset(), mirrorPreset({ id: "m2", name: "V001", destination: "/Volumes/Media/Mirror/V001" })];
    const { api } = show(presets);
    api.previewMirror.mockRejectedValueOnce(new Error("Origin “/Volumes/P001” not found."));
    await fireEvent.click(screen.getByRole("button", { name: "Preview…" }));
    await screen.findByText("Origin “/Volumes/P001” not found.");
    await fireEvent.click(screen.getByRole("button", { name: "V001" }));
    await screen.findByRole("heading", { name: "V001" });
    expect(screen.queryByText("Origin “/Volumes/P001” not found.")).toBeNull();
  });

  test("a mirror has its own Also ignore list (#164)", async () => {
    const { api } = show();
    await fireEvent.input(screen.getByLabelText("Name pattern"), { target: { value: "*.LRF" } });
    await fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(api.editMirrorPreset).toHaveBeenCalledWith("m1", expect.objectContaining({ ignore: ["*.LRF"] })),
    );
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

  const held = (over = {}) => ({
    destination: "/Volumes/Media/Footage",
    files: 124,
    bytes: 38_200_000_000,
    oldest: "2026-09-12T10:00:00+02:00",
    connected: true,
    busy: false,
    ...over,
  });

  test("a saved mirror shows its archive: files, size and oldest run", async () => {
    const { api } = show(undefined, (api) => api.mirrorArchive.mockResolvedValue(held()));
    const oldest = new Intl.DateTimeFormat("en", { day: "numeric", month: "short", year: "numeric" }).format(
      new Date("2026-09-12T10:00:00+02:00"),
    );
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText(`124 files · 38.2 GB · oldest ${oldest}`);
    await fireEvent.click(archive.getByRole("button", { name: "Show in Finder" }));
    expect(api.reveal).toHaveBeenCalledWith("/Volumes/Media/Footage/.secopy-archive");
  });

  test("an empty archive, or one whose destination isn't connected, says so; nothing to delete", async () => {
    const presets = [
      mirrorPreset(),
      mirrorPreset({ id: "m2", name: "Photos → Backup", destination: "/Volumes/Backup/Photos" }),
    ];
    const { api } = show(presets, (api) => api.mirrorArchive.mockResolvedValue(held({ files: 0, bytes: 0, oldest: null })));
    let archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText("Empty");
    expect(archive.getByRole("button", { name: "Delete archive…" })).toHaveProperty("disabled", true);
    api.mirrorArchive.mockResolvedValue(
      held({ destination: "/Volumes/Backup/Photos", files: 0, bytes: 0, oldest: null, connected: false }),
    );
    await fireEvent.click(screen.getByRole("button", { name: "Photos → Backup" }));
    archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText("Destination not found");
    expect(archive.getByRole("button", { name: "Show in Finder" })).toHaveProperty("disabled", true);
  });

  // #195: a destination that comes back (created again, plugged in) shows its archive again.
  const gone = () => held({ files: 0, bytes: 0, oldest: null, connected: false });

  test("the archive is looked at again after Choose… for the destination, even the same one (#195)", async () => {
    const { api } = show(undefined, (api) => api.mirrorArchive.mockResolvedValue(gone()));
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText("Destination not found");
    api.mirrorArchive.mockResolvedValue(held({ files: 0, bytes: 0, oldest: null }));
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Media/Footage");
    const [, destination] = screen.getAllByRole("button", { name: "Choose…" });
    await fireEvent.click(destination);
    await archive.findByText("Empty");
  });

  test("the archive is looked at again when the window comes to the front (#195)", async () => {
    const { api } = show(undefined, (api) => api.mirrorArchive.mockResolvedValue(gone()));
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText("Destination not found");
    api.mirrorArchive.mockResolvedValue(held({ files: 0, bytes: 0, oldest: null }));
    window.dispatchEvent(new Event("focus"));
    await archive.findByText("Empty");
  });

  test("the archive is looked at again after Save, with the same destination (#195)", async () => {
    const { api } = show(undefined, (api) => api.mirrorArchive.mockResolvedValue(gone()));
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText("Destination not found");
    api.mirrorArchive.mockResolvedValue(held({ files: 0, bytes: 0, oldest: null }));
    await fireEvent.click(screen.getByRole("radio", { name: "Paranoid" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await archive.findByText("Empty");
  });

  test("Delete… asks, deletes the archive and shows what's left", async () => {
    const { api } = show(undefined, (api) => api.mirrorArchive.mockResolvedValue(held()));
    api.deleteMirrorArchive.mockResolvedValue({ removed: 124, notDeleted: null });
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText(/124 files/);
    api.mirrorArchive.mockResolvedValue(held({ files: 0, bytes: 0, oldest: null }));
    await fireEvent.click(archive.getByRole("button", { name: "Delete archive…" }));
    expect(api.confirm).toHaveBeenCalledWith(
      "124 files (38.2 GB) are deleted. This can’t be undone.",
      "Delete archive?",
      "Delete",
      "Keep",
    );
    await waitFor(() => expect(api.deleteMirrorArchive).toHaveBeenCalledWith("m1", "/Volumes/Media/Footage"));
    await archive.findByText("Empty");
    await screen.findByText("Deleted 124 archived files.");
  });

  test("a new destination shows its own archive, not the old one's", async () => {
    const { api, rerender } = show(undefined, (api) => api.mirrorArchive.mockResolvedValue(held()));
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText(/124 files/);
    api.mirrorArchive.mockResolvedValue(held({ destination: "/Volumes/Other", files: 0, bytes: 0, oldest: null }));
    await rerender({ presets: [mirrorPreset({ destination: "/Volumes/Other" })] });
    await archive.findByText("Empty");
    expect(api.mirrorArchive).toHaveBeenCalledTimes(2);
  });

  test("a slow, older look at the archive doesn't replace a newer one", async () => {
    const presets = [mirrorPreset(), mirrorPreset({ id: "m2", name: "Photos → Backup", destination: "/Volumes/Backup/Photos" })];
    let slow: (v: ReturnType<typeof held>) => void = () => {};
    const { api } = show(presets, (api) => api.mirrorArchive.mockReturnValueOnce(new Promise((r) => (slow = r))));
    await fireEvent.click(screen.getByRole("button", { name: "Photos → Backup" }));
    api.mirrorArchive.mockResolvedValue(held({ files: 0, bytes: 0, oldest: null }));
    await fireEvent.click(screen.getByRole("button", { name: "Footage → NAS" }));
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText("Empty");
    slow(held());
    await new Promise((r) => setTimeout(r, 0));
    archive.getByText("Empty");
  });

  test("a deletion pending for the next run is shown", async () => {
    show([mirrorPreset({ deleted: { mode: "delete", days: 30 }, clearArchive: "/Volumes/Media/Footage" })], (api) =>
      api.mirrorArchive.mockResolvedValue(held()),
    );
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText("Deleted at the next run");
  });

  test("while a job runs, the archive can't be deleted", async () => {
    show(undefined, (api) => api.mirrorArchive.mockResolvedValue(held({ busy: true })));
    const archive = within(await screen.findByRole("region", { name: "Archive" }));
    await archive.findByText(/124 files/);
    const del = archive.getByRole("button", { name: "Delete archive…" });
    expect(del).toHaveProperty("disabled", true);
    archive.getByText("Wait until the job finishes.");
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

  test("switching to Delete asks about the archive; Delete them now saves, then deletes it", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue(held({ oldest: null }));
    api.deleteMirrorArchive.mockResolvedValue({ removed: 124, notDeleted: null });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    within(dialog).getByText("The archive holds 124 files (38.2 GB) from earlier runs.");
    await fireEvent.click(within(dialog).getByRole("button", { name: "Delete them now" }));
    await waitFor(() => expect(api.deleteMirrorArchive).toHaveBeenCalledWith("m1", "/Volumes/Media/Footage"));
    // Saved first: a save that fails deletes nothing (#113).
    expect(api.editMirrorPreset.mock.invocationCallOrder[0]).toBeLessThan(api.deleteMirrorArchive.mock.invocationCallOrder[0]);
    await screen.findByText("Deleted 124 archived files.");
  });

  test("an archive that can't be deleted after the save says so, though the editor is new (code review, #192)", async () => {
    const { api } = fakeApi();
    api.mirrorArchive.mockResolvedValue(held({ oldest: null }));
    const edited = { ...mirrorPreset(), deleted: { mode: "delete" as const, days: 30 } };
    api.editMirrorPreset.mockResolvedValue([edited]);
    api.deleteMirrorArchive.mockRejectedValueOnce(new Error("The destination isn’t connected."));
    // The app passes the saved list back, so the editor is made again (as in App.svelte).
    const result = render(MirrorScreen, {
      props: {
        presets: [mirrorPreset()],
        onPresets: (p: MirrorPreset[]) => void result.rerender({ presets: p }),
        onPreview: () => {},
        onQueue: () => {},
      },
      context: apiContext(api),
    });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    await fireEvent.click(within(dialog).getByRole("button", { name: "Delete them now" }));
    await screen.findByText("Saved. The archived files weren’t deleted: The destination isn’t connected.");
  });

  test("Delete them now deletes nothing when the edit can't be saved", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue(held({ oldest: null }));
    api.editMirrorPreset.mockRejectedValueOnce(new Error("A mirror named that already exists."));
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    await fireEvent.click(within(dialog).getByRole("button", { name: "Delete them now" }));
    await screen.findByText("A mirror named that already exists.");
    expect(api.deleteMirrorArchive).not.toHaveBeenCalled();
  });

  test("Keep them for the preset's days saves and deletes nothing; Cancel doesn't save", async () => {
    const { api } = show([mirrorPreset({ deleted: { mode: "archive", days: 7 } })]);
    api.mirrorArchive.mockResolvedValue({ destination: "/Volumes/Media/Footage", files: 1, bytes: 5, oldest: null, connected: true, busy: false });
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
    api.mirrorArchive.mockResolvedValue({ destination: "/Volumes/Media/Footage", files: 1, bytes: 5, oldest: null, connected: true, busy: false });
    await fireEvent.input(screen.getByRole("spinbutton", { name: "Days to keep" }), { target: { value: "7" } });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    within(dialog).getByRole("button", { name: "Keep them 7 days" });
  });

  test("a new destination in the same save doesn't ask about the old one's archive", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue({ destination: "/Volumes/Media/Footage", files: 1, bytes: 5, oldest: null, connected: true, busy: false });
    await fireEvent.input(screen.getByRole("textbox", { name: "Destination" }), { target: { value: "/Volumes/Other/Footage" } });
    await switchToDelete();
    await waitFor(() => expect(api.editMirrorPreset).toHaveBeenCalled());
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("a destination that isn't found can have its archive deleted at the next run", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue({ destination: "/Volumes/Media/Footage", files: 0, bytes: 0, oldest: null, connected: false, busy: false });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    within(dialog).getByText("Destination not found: its archive can’t be checked.");
    expect(within(dialog).queryByRole("button", { name: "Delete them now" })).toBeNull();
    await fireEvent.click(within(dialog).getByRole("button", { name: "Delete them at the next run" }));
    await waitFor(() => expect(api.clearMirrorArchiveNextRun).toHaveBeenCalledWith("m1"));
    expect(api.editMirrorPreset).toHaveBeenCalled();
  });

  test("while a job runs, the archive is deleted at the next run or kept", async () => {
    const { api } = show();
    api.mirrorArchive.mockResolvedValue({ destination: "/Volumes/Media/Footage", files: 3, bytes: 5, oldest: null, connected: true, busy: true });
    await switchToDelete();
    const dialog = await screen.findByRole("dialog", { name: "Files already archived" });
    within(dialog).getByText("A job is running, so the archive can’t be deleted now.");
    within(dialog).getByRole("button", { name: "Delete them at the next run" });
    within(dialog).getByRole("button", { name: "Keep archived files 30 days" });
  });

  test("the days say when archived files go; shortening them says what the next run removes", async () => {
    show();
    screen.getByText(/Each run deletes those older than this/);
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
    screen.getByText(/can’t be undone/);
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
    screen.getByText(/keeps a destination identical to its origin/);
    await fireEvent.click(screen.getByRole("button", { name: "+ New mirror…" }));
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "");
  });
});

describe("MirrorScreen: help on Add to queue", () => {
  test("says the mirror is worked out again when it runs; Preview… has none", () => {
    show();
    expect(helpOf(screen.getByRole("button", { name: "Add to queue" }))).toBe(
      "Adds this mirror to the queue; what to copy and remove is worked out again when it runs.",
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
