import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import { apiContext } from "../lib/api";
import type { Profile, QueueView, SessionView, Settings } from "../lib/bindings";
import {
  destinationView,
  fakeApi,
  profile,
  readyView,
  sessionView,
  settingsView,
  sourceView,
} from "../test/fake-api";
import Setup from "./Setup.svelte";

function setup(
  view: SessionView = sessionView(),
  answer: SessionView = view,
  props: Partial<{ profiles: Profile[]; settings: Settings; recent: string[] }> = {},
) {
  const { api, state } = fakeApi(answer);
  const started: number[] = [];
  const calls = { profiles: [] as Profile[][], manage: 0, modes: [] as boolean[], queued: [] as QueueView[] };
  const result = render(Setup, {
    props: {
      view,
      verify: true,
      profiles: props.profiles ?? [],
      settings: props.settings ?? settingsView(),
      recent: props.recent ?? [],
      onStart: () => started.push(1),
      onProfiles: (p: Profile[]) => calls.profiles.push(p),
      onManageProfiles: () => calls.manage++,
      onMode: (v: boolean) => calls.modes.push(v),
      onQueued: (q: QueueView) => calls.queued.push(q),
    },
    context: apiContext(api),
  });
  return { api, state, started, calls, ...result };
}

const start = () => screen.getByRole("button", { name: /copy & verify|start copy/i });
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
    const button = screen.getByRole("button", { name: "Start copy" });
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
    await screen.findByText("1,284 files · 212.4 GB · 37 system files skipped");
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
    setup(readyView({ source: sourceView({ isFolder: false, folder: null, rootDir: null, label: "2 files" }) }));
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
        destination: destinationView({ blocker: "The destination is the source directory or inside it" }),
        plan: null,
      }),
    );
    expect(screen.getByRole("alert").textContent).toContain("inside it");
    expect(start()).toHaveProperty("disabled", true);
  });

  test("not enough space blocks Start", () => {
    setup(
      readyView({
        plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, blocker: "Not enough free space" },
      }),
    );
    expect(screen.getByRole("alert").textContent).toBe("Not enough free space");
    expect(start()).toHaveProperty("disabled", true);
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
          problems: [{ path: "DCIM/a:b.mov", reason: "the name contains \":\", which this drive doesn't allow" }],
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
    screen.getByText("2 unfinished files from an interrupted copy will be replaced.");
    expect(screen.getByLabelText("Keep both")).toHaveProperty("checked", true);
    await fireEvent.click(screen.getByLabelText("Overwrite"));
    expect(api.setConflicts).toHaveBeenCalledWith("overwrite");
  });

  test("the mode is chosen once, next to Start; Start doesn't repeat it", async () => {
    const { calls } = setup(readyView());
    await fireEvent.click(screen.getByLabelText("Copy"));
    expect(calls.modes).toEqual([false]);
    screen.getByRole("button", { name: "Start copy" });
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
    const { api } = setup(readyView({ source: sourceView({ contentsOnly: true, rootDir: null }) }));
    expect(includeFolder()).toHaveProperty("checked", false);
    await fireEvent.click(includeFolder());
    await waitFor(() => expect(api.setIncludeFolder).toHaveBeenLastCalledWith(true));
  });

  test("a retry has no folder choice or filter to change", () => {
    setup(readyView({ source: sourceView({ isRetry: true, label: "Retry: 3 failed files" }) }));
    screen.getByText("Retry: 3 failed files");
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(screen.queryByRole("button", { name: /\.xml/ })).toBeNull();
  });

  test("a pick problem is shown in FROM", () => {
    setup(sessionView({ pickProblem: "CARD_A has no PRIVATE/M4ROOT/CLIP" }));
    expect(from().getByRole("alert").textContent).toBe("CARD_A has no PRIVATE/M4ROOT/CLIP");
  });

  test("a command error is shown where it happened", async () => {
    const { api } = setup(sessionView({ source: sourceView() }));
    api.setDestination.mockRejectedValueOnce(new Error("Can't write to the destination"));
    await fireEvent.click(to().getByRole("button", { name: "Choose…" }));
    await screen.findByText("Can't write to the destination");
  });

  test("choosing a profile selects it; Manage profiles… opens Settings", async () => {
    const { api, calls } = setup(readyView(), readyView(), { profiles: [profile()] });
    const menu = screen.getByRole("combobox", { name: "Profile" });
    await fireEvent.change(menu, { target: { value: "fx3" } });
    await waitFor(() => expect(api.selectProfile).toHaveBeenCalledWith("fx3"));
    await fireEvent.change(menu, { target: { value: "manage" } });
    expect(calls.manage).toBe(1);
  });

  test("a profile changed for this run offers Update profile", async () => {
    const { api, calls } = setup(readyView({ profileId: "fx3", profileChanged: true }), readyView(), {
      profiles: [profile()],
    });
    screen.getByText("Changed for this run");
    await fireEvent.click(screen.getByRole("button", { name: "Update profile" }));
    await waitFor(() => expect(api.updateProfile).toHaveBeenCalled());
    expect(calls.profiles).toHaveLength(1);
  });

  test("Save as new… asks only for a name; the source and settings are what's on screen", async () => {
    const { api } = setup(readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Save as new…" }));
    expect(screen.getAllByRole("textbox")).toHaveLength(1);
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "FX3" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.saveProfileAs).toHaveBeenCalledWith("FX3"));
  });

  test("a profile that can't be saved says why", async () => {
    const { api } = setup(readyView());
    api.saveProfileAs.mockRejectedValueOnce(new Error("There is already a profile called “FX3”."));
    await fireEvent.click(screen.getByRole("button", { name: "Save as new…" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await screen.findByText("There is already a profile called “FX3”.");
  });

  test("profiles don't apply to files", () => {
    setup(readyView({ source: sourceView({ isFolder: false, folder: null, rootDir: null, label: "2 files" }) }), undefined, {
      profiles: [profile()],
    });
    expect(screen.getByRole("combobox", { name: "Profile" })).toHaveProperty("disabled", true);
    expect(screen.queryByRole("button", { name: "Save as new…" })).toBeNull();
  });

  test("the hidden count follows the setting", () => {
    setup(readyView(), undefined, { settings: settingsView({ showSystemCount: false }) });
    within(screen.getByRole("group", { name: "Source" })).getByText("1,284 files · 212.4 GB");
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

  test("profile actions and the Include checkbox wait for a scan", async () => {
    const { api } = setup(readyView({ profileId: "fx3", profileChanged: true }), readyView(), {
      profiles: [profile()],
    });
    let finishScan = (_v: SessionView) => {};
    api.selectProfile.mockImplementationOnce(() => new Promise((resolve) => (finishScan = resolve)));
    await fireEvent.change(screen.getByRole("combobox", { name: "Profile" }), { target: { value: "" } });
    await screen.findByText("Scanning…");
    expect(screen.getByRole("button", { name: "Update profile" })).toHaveProperty("disabled", true);
    expect(includeFolder()).toHaveProperty("disabled", true);
    finishScan(readyView());
    await waitFor(() => expect(screen.queryByText("Scanning…")).toBeNull());
  });

  test("Source shows the chosen source, like Destination; no drives, no Selected row", () => {
    setup(readyView());
    const source = within(screen.getByRole("group", { name: "Source" }));
    source.getByText(readyView().source!.label);
    source.getByText("1,284 files · 212.4 GB · 37 system files skipped");
    source.getByRole("button", { name: "Choose…" });
    expect(screen.queryByRole("group", { name: "Selected" })).toBeNull();
    expect(screen.queryByText(/cards or drives/)).toBeNull();
  });

  test("with nothing chosen, Source says how to choose", () => {
    setup();
    within(screen.getByRole("group", { name: "Source" })).getByText("Drop a directory or files here, or choose them.");
  });

  test("Esc closes Save as new…", async () => {
    setup(readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Save as new…" }));
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

  test("with no profiles, Profile offers to create one instead of an empty menu", async () => {
    const { calls } = setup();
    expect(screen.queryByRole("combobox", { name: "Profile" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Create a profile…" }));
    expect(calls.manage).toBe(1);
  });

  test("closing Save as new… puts focus back on its button", async () => {
    setup(readyView());
    const open = screen.getByRole("button", { name: "Save as new…" });
    await fireEvent.click(open);
    await fireEvent.keyDown(screen.getByRole("textbox", { name: "Name" }), { key: "Escape" });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Save as new…" }));
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
