import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import ExportDialog from "./ExportDialog.svelte";

describe("ExportDialog", () => {
  test("everything there is ticked; an empty kind is off", async () => {
    const onExport = vi.fn();
    render(ExportDialog, { props: { copyPresets: 3, mirrorPresets: 0, onExport, onClose: () => {} } });
    expect(screen.getByRole("checkbox", { name: "Settings" })).toHaveProperty("checked", true);
    expect(screen.getByRole("checkbox", { name: "Copy presets (3)" })).toHaveProperty("checked", true);
    const mirrors = screen.getByRole("checkbox", { name: "Mirror presets (none)" });
    expect(mirrors).toHaveProperty("checked", false);
    expect(mirrors).toHaveProperty("disabled", true);
    await fireEvent.click(screen.getByRole("checkbox", { name: "Settings" }));
    await fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    expect(onExport).toHaveBeenCalledWith({ settings: false, copyPresets: true, mirrorPresets: false });
  });

  test("with nothing ticked, Export… is off", async () => {
    render(ExportDialog, { props: { copyPresets: 0, mirrorPresets: 0, onExport: () => {}, onClose: () => {} } });
    await fireEvent.click(screen.getByRole("checkbox", { name: "Settings" }));
    expect(screen.getByRole("button", { name: "Export…" })).toHaveProperty("disabled", true);
  });
});
