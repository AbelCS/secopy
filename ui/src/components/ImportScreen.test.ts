import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import { raw } from "../test/fake-api";
import type { ImportView } from "../lib/bindings";
import ImportScreen from "./ImportScreen.svelte";

const view = (over: Partial<ImportView> = {}): ImportView => ({
  fileName: "Team presets.secopy",
  madeBy: null,
  settings: { changes: [raw("Write the checksum file: on → off")], problem: null, notImported: [], defaulted: [] },
  copyPresets: [
    { name: "Sony FX3", paths: ["/Volumes/CARD_A/CLIP"], clash: "Sony FX3", newName: "Sony FX3 (2)", missing: [], problem: null, replaceNotes: [], section: false },
    { name: "DJI", paths: ["/Volumes/DJI/DCIM"], clash: null, newName: "DJI", missing: ["/Volumes/DJI/DCIM"], problem: null, replaceNotes: [], section: false },
    { name: "Bad", paths: [], clash: null, newName: "Bad", missing: [], problem: raw("Its details can't be read (…)."), replaceNotes: [], section: false },
  ],
  mirrorPresets: [],
  ...over,
});

describe("ImportScreen", () => {
  test("a list of presets that can't be read is named as the list", () => {
    const section = { name: "", paths: [], clash: null, newName: "", missing: [], problem: raw("This part of the file can't be read."), replaceNotes: [], section: true };
    render(ImportScreen, {
      props: { view: view({ copyPresets: [section], mirrorPresets: [section] }), onImport: vi.fn(), onBack: () => {} },
    });
    screen.getByRole("checkbox", { name: "The copy presets" });
    screen.getByRole("checkbox", { name: "The mirror presets" });
  });

  test("replacing a mirror with fewer days says what its next run removes", async () => {
    const mirror = {
      name: "Footage",
      paths: ["/o", "/d"],
      clash: "Footage",
      newName: "Footage (2)",
      missing: [],
      problem: null,
      replaceNotes: [
        raw("Replacing deletes files removed from the origin instead of archiving them."),
        raw("Replacing removes archived files older than 7 days at its next run."),
      ],
      section: false,
    };
    render(ImportScreen, { props: { view: view({ copyPresets: [], mirrorPresets: [mirror] }), onImport: vi.fn(), onBack: () => {} } });
    expect(screen.queryByText(/Replacing removes/)).toBeNull();
    await fireEvent.click(screen.getByRole("radio", { name: "Replace yours" }));
    screen.getByText("Replacing deletes files removed from the origin instead of archiving them.");
    screen.getByText("Replacing removes archived files older than 7 days at its next run.");
  });

  test("a preset with no name in the file says so", () => {
    const unnamed = { name: "", paths: [], clash: null, newName: "", missing: [], problem: raw("Its details can't be read."), replaceNotes: [], section: false };
    render(ImportScreen, {
      props: { view: view({ copyPresets: [unnamed], mirrorPresets: [unnamed] }), onImport: vi.fn(), onBack: () => {} },
    });
    screen.getByRole("checkbox", { name: "A copy preset with no name" });
    screen.getByRole("checkbox", { name: "A mirror preset with no name" });
  });

  test("a file from another Secopy says so, and what its settings leave out or lack", () => {
    const settings = {
      changes: [],
      problem: null,
      notImported: ["turbo"],
      defaulted: [{ key: "import.setting.menuBar", args: {} }],
    };
    const madeBy = { key: "import.madeBy", args: { theirs: "0.19.0", ours: "0.17.6" } };
    render(ImportScreen, { props: { view: view({ madeBy, settings }), onImport: vi.fn(), onBack: () => {} } });
    screen.getByText("Made by Secopy 0.19.0; this is 0.17.6.");
    screen.getByText("“turbo” isn't imported: it's from a newer Secopy.");
    screen.getByText("Keep copying in the menu bar: not in the file, so set to its default.");
  });

  test("shows what's in the file, what changes and what clashes", () => {
    render(ImportScreen, { props: { view: view(), onImport: vi.fn(), onBack: () => {} } });
    screen.getByRole("heading", { name: "Import" });
    screen.getByText("Team presets.secopy");
    screen.getByText("Write the checksum file: on → off");
    screen.getByText("/Volumes/DJI/DCIM isn't connected now.");
    screen.getByText("Its details can't be read (…).");
    expect(screen.getByRole("checkbox", { name: "Bad" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("radio", { name: "Keep both, as “Sony FX3 (2)”" })).toHaveProperty("checked", true);
  });

  test("Import sends what's ticked, with Replace where chosen", async () => {
    const onImport = vi.fn();
    render(ImportScreen, { props: { view: view(), onImport, onBack: () => {} } });
    await fireEvent.click(screen.getByRole("radio", { name: "Replace yours" }));
    await fireEvent.click(screen.getByRole("checkbox", { name: "DJI" }));
    await fireEvent.click(screen.getByRole("button", { name: "Import" }));
    expect(onImport).toHaveBeenCalledWith({
      settings: true,
      copyPresets: [{ index: 0, replace: true }],
      mirrorPresets: [],
    });
  });

  test("settings that are the same as yours can't be ticked; nothing ticked, no Import", async () => {
    const v = view({ settings: { changes: [], problem: null, notImported: [], defaulted: [] }, copyPresets: [view().copyPresets[1]] });
    render(ImportScreen, { props: { view: v, onImport: vi.fn(), onBack: () => {} } });
    screen.getByText("Same as yours");
    await fireEvent.click(screen.getByRole("checkbox", { name: "DJI" }));
    expect(screen.getByRole("button", { name: "Import" })).toHaveProperty("disabled", true);
  });

  test("Back changes nothing", async () => {
    const onImport = vi.fn();
    const onBack = vi.fn();
    render(ImportScreen, { props: { view: view(), onImport, onBack } });
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(onBack).toHaveBeenCalled();
    expect(onImport).not.toHaveBeenCalled();
  });

  test("Import can't be pressed twice", async () => {
    let finish = () => {};
    const onImport = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    render(ImportScreen, { props: { view: view(), onImport, onBack: () => {} } });
    const button = screen.getByRole("button", { name: "Import" });
    await fireEvent.click(button);
    await fireEvent.click(button);
    expect(onImport).toHaveBeenCalledTimes(1);
    finish();
  });

  test("a preset renamed only because the file has its name twice says its new name", () => {
    const v = view({
      copyPresets: [
        { name: "A", paths: [], clash: null, newName: "A", missing: [], problem: null, replaceNotes: [], section: false },
        { name: "a", paths: [], clash: null, newName: "a (2)", missing: [], problem: null, replaceNotes: [], section: false },
      ],
    });
    render(ImportScreen, { props: { view: v, onImport: vi.fn(), onBack: () => {} } });
    screen.getByText("Imported as “a (2)”: the file has this name twice.");
  });
});
