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
    const box = screen.getByLabelText("Show the count of skipped system files");
    await fireEvent.click(box);
    expect(save()).toHaveProperty("disabled", false);
    await fireEvent.click(box);
    expect(save()).toHaveProperty("disabled", true);
  });

  test("Cancel drops the changes and goes back", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByLabelText("Show the count of skipped system files"));
    await fireEvent.click(actions().getByRole("button", { name: "Cancel" }));
    expect(calls.done).toBe(1);
    expect(api.setSettings).not.toHaveBeenCalled();
    expect(calls.settings).toEqual([]);
  });

  test("Esc is Cancel", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByLabelText("Show the count of skipped system files"));
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(calls.done).toBe(1);
    expect(api.setSettings).not.toHaveBeenCalled();
  });

  test("the report option is off while there is no checksum file", async () => {
    show();
    const report = () => screen.getByLabelText("Also save the job report next to the checksum file");
    expect(report()).toHaveProperty("disabled", false);
    await fireEvent.click(screen.getByLabelText("Write the checksum file to the destination"));
    expect(report()).toHaveProperty("disabled", true);
  });

  test("a save error is shown, and nothing is left", async () => {
    const { api, calls } = show();
    api.setSettings.mockRejectedValueOnce(new Error("Couldn't save the settings: disk full"));
    await fireEvent.click(screen.getByLabelText("Show the count of skipped system files"));
    await fireEvent.click(save());
    await screen.findByText("Couldn't save the settings: disk full");
    expect(calls.done).toBe(0);
    expect(calls.settings).toEqual([]);
  });

  test("Settings has only the settings; profiles have their own screen", () => {
    show();
    expect(screen.queryByText(/profile/i)).toBeNull();
  });

  test("each setting explains itself", () => {
    show();
    screen.getByText(/xxhsum -c/);
    screen.getByText(/\.DS_Store/);
    screen.getByText(/also kept in the app/);
  });

  test("notifications can be turned off", async () => {
    const { api } = show();
    await fireEvent.click(screen.getByLabelText("Notify when a copy finishes"));
    await fireEvent.click(save());
    await waitFor(() => expect(api.setSettings).toHaveBeenCalledWith(settingsView({ notifyWhenDone: false })));
  });
});
