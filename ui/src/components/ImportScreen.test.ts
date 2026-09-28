import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import type { ImportView } from "../lib/bindings";
import ImportScreen from "./ImportScreen.svelte";

const view = (over: Partial<ImportView> = {}): ImportView => ({
  fileName: "Team presets.secopy",
  settings: { changes: ["Write the checksum file: on → off"], problem: null },
  copyPresets: [
    { name: "Sony FX3", paths: ["/Volumes/CARD_A/CLIP"], clash: "Sony FX3", newName: "Sony FX3 (2)", missing: [], problem: null },
    { name: "DJI", paths: ["/Volumes/DJI/DCIM"], clash: null, newName: "DJI", missing: ["/Volumes/DJI/DCIM"], problem: null },
    { name: "Bad", paths: [], clash: null, newName: "Bad", missing: [], problem: "Its details can't be read (…)." },
  ],
  mirrorPresets: [],
  ...over,
});

describe("ImportScreen", () => {
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
});
