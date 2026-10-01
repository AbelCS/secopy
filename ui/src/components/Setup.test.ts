import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import { raw } from "../test/fake-api";
import { apiContext } from "../lib/api";
import type { CopyPreset, CopyPresetsView, QueueView, SessionView, Settings } from "../lib/bindings";
import {
  destinationView,
  fakeApi,
  copyPreset,
  queueView,
  readyView,
  sessionView,
  settingsView,
  sourceView,
} from "../test/fake-api";
import Setup from "./Setup.svelte";
import { helpOf, hintOf, showing } from "../test/hint";

function setup(
  view: SessionView = sessionView(),
  answer: SessionView = view,
  props: Partial<{ presets: CopyPreset[]; settings: Settings; recent: string[] }> = {},
) {
  const { api, state } = fakeApi(answer);
  const started: number[] = [];
  const calls = { presets: [] as CopyPreset[][], manage: 0, modes: [] as boolean[], queued: [] as QueueView[] };
  const result = render(Setup, {
    props: {
      view,
      verify: true,
      presets: props.presets ?? [],
      settings: props.settings ?? settingsView(),
      recent: props.recent ?? [],
      onStart: () => started.push(1),
      onPresets: (p: CopyPreset[]) => calls.presets.push(p),
      onManagePresets: () => calls.manage++,
      onMode: (v: boolean) => calls.modes.push(v),
      onQueued: (q: QueueView) => calls.queued.push(q),
    },
    context: apiContext(api),
  });
  return { api, state, started, calls, ...result };
}

const start = () => screen.getByRole("button", { name: "Start" });
const from = () => within(screen.getByRole("region", { name: "From" }));
const to = () => within(screen.getByRole("region", { name: "To" }));
const includeFolder = () => screen.getByLabelText("Include the “DCIM” directory");

