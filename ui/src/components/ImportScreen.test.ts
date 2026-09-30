import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import { raw } from "../test/fake-api";
import type { ImportView } from "../lib/bindings";
import ImportScreen from "./ImportScreen.svelte";

const view = (over: Partial<ImportView> = {}): ImportView => ({
  fileName: "Team presets.secopy",
  settings: { changes: [raw("Write the checksum file: on → off")], problem: null },
  copyPresets: [
    { name: "Sony FX3", paths: ["/Volumes/CARD_A/CLIP"], clash: "Sony FX3", newName: "Sony FX3 (2)", missing: [], problem: null, replaceNote: null, section: false },
    { name: "DJI", paths: ["/Volumes/DJI/DCIM"], clash: null, newName: "DJI", missing: ["/Volumes/DJI/DCIM"], problem: null, replaceNote: null, section: false },
    { name: "Bad", paths: [], clash: null, newName: "Bad", missing: [], problem: raw("Its details can't be read (…)."), replaceNote: null, section: false },
  ],
  mirrorPresets: [],
  ...over,
});

describe("ImportScreen", () => {
  test("a list of presets that can't be read is named as the list", () => {
    const section = { name: "", paths: [], clash: null, newName: "", missing: [], problem: raw("This part of the file can't be read."), replaceNote: null, section: true };
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
      replaceNote: raw("Replacing removes archived files older than 7 days at its next run."),
      section: false,
    };
    render(ImportScreen, { props: { view: view({ copyPresets: [], mirrorPresets: [mirror] }), onImport: vi.fn(), onBack: () => {} } });
    expect(screen.queryByText(/Replacing removes/)).toBeNull();
    await fireEvent.click(screen.getByRole("radio", { name: "Replace yours" }));
    screen.getByText("Replacing removes archived files older than 7 days at its next run.");
  });

  test("a preset with no name in the file says so", () => {
    const unnamed = { name: "", paths: [], clash: null, newName: "", missing: [], problem: raw("Its details can't be read."), replaceNote: null, section: false };
    render(ImportScreen, {
      props: { view: view({ copyPresets: [unnamed], mirrorPresets: [unnamed] }), onImport: vi.fn(), onBack: () => {} },
    });
    screen.getByRole("checkbox", { name: "A copy preset with no name" });
    screen.getByRole("checkbox", { name: "A mirror preset with no name" });
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
    const v = view({ settings: { changes: [], problem: null }, copyPresets: [view().copyPresets[1]] });
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
        { name: "A", paths: [], clash: null, newName: "A", missing: [], problem: null, replaceNote: null, section: false },
        { name: "a", paths: [], clash: null, newName: "a (2)", missing: [], problem: null, replaceNote: null, section: false },
      ],
    });
    render(ImportScreen, { props: { view: v, onImport: vi.fn(), onBack: () => {} } });
    screen.getByText("Imported as “a (2)”: the file has this name twice.");
  });
});
