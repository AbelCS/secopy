import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { AppError } from "../lib/message";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { CopyPreset } from "../lib/bindings";
import { fakeApi, copyPreset } from "../test/fake-api";
import CopyPresetsScreen from "./CopyPresetsScreen.svelte";

function show(presets: CopyPreset[] = [copyPreset(), copyPreset({ id: "dji", name: "DJI Mini 4", source: "/Volumes/DJI/DCIM", extensions: null })]) {
  const { api } = fakeApi();
  const calls = { presets: [] as CopyPreset[][], done: 0 };
  render(CopyPresetsScreen, {
    props: {
      presets,
      onPresets: (p: CopyPreset[]) => calls.presets.push(p),
      onView: () => {},
      onDone: () => calls.done++,
    },
    context: apiContext(api),
  });
  return { api, calls };
}

const save = () => screen.getByRole("button", { name: "Save" });
const typeInput = () => screen.getByLabelText("Add a file type");

describe("CopyPresetsScreen", () => {
  test("with no presets it explains what they are", async () => {
    show([]);
    screen.getByText(/A copy preset saves a source and its settings/);
    await fireEvent.click(screen.getByRole("button", { name: "+ New preset" }));
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "");
  });

  test("the first preset is shown in the editor, and the list switches it", async () => {
    show();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "Sony FX3");
    expect(screen.getByRole("textbox", { name: "Source" })).toHaveProperty("value", "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP");
    expect(screen.getByLabelText("Only these")).toHaveProperty("checked", true);
    screen.getByText(".mp4");
    await fireEvent.click(screen.getByRole("button", { name: "DJI Mini 4" }));
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "DJI Mini 4");
    expect(screen.getByLabelText("All types")).toHaveProperty("checked", true);
    screen.getByLabelText("Include the “DCIM” directory");
  });

  test("Save is only active once something changed", async () => {
    const { api } = show();
    expect(save()).toHaveProperty("disabled", true);
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "FX3 A-cam" } });
    expect(save()).toHaveProperty("disabled", false);
    await fireEvent.click(save());
    await waitFor(() =>
      expect(api.editCopyPreset).toHaveBeenCalledWith("fx3", {
        name: "FX3 A-cam",
        source: "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP",
        includeFolder: true,
        extensions: ["mp4"],
      }),
    );
  });

  test("file types are chips: add, remove, or all types", async () => {
    const { api } = show();
    await fireEvent.input(typeInput(), { target: { value: ".MOV" } });
    await fireEvent.keyDown(typeInput(), { key: "Enter" });
    screen.getByText(".mov");
    await fireEvent.click(screen.getByRole("button", { name: "Remove .mp4" }));
    expect(screen.queryByText(".mp4")).toBeNull();
    await fireEvent.click(save());
    await waitFor(() => expect(api.editCopyPreset).toHaveBeenLastCalledWith("fx3", expect.objectContaining({ extensions: ["mov"] })));
    await fireEvent.click(screen.getByLabelText("All types"));
    await fireEvent.click(save());
    await waitFor(() => expect(api.editCopyPreset).toHaveBeenLastCalledWith("fx3", expect.objectContaining({ extensions: null })));
  });

  test("types typed together, split by commas or spaces, become one chip each", async () => {
    const { api } = show();
    await fireEvent.input(typeInput(), { target: { value: " .MOV, wav  mp4,,.mov braw *.MXF" } });
    await fireEvent.keyDown(typeInput(), { key: "Enter" });
    for (const t of [".mp4", ".mov", ".wav", ".braw", ".mxf"]) screen.getByText(t);
    await fireEvent.click(save());
    await waitFor(() =>
      expect(api.editCopyPreset).toHaveBeenLastCalledWith("fx3", expect.objectContaining({ extensions: ["mp4", "mov", "wav", "braw", "mxf"] })),
    );
  });

  test("* or *.* typed as a type means All types", async () => {
    const { api } = show();
    for (const all of ["*", " *.* "]) {
      await fireEvent.click(screen.getByLabelText("Only these"));
      await fireEvent.input(typeInput(), { target: { value: all } });
      await fireEvent.keyDown(typeInput(), { key: "Enter" });
      expect(screen.getByLabelText("All types")).toHaveProperty("checked", true);
      expect(screen.queryByText(".*")).toBeNull();
    }
    await fireEvent.click(save());
    await waitFor(() => expect(api.editCopyPreset).toHaveBeenLastCalledWith("fx3", expect.objectContaining({ extensions: null })));
    await fireEvent.click(screen.getByLabelText("Only these"));
    expect(screen.getAllByRole("button", { name: /^Remove / }).map((b) => b.getAttribute("aria-label"))).toEqual(["Remove .mp4"]);
  });

  test("only these, with no types, can't be saved", async () => {
    show();
    await fireEvent.click(screen.getByRole("button", { name: "Remove .mp4" }));
    screen.getByText("Add at least one file type.");
    expect(save()).toHaveProperty("disabled", true);
  });

  test("Choose… fills the source with the chosen directory", async () => {
    const { api } = show();
    api.pickDirectory.mockResolvedValueOnce("/Users/me/Desktop/A");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Source" })).toHaveProperty("value", "/Users/me/Desktop/A"));
    screen.getByLabelText("Include the “A” directory");
  });

  test("a problem is shown next to its field", async () => {
    const { api } = show();
    api.editCopyPreset.mockRejectedValueOnce(
      new AppError({ key: "errors.field.name.presetTaken", args: { name: "DJI Mini 4" } }),
    );
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "DJI Mini 4" } });
    await fireEvent.click(save());
    const nameError = await screen.findByText("There is already a preset called “DJI Mini 4”.");
    expect(screen.getByRole("textbox", { name: "Name" }).getAttribute("aria-describedby")).toBe(nameError.id);
    api.editCopyPreset.mockRejectedValueOnce(new AppError({ key: "errors.field.source.notFull", args: {} }));
    await fireEvent.click(save());
    const sourceError = await screen.findByText("The source must be a full path, like /Volumes/CARD_A/DCIM.");
    expect(screen.getByRole("textbox", { name: "Source" }).getAttribute("aria-describedby")).toBe(sourceError.id);
  });

  test("an error that isn't about a field is shown for the form, whatever its words", async () => {
    const { api } = show();
    const why = { key: "errors.os.unknown", args: { text: "File name too long" } };
    api.editCopyPreset.mockRejectedValueOnce(new AppError({ key: "errors.save.preset", args: { why } }));
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "FX3 A-cam" } });
    await fireEvent.click(save());
    await screen.findByText("Couldn't save the preset: File name too long");
    expect(screen.getByRole("textbox", { name: "Name" }).getAttribute("aria-describedby")).toBeNull();
  });

  test("Revert undoes unsaved changes", async () => {
    show();
    const revert = () => screen.getByRole("button", { name: "Revert" });
    expect(revert()).toHaveProperty("disabled", true);
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "FX3 A-cam" } });
    await fireEvent.click(revert());
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "Sony FX3");
    expect(save()).toHaveProperty("disabled", true);
  });

  test("a new preset is created and selected", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByRole("button", { name: "+ New preset" }));
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "GoPro" } });
    await fireEvent.input(screen.getByRole("textbox", { name: "Source" }), { target: { value: "/Volumes/GOPRO/DCIM" } });
    await fireEvent.click(save());
    await waitFor(() =>
      expect(api.createCopyPreset).toHaveBeenCalledWith({
        name: "GoPro",
        source: "/Volumes/GOPRO/DCIM",
        includeFolder: true,
        extensions: null,
      }),
    );
    expect(calls.presets).toHaveLength(1);
  });

  test("Delete asks first", async () => {
    const { api } = show();
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "Delete…" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(api.deleteCopyPreset).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Delete…" }));
    await waitFor(() => expect(api.deleteCopyPreset).toHaveBeenCalledWith("fx3"));
  });

  test("Back goes back, from the action bar", async () => {
    const { calls } = show();
    await fireEvent.click(within(screen.getByRole("group", { name: "Actions" })).getByRole("button", { name: "Back" }));
    expect(calls.done).toBe(1);
  });

  test("leaving a preset with unsaved changes asks first", async () => {
    const { api } = show();
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "FX3 A-cam" } });
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "DJI Mini 4" }));
    await waitFor(() =>
      expect(api.confirm).toHaveBeenCalledWith(
        "Your changes to “Sony FX3” aren't saved.",
        "Discard changes?",
        "Discard",
        "Keep editing",
      ),
    );
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "FX3 A-cam");
    api.confirm.mockResolvedValueOnce(true);
    await fireEvent.click(screen.getByRole("button", { name: "DJI Mini 4" }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "DJI Mini 4"));
  });

  test("Back with unsaved changes asks first", async () => {
    const { api, calls } = show();
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "FX3 A-cam" } });
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(calls.done).toBe(0);
  });

  test("without changes nothing asks", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByRole("button", { name: "DJI Mini 4" }));
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(api.confirm).not.toHaveBeenCalled();
    expect(calls.done).toBe(1);
  });

  test("Esc is Back, and still asks about unsaved changes", async () => {
    const { api, calls } = show();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(calls.done).toBe(1);
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "FX3 A-cam" } });
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(calls.done).toBe(1);
  });

  test("holding Esc, or pressing it again, asks only once", async () => {
    const { api, calls } = show();
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "FX3 A-cam" } });
    let answer: ((discard: boolean) => void) | undefined;
    api.confirm.mockImplementationOnce(() => new Promise((r) => (answer = r)));
    await fireEvent.keyDown(window, { key: "Escape" });
    await fireEvent.keyDown(window, { key: "Escape", repeat: true });
    await fireEvent.keyDown(window, { key: "Escape" });
    await fireEvent.click(screen.getByRole("button", { name: "DJI Mini 4" }));
    expect(api.confirm).toHaveBeenCalledTimes(1);
    answer!(true);
    await waitFor(() => expect(calls.done).toBe(1));
  });

  test("Export… saves the selected preset under its name", async () => {
    const { api } = show();
    api.pickExportPath.mockResolvedValue("/Users/me/Sony FX3.secopy");
    api.exportCopyPreset.mockResolvedValue("Exported “Sony FX3”.");
    await fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    expect(api.pickExportPath).toHaveBeenCalledWith(`${copyPreset().name}.secopy`);
    await waitFor(() => expect(api.exportCopyPreset).toHaveBeenCalledWith(copyPreset().id, "/Users/me/Sony FX3.secopy"));
    await screen.findByText("Exported “Sony FX3”.");
  });

  test("Export… with unsaved changes asks first; Keep editing exports nothing", async () => {
    const { api } = show();
    api.confirm.mockResolvedValue(false);
    await fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "FX3 A-cam" } });
    await fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(api.pickExportPath).not.toHaveBeenCalled();
  });
});