describe("Setup", () => {
  test("Start stays disabled until there is something to copy", () => {
    setup();
    screen.getByText("Drop a directory or files here, or choose them.");
    screen.getByText("Drop the destination directory here, or choose it.");
    expect(start()).toHaveProperty("disabled", true);
  });

  test("a ready session enables Start and says what it will do", async () => {
    const { started } = setup(readyView());
    const button = screen.getByRole("button", { name: "Start" });
    screen.getByText("1,284 files · 212.4 GB");
    expect(button).toHaveProperty("disabled", false);
    await fireEvent.click(button);
    expect(started).toHaveLength(1);
  });

  test("one Choose… picks a folder or files, and scans what was picked", async () => {
    const { api } = setup(sessionView(), readyView());
    expect(from().getAllByRole("button", { name: "Choose…" })).toHaveLength(1);
    await fireEvent.click(from().getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(api.scanSource).toHaveBeenCalledWith(["/Volumes/CARD/DCIM"]));
    await screen.findByText(showing("1,284 files · 212.4 GB · 37 ignored"));
    api.pickSource.mockResolvedValueOnce(["/a.wav", "/b.wav"]);
    await fireEvent.click(from().getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(api.scanSource).toHaveBeenLastCalledWith(["/a.wav", "/b.wav"]));
  });

  test("a folder is included by default; unticking it copies only what's inside", async () => {
    const { api } = setup(readyView());
    expect(includeFolder()).toHaveProperty("checked", true);
    await fireEvent.click(includeFolder());
    await waitFor(() => expect(api.setIncludeFolder).toHaveBeenLastCalledWith(false));
  });

  test("files have no folder to include", () => {
    setup(readyView({ source: sourceView({ isFolder: false, folder: null, label: raw("2 files") }) }));
    expect(screen.queryByRole("checkbox")).toBeNull();
  });

  test("a chip turned off narrows the filter; All selects everything again", async () => {
    const { api } = setup(readyView());
    await fireEvent.click(screen.getByRole("button", { name: /\.xml/ }));
    expect(api.setFilter).toHaveBeenLastCalledWith(["mov", "wav"]);
    await fireEvent.click(screen.getByRole("button", { name: "All" }));
    expect(api.setFilter).toHaveBeenLastCalledWith(null);
  });

  test("a Finder drop on FROM scans it, a drop on TO sets the destination", async () => {
    const { api, state, container } = setup(readyView());
    await waitFor(() => expect(state.drop).not.toBeNull());
    state.drop!(["/Volumes/CARD2"], container.querySelector('[data-drop="from"] p'));
    await waitFor(() => expect(api.scanSource).toHaveBeenCalledWith(["/Volumes/CARD2"]));
    state.drop!(["/Volumes/Backup"], container.querySelector('[data-drop="to"]'));
    await waitFor(() => expect(api.setDestination).toHaveBeenCalledWith("/Volumes/Backup"));
  });

  test("a blocker is shown and Start stays disabled", () => {
    setup(
      readyView({
        destination: destinationView({ blocker: raw("The destination is the source directory or inside it") }),
        plan: null,
      }),
    );
    expect(screen.getByRole("alert").textContent).toContain("inside it");
    expect(start()).toHaveProperty("disabled", true);
  });

  test("not enough space blocks Start", () => {
    setup(
      readyView({
        plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, overwrites: 0, blocker: raw("Not enough free space"), purgeable: null, mhl: null },
      }),
    );
    expect(screen.getByRole("alert").textContent).toBe("Not enough free space");
    expect(start()).toHaveProperty("disabled", true);
  });

  test("the plan says the ASC MHL history it writes (#154)", () => {
    setup(
      readyView({
        plan: {
          filesToWrite: 1284,
          bytesToWrite: 212_400_000_000,
          overwrites: 0,
          blocker: null,
          purgeable: null,
          mhl: { generation: 3, alsoReads: 212, alsoReadsBytes: 48_000_000_000 },
        },
      }),
    );
    to().getByText("ASC MHL: continues the history (generation 3)");
    to().getByText("Also records 212 files already here (48.0 GB).");
    expect(start()).toHaveProperty("disabled", false);
  });

  test("a new ASC MHL history says so", () => {
    setup(
      readyView({
        plan: {
          filesToWrite: 1,
          bytesToWrite: 1,
          overwrites: 0,
          blocker: null,
          purgeable: null,
          mhl: { generation: 1, alsoReads: 0, alsoReadsBytes: 0 },
        },
      }),
    );
    to().getByText("ASC MHL: new history");
    expect(to().queryByText(/Also records/)).toBeNull();
  });

  test("ignored files are counted, with the patterns in the hint (#158)", () => {
    setup(readyView({ source: { ...readyView().source!, ignored: 12 } }));
    const count = screen.getByText("12 ignored");
    expect(hintOf(count)).toContain(".DS_Store, ._*, Thumbs.db");
  });

  test("a copy that needs purgeable space warns but doesn't block", () => {
    setup(
      readyView({
        plan: {
          filesToWrite: 1284,
          bytesToWrite: 212_400_000_000,
          overwrites: 0,
          blocker: null,
          purgeable: raw("Needs purgeable space: 47.9 GB needed, 29.3 GB free now"), mhl: null,
        },
      }),
    );
    const warning = to().getByText(showing("Needs purgeable space: 47.9 GB needed, 29.3 GB free now"));
    expect(hintOf(within(warning).getByRole("button", { name: "About purgeable space" }))).toContain(
      "Time Machine local snapshots",
    );
    expect(start()).toHaveProperty("disabled", false);
  });

  test("a double click on Add to queue adds it once (#117)", async () => {
    const { api } = setup(readyView());
    api.addToQueue.mockReturnValue(new Promise(() => {}));
    const add = screen.getByRole("button", { name: "Add to queue" });
    await fireEvent.click(add);
    await fireEvent.click(add);
    expect(api.addToQueue).toHaveBeenCalledTimes(1);
  });

  test("special files (a FIFO, a socket) are said to be skipped, with what they are (#135)", () => {
    setup(readyView({ source: sourceView({ skippedSpecial: 2 }) }));
    const said = screen.getByText("2 special files skipped");
    expect(hintOf(said)).toMatch(/FIFO/);
  });

  test("where the files go is shown even when Start is blocked", () => {
    setup(
      readyView({
        plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, overwrites: 0, blocker: raw("Not enough free space"), purgeable: null, mhl: null },
      }),
    );
    to().getByText("Files go to");
    to().getByText("/Volumes/RAID/Day01/DCIM");
  });

  test("Start's status says how many files it replaces", () => {
    setup(
      readyView({
        conflicts: "overwrite",
        plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, overwrites: 3, blocker: null, purgeable: null, mhl: null },
      }),
    );
    screen.getByText("1,284 files · 212.4 GB · replaces 3 files");
  });

  test("a non-empty copy root is a warning, not a block", () => {
    setup(readyView({ destination: destinationView({ existingItems: 1204 }) }));
    screen.getByText(/Already contains 1,204 items/);
    expect(start()).toHaveProperty("disabled", false);
  });

  test("files that will fail are listed with their reasons", () => {
    setup(
      readyView({
        destination: destinationView({
          problems: [{ path: "DCIM/a:b.mov", reason: raw("the name contains \":\", which this drive doesn't allow") }],
          problemCount: 3,
        }),
      }),
    );
    screen.getByText("3 files will fail");
    screen.getByText("DCIM/a:b.mov");
    screen.getByText("and 2 more");
  });

  test("identical files, different files and leftovers are explained", async () => {
    const { api } = setup(
      readyView({ destination: destinationView({ identical: 284, differs: 12, stalePartials: 2 }) }),
    );
    screen.getByText("284 identical files will be skipped (not checked).");
    expect(hintOf(screen.getByText("Identical"))).toMatch(/same name, size and date/);
    expect(hintOf(screen.getByText("Existing files"))).toMatch(/Keep both/);
    screen.getByText("2 unfinished files from an interrupted copy will be replaced.");
    expect(screen.getByLabelText("Keep both")).toHaveProperty("checked", true);
    await fireEvent.click(screen.getByLabelText("Overwrite"));
    expect(api.setConflicts).toHaveBeenCalledWith("overwrite");
  });

  test("the mode is chosen once, next to Start; Start doesn't repeat it", async () => {
    const { calls } = setup(readyView());
    await fireEvent.click(screen.getByLabelText("Copy"));
    expect(calls.modes).toEqual([false]);
    screen.getByRole("button", { name: "Start" });
    expect(screen.queryByRole("button", { name: /^Copy/ })).toBeNull();
  });

  test("a scan in progress says so, and Start waits for it", async () => {
    const { api } = setup(readyView());
    let finishScan = (_v: SessionView) => {};
    api.scanSource.mockImplementationOnce(() => new Promise((resolve) => (finishScan = resolve)));
    await fireEvent.click(from().getByRole("button", { name: "Choose…" }));
    await screen.findByText("Scanning…");
    expect(start()).toHaveProperty("disabled", true);
    // A quicker destination check finishing meanwhile doesn't enable Start.
    await fireEvent.click(to().getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(api.setDestination).toHaveBeenCalled());
    await Promise.resolve();
    expect(start()).toHaveProperty("disabled", true);
    finishScan(readyView());
    await waitFor(() => expect(screen.queryByText("Scanning…")).toBeNull());
    expect(start()).toHaveProperty("disabled", false);
  });

  test("a scan replaced by a newer one doesn't change the view", async () => {
    const { api } = setup(readyView(), sessionView({ stale: true }));
    await fireEvent.click(from().getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(api.scanSource).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByText("Scanning…")).toBeNull());
    screen.getByText("/Volumes/CARD/DCIM");
    expect(start()).toHaveProperty("disabled", false);
  });

  test("copying only what's inside still names the folder, and ticking it includes it again", async () => {
    const { api } = setup(readyView({ source: sourceView({ contentsOnly: true }) }));
    expect(includeFolder()).toHaveProperty("checked", false);
    await fireEvent.click(includeFolder());
    await waitFor(() => expect(api.setIncludeFolder).toHaveBeenLastCalledWith(true));
  });

  test("a retry has no folder choice or filter to change", () => {
    setup(readyView({ source: sourceView({ isRetry: true, label: raw("Retry: 3 failed files") }) }));
    screen.getByText("Retry: 3 failed files");
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(screen.queryByRole("button", { name: /\.xml/ })).toBeNull();
  });

  test("a pick problem is shown in FROM", () => {
    setup(sessionView({ pickProblem: raw("CARD_A has no PRIVATE/M4ROOT/CLIP") }));
    expect(from().getByRole("alert").textContent).toBe("CARD_A has no PRIVATE/M4ROOT/CLIP");
  });

  test("a command error is shown where it happened", async () => {
    const { api } = setup(sessionView({ source: sourceView() }));
    api.setDestination.mockRejectedValueOnce(new Error("Can't write to the destination"));
    await fireEvent.click(to().getByRole("button", { name: "Choose…" }));
    await screen.findByText("Can't write to the destination");
  });

  test("choosing a preset selects it; Manage presets… opens Copy presets", async () => {
    const { api, calls } = setup(readyView(), readyView(), { presets: [copyPreset()] });
    const menu = screen.getByRole("combobox", { name: "Preset" });
    await fireEvent.change(menu, { target: { value: "fx3" } });
    await waitFor(() => expect(api.selectCopyPreset).toHaveBeenCalledWith("fx3"));
    await fireEvent.change(menu, { target: { value: "manage" } });
    expect(calls.manage).toBe(1);
  });

  test("a preset changed for this run offers Update", async () => {
    const { api, calls } = setup(readyView({ presetId: "fx3", presetChanged: true }), readyView(), {
      presets: [copyPreset()],
    });
    screen.getByText("Changed for this run");
    await fireEvent.click(screen.getByRole("button", { name: "Update" }));
    await waitFor(() => expect(api.updateCopyPreset).toHaveBeenCalled());
    expect(calls.presets).toHaveLength(1);
  });

  test("Save as… asks only for a name; the source and settings are what's on screen", async () => {
    const { api } = setup(readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Save as…" }));
    expect(screen.getAllByRole("textbox")).toHaveLength(1);
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "FX3" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.saveCopyPresetAs).toHaveBeenCalledWith("FX3"));
  });

  test("a preset that can't be saved says why", async () => {
    const { api } = setup(readyView());
    api.saveCopyPresetAs.mockRejectedValueOnce(new Error("There is already a preset called “FX3”."));
    await fireEvent.click(screen.getByRole("button", { name: "Save as…" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await screen.findByText("There is already a preset called “FX3”.");
  });

  test("choosing another preset drops Save as… and its error", async () => {
    const { api } = setup(readyView(), readyView({ presetId: "fx3" }), { presets: [copyPreset()] });
    api.saveCopyPresetAs.mockRejectedValueOnce(new Error("There is already a preset called “FX3”."));
    await fireEvent.click(screen.getByRole("button", { name: "Save as…" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await screen.findByText("There is already a preset called “FX3”.");
    await fireEvent.change(screen.getByRole("combobox", { name: "Preset" }), { target: { value: "fx3" } });
    await waitFor(() => expect(screen.queryByText("There is already a preset called “FX3”.")).toBeNull());
    expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();
  });

  test("another source drops Save as… and its error", async () => {
    const { api, rerender } = setup(readyView());
    api.saveCopyPresetAs.mockRejectedValueOnce(new Error("There is already a preset called “FX3”."));
    await fireEvent.click(screen.getByRole("button", { name: "Save as…" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await screen.findByText("There is already a preset called “FX3”.");
    const other = "/Volumes/CARD_B/DCIM";
    await rerender({ view: readyView({ source: sourceView({ label: raw(other), folder: other }) }) });
    expect(screen.queryByText("There is already a preset called “FX3”.")).toBeNull();
    expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();
  });

  test("a new scan of the same source keeps Save as… open", async () => {
    const { rerender } = setup(readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Save as…" }));
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "FX3" } });
    await rerender({ view: readyView({ source: sourceView({ selectedExtensions: ["mov"] }) }) });
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "FX3");
  });

  test("presets don't apply to files", () => {
    setup(readyView({ source: sourceView({ isFolder: false, folder: null, label: raw("2 files") }) }), undefined, {
      presets: [copyPreset()],
    });
    expect(screen.getByRole("combobox", { name: "Preset" })).toHaveProperty("disabled", true);
    expect(screen.queryByRole("button", { name: "Save as…" })).toBeNull();
  });

  test("the terms that need it explain themselves", () => {
    setup(readyView());
    expect(hintOf(screen.getByText("37 ignored"))).toMatch(/\.DS_Store/);
    expect(hintOf(screen.getByRole("button", { name: "About verifying" }))).toMatch(/back from the destination/);
  });

  test("the hidden count follows the setting", () => {
    setup(readyView(), undefined, { settings: settingsView({ showSystemCount: false }) });
    within(screen.getByRole("group", { name: "Source" })).getByText("1,284 files · 212.4 GB");
  });

  test("a setup changed while the job was being added can still be added after (#138)", async () => {
    const { api } = setup(readyView({ revision: 1 }));
    let added: (q: QueueView) => void = () => {};
    api.addToQueue.mockReturnValueOnce(new Promise((r) => (added = r)));
    api.clearSource.mockRejectedValue(new Error("busy"));
    api.setFilter.mockResolvedValueOnce(readyView({ revision: 2 }));
    await fireEvent.click(screen.getByRole("button", { name: "Add to queue" }));
    await fireEvent.click(screen.getByRole("button", { name: "None" }));
    await waitFor(() => expect(api.setFilter).toHaveBeenCalled());
    added(queueView());
    await screen.findByText("busy");
    await fireEvent.click(screen.getByRole("button", { name: "Add to queue" }));
    await waitFor(() => expect(api.addToQueue).toHaveBeenCalledTimes(2));
  });

  test("a preset's Update answering late doesn't replace a newer view (#138)", async () => {
    const view = readyView({ presetId: "fx3", presetChanged: true, revision: 1 });
    const { api } = setup(view, undefined, {
      presets: [copyPreset()],
      recent: ["/Volumes/B"],
    });
    let answer: (v: CopyPresetsView) => void = () => {};
    api.updateCopyPreset.mockReturnValueOnce(new Promise((r) => (answer = r)));
    api.setDestination.mockResolvedValueOnce(
      readyView({ revision: 3, destination: destinationView({ path: "/Volumes/B" }) }),
    );
    await fireEvent.click(screen.getByRole("button", { name: "Update" }));
    await fireEvent.change(screen.getByLabelText("Recent destinations"), { target: { value: "/Volumes/B" } });
    const shown = () => to().getAllByText(/^\/Volumes\/(B|RAID\/Day01)$/).find((e) => e.tagName === "P")?.textContent;
    await waitFor(() => expect(shown()).toBe("/Volumes/B"));
    answer({ presets: [copyPreset()], session: readyView({ revision: 2 }) });
    await new Promise((r) => setTimeout(r, 0));
    expect(shown()).toBe("/Volumes/B");
  });

  test("a job added once isn't added again when New copy couldn't clear after it (#138)", async () => {
    const { api } = setup(readyView());
    api.clearSource.mockRejectedValue(new Error("busy"));
    const add = screen.getByRole("button", { name: "Add to queue" });
    await fireEvent.click(add);
    await waitFor(() => expect(api.clearSource).toHaveBeenCalledTimes(1));
    await screen.findByText("busy");
    await fireEvent.click(add);
    expect(api.addToQueue).toHaveBeenCalledTimes(1);
  });

  test("answers that arrive out of order show the newest, not the last (#138)", async () => {
    const { api } = setup(readyView(), undefined, { recent: ["/Volumes/A", "/Volumes/B"] });
    let answerA: (v: SessionView) => void = () => {};
    api.setDestination.mockReturnValueOnce(new Promise((r) => (answerA = r)));
    api.setDestination.mockResolvedValueOnce(
      readyView({ revision: 3, destination: destinationView({ path: "/Volumes/B" }) }),
    );
    const menu = screen.getByLabelText("Recent destinations");
    const shown = () => to().getAllByText(/^\/Volumes\/[AB]$/).find((e) => e.tagName === "P")?.textContent;
    await fireEvent.change(menu, { target: { value: "/Volumes/A" } });
    await fireEvent.change(menu, { target: { value: "/Volumes/B" } });
    await waitFor(() => expect(shown()).toBe("/Volumes/B"));
    answerA(readyView({ revision: 2, destination: destinationView({ path: "/Volumes/A" }) }));
    await new Promise((r) => setTimeout(r, 0));
    expect(shown()).toBe("/Volumes/B");
  });

  test("a recent destination can be chosen again", async () => {
    const { api } = setup(readyView(), undefined, { recent: ["/Volumes/RAID/Day01"] });
    await fireEvent.change(screen.getByLabelText("Recent destinations"), {
      target: { value: "/Volumes/RAID/Day01" },
    });
    await waitFor(() => expect(api.setDestination).toHaveBeenCalledWith("/Volumes/RAID/Day01"));
  });

  test("no recent destinations, no menu", () => {
    setup(readyView());
    expect(screen.queryByLabelText("Recent destinations")).toBeNull();
  });

  test("changing the mode is reported", async () => {
    const { calls } = setup(readyView());
    await fireEvent.click(screen.getByLabelText("Copy"));
    expect(calls.modes).toEqual([false]);
  });

  test("preset actions and the Include checkbox wait for a scan", async () => {
    const { api } = setup(readyView({ presetId: "fx3", presetChanged: true }), readyView(), {
      presets: [copyPreset()],
    });
    let finishScan = (_v: SessionView) => {};
    api.selectCopyPreset.mockImplementationOnce(() => new Promise((resolve) => (finishScan = resolve)));
    await fireEvent.change(screen.getByRole("combobox", { name: "Preset" }), { target: { value: "" } });
    await screen.findByText("Scanning…");
    expect(screen.getByRole("button", { name: "Update" })).toHaveProperty("disabled", true);
    expect(includeFolder()).toHaveProperty("disabled", true);
    finishScan(readyView());
    await waitFor(() => expect(screen.queryByText("Scanning…")).toBeNull());
  });

  test("Source shows the chosen source, like Destination; no drives, no Selected row", () => {
    setup(readyView());
    const source = within(screen.getByRole("group", { name: "Source" }));
    source.getByText("/Volumes/CARD/DCIM");
    source.getByText(showing("1,284 files · 212.4 GB · 37 ignored"));
    source.getByRole("button", { name: "Choose…" });
    expect(screen.queryByRole("group", { name: "Selected" })).toBeNull();
    expect(screen.queryByText(/cards or drives/)).toBeNull();
  });

  test("with nothing chosen, Source says how to choose", () => {
    setup();
    within(screen.getByRole("group", { name: "Source" })).getByText("Drop a directory or files here, or choose them.");
  });

  test("Esc closes Save as…", async () => {
    setup(readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Save as…" }));
    const form = screen.getByRole("textbox", { name: "Name" }).closest("form")!;
    await fireEvent.keyDown(form, { key: "Escape" });
    expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();
  });

  test("files that differ are under Existing files", () => {
    setup(readyView({ destination: destinationView({ differs: 12 }) }));
    within(screen.getByRole("group", { name: "Existing files" })).getByLabelText("Keep both");
  });

  test("the status only explains why Start is off; it doesn't repeat Files go to", () => {
    setup(readyView());
    expect(screen.queryByText(/^To \//)).toBeNull();
    screen.getByText("/Volumes/RAID/Day01/DCIM");
  });

  test("with no presets, Preset offers to create one instead of an empty menu", async () => {
    const { calls } = setup();
    expect(screen.queryByRole("combobox", { name: "Preset" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "New preset…" }));
    expect(calls.manage).toBe(1);
  });

  test("closing Save as… puts focus back on its button", async () => {
    setup(readyView());
    const open = screen.getByRole("button", { name: "Save as…" });
    await fireEvent.click(open);
    await fireEvent.keyDown(screen.getByRole("textbox", { name: "Name" }), { key: "Escape" });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Save as…" }));
  });

  test("Add to queue saves the setup, says so, and clears the source", async () => {
    const { api, calls } = setup(readyView(), sessionView({ destination: readyView().destination }));
    const add = screen.getByRole("button", { name: "Add to queue" });
    expect(add).toHaveProperty("disabled", false);
    await fireEvent.click(add);
    await waitFor(() => expect(api.addToQueue).toHaveBeenCalledWith(true));
    expect(api.clearSource).toHaveBeenCalled();
    await screen.findByText(/Added to the queue/);
    expect(calls.queued).toHaveLength(1);
  });

  test("Add to queue is off exactly when Start is, and for a retry", () => {
    setup(sessionView());
    expect(screen.getByRole("button", { name: "Add to queue" })).toHaveProperty("disabled", true);
    setup(readyView({ source: sourceView({ isRetry: true }) }));
    expect(screen.getAllByRole("button", { name: "Add to queue" }).at(-1)).toHaveProperty("disabled", true);
  });
});

describe("Setup: help on the buttons", () => {
  test("Start says what it copies, where, whether it verifies, and its shortcut", async () => {
    setup(readyView());
    expect(helpOf(start())).toBe(
      "Copies 1,284 files (212.4 GB) to /Volumes/RAID/Day01/DCIM and verifies them (⌘↩).",
    );
    await fireEvent.click(screen.getByLabelText("Copy"));
    expect(helpOf(start())).toBe("Copies 1,284 files (212.4 GB) to /Volumes/RAID/Day01/DCIM (⌘↩).");
  });

  test("a disabled Start has no help: there are no figures yet", () => {
    setup();
    expect(helpOf(start())).toBeNull();
  });

  test("Add to queue says the copy runs later, from the Queue", () => {
    setup(readyView());
    expect(helpOf(screen.getByRole("button", { name: "Add to queue" }))).toBe(
      "Adds this copy, as set up now, to the Queue; it runs when you start the queue.",
    );
  });

  test("Update and Save as… say what they save", () => {
    setup(readyView({ presetId: "fx3", presetChanged: true }), readyView(), { presets: [copyPreset()] });
    expect(helpOf(screen.getByRole("button", { name: "Update" }))).toBe(
      "Saves this source and these choices into the preset “Sony FX3”.",
    );
    expect(helpOf(screen.getByRole("button", { name: "Save as…" }))).toBe(
      "Saves this source and these choices as a new preset.",
    );
  });
});
