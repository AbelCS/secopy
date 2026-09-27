import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
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

describe("SettingsScreen", () => {
  test("a toggle applies at once and is saved", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByLabelText("Write the checksum file to the destination"));
    const off = settingsView({ writeChecksumFile: false });
    expect(calls.settings).toEqual([off]);
    await waitFor(() => expect(api.setSettings).toHaveBeenCalledWith(off));
  });

  test("the report option is off while there is no checksum file", () => {
    show(settingsView({ writeChecksumFile: false }));
    expect(screen.getByLabelText("Also save the job report next to the checksum file")).toHaveProperty(
      "disabled",
      true,
    );
  });

  test("a settings save error is shown", async () => {
    const { api } = show();
    api.setSettings.mockRejectedValueOnce(new Error("Couldn't save the settings: disk full"));
    await fireEvent.click(screen.getByLabelText("Show the count of skipped hidden items"));
    await screen.findByText("Couldn't save the settings: disk full");
  });

  test("Settings has only the settings; profiles have their own screen", () => {
    show();
    expect(screen.queryByText(/profile/i)).toBeNull();
  });

  test("Done goes back", async () => {
    const { calls } = show();
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(calls.done).toBe(1);
  });
});
