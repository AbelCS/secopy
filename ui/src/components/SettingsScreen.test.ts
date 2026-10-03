import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { Settings } from "../lib/bindings";
import { fakeApi, settingsView } from "../test/fake-api";
import SettingsScreen from "./SettingsScreen.svelte";

function show(settings: Settings = settingsView()) {
  const { api, state } = fakeApi();
  const calls = { settings: [] as Settings[], done: 0 };
  render(SettingsScreen, {
    props: {
      settings,
      onSettings: (s: Settings) => calls.settings.push(s),
      onDone: () => calls.done++,
    },
    context: apiContext(api),
  });
  return { api, state, calls };
}

const actions = () => within(screen.getByRole("group", { name: "Actions" }));
const save = () => actions().getByRole("button", { name: "Save" });

describe("SettingsScreen", () => {
  test("the patterns are a list, one per row, with how many (#161)", async () => {
    show();
    const list = screen.getByRole("list", { name: "Always ignore when copying" });
    expect(within(list).getAllByRole("listitem").map((li) => li.textContent?.trim())).toEqual([
      ".DS_Store",
      "._*",
      "Thumbs.db",
    ]);
    screen.getByText("3 patterns");
    await fireEvent.click(screen.getByRole("button", { name: "Remove .DS_Store" }));
    screen.getByText("2 patterns");
  });

  test("an empty list says every file is copied", async () => {
    show(settingsView({ ignore: [] }));
    screen.getByText("No patterns. Secopy’s own working files are still left out.");
  });

  test("a pattern is added and removed, and Save sends the list (#158)", async () => {
    const { api, calls } = show();
    await fireEvent.input(screen.getByLabelText("Name pattern"), { target: { value: "*.LRF" } });
    await fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await fireEvent.click(screen.getByRole("button", { name: "Remove ._*" }));
    await fireEvent.click(save());
    await waitFor(() => expect(calls.done).toBe(1));
    expect(api.setSettings).toHaveBeenCalledWith(settingsView({ ignore: [".DS_Store", "Thumbs.db", "*.LRF"] }));
  });

  test("Restore defaults puts the defaults back", async () => {
    const { api } = show(settingsView({ ignore: ["*.LRF"] }));
    api.defaultIgnore.mockResolvedValue([".DS_Store", "Thumbs.db"]);
    await fireEvent.click(screen.getByRole("button", { name: "Restore defaults" }));
    await screen.findByText(".DS_Store");
    expect(screen.queryByText("*.LRF")).toBeNull();
  });

  test("a pattern with / is refused with why, and a repeat isn't added", async () => {
    show();
    const field = screen.getByLabelText("Name pattern");
    await fireEvent.input(field, { target: { value: "a/b" } });
    await fireEvent.click(screen.getByRole("button", { name: "Add" }));
    screen.getByText("A pattern is a name: it can’t contain /.");
    expect(screen.queryByText("a/b")).toBeNull();
    await fireEvent.input(field, { target: { value: ".ds_store" } });
    await fireEvent.click(screen.getByRole("button", { name: "Add" }));
    screen.getByText("It’s already in the list.");
    expect(screen.getAllByRole("button", { name: /^Remove / })).toHaveLength(3);
  });

  test("Write ASC MHL can be turned on (#154)", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByLabelText("Write ASC MHL"));
    await fireEvent.click(save());
    await waitFor(() => expect(calls.done).toBe(1));
    expect(api.setSettings).toHaveBeenCalledWith(settingsView({ writeMhl: true }));
  });

  test("Esc goes back once; a held key's repeats don't count", async () => {
    const { calls } = show();
    await fireEvent.keyDown(window, { key: "Escape", repeat: true });
    expect(calls.done).toBe(0);
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(calls.done).toBe(1);
  });

  test("a change applies when saved, and Save goes back", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByLabelText("Write the checksum file to the destination"));
    expect(api.setSettings).not.toHaveBeenCalled();
    expect(calls.settings).toEqual([]);
    await fireEvent.click(save());
    const off = settingsView({ writeChecksumFile: false });
    await waitFor(() => expect(calls.done).toBe(1));
    expect(api.setSettings).toHaveBeenCalledWith(off);
    expect(calls.settings).toEqual([off]);
  });

  test("Save is on only when something changed", async () => {
    show();
    expect(save()).toHaveProperty("disabled", true);
    const box = screen.getByLabelText("Show the count of ignored files");
    await fireEvent.click(box);
    expect(save()).toHaveProperty("disabled", false);
    await fireEvent.click(box);
    expect(save()).toHaveProperty("disabled", true);
  });

  test("Cancel drops the changes and goes back", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByLabelText("Show the count of ignored files"));
    await fireEvent.click(actions().getByRole("button", { name: "Cancel" }));
    expect(calls.done).toBe(1);
    expect(api.setSettings).not.toHaveBeenCalled();
    expect(calls.settings).toEqual([]);
  });

  test("Esc is Cancel", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByLabelText("Show the count of ignored files"));
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(calls.done).toBe(1);
    expect(api.setSettings).not.toHaveBeenCalled();
  });

  test("the report option is off while there is no checksum file", async () => {
    show();
    const report = () => screen.getByLabelText("Save the report next to the checksum file");
    expect(report()).toHaveProperty("disabled", false);
    await fireEvent.click(screen.getByLabelText("Write the checksum file to the destination"));
    expect(report()).toHaveProperty("disabled", true);
  });

  test("a save error is shown, and nothing is left", async () => {
    const { api, calls } = show();
    api.setSettings.mockRejectedValueOnce(new Error("Couldn’t save the settings: disk full"));
    await fireEvent.click(screen.getByLabelText("Show the count of ignored files"));
    await fireEvent.click(save());
    await screen.findByText("Couldn’t save the settings: disk full");
    expect(calls.done).toBe(0);
    expect(calls.settings).toEqual([]);
  });

  test("Settings has only the settings; copy presets have their own screen", () => {
    show();
    expect(screen.queryByText(/preset/i)).toBeNull();
  });

  test("each setting explains itself", () => {
    show();
    screen.getByText(/xxhsum -c/);
    screen.getByText(/\.DS_Store/);
    screen.getByText(/also kept in the app/);
  });

  test("#172: copy options under Copies, the app's under General", () => {
    show();
    const copies = within(screen.getByRole("region", { name: "Copies" }));
    copies.getByRole("checkbox", { name: "Write the checksum file to the destination" });
    copies.getByRole("checkbox", { name: "Save the report next to the checksum file" });
    const general = within(screen.getByRole("region", { name: "General" }));
    general.getByRole("checkbox", { name: "Notify when a job finishes" });
    general.getByRole("checkbox", { name: "Keep jobs running in the menu bar when the window is closed" });
    expect(copies.queryByRole("checkbox", { name: "Notify when a job finishes" })).toBeNull();
  });

  test("notifications can be turned off", async () => {
    const { api } = show();
    await fireEvent.click(screen.getByLabelText("Notify when a job finishes"));
    await fireEvent.click(save());
    await waitFor(() => expect(api.setSettings).toHaveBeenCalledWith(settingsView({ notifyWhenDone: false })));
  });

  test("the menu bar setting is on, and saving keeps it off when unticked", async () => {
    const { api } = show();
    const box = screen.getByRole("checkbox", { name: "Keep jobs running in the menu bar when the window is closed" });
    expect(box).toHaveProperty("checked", true);
    await fireEvent.click(box);
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.setSettings).toHaveBeenCalledWith(expect.objectContaining({ keepInMenuBar: false }));
  });
});
