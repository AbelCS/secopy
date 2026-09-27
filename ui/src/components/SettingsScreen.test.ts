import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { Profile, Settings } from "../lib/bindings";
import { fakeApi, profile, settingsView } from "../test/fake-api";
import SettingsScreen from "./SettingsScreen.svelte";

function show(settings: Settings = settingsView(), profiles: Profile[] = [profile()]) {
  const { api, state } = fakeApi();
  const calls = { settings: [] as Settings[], profiles: [] as Profile[][], done: 0 };
  render(SettingsScreen, {
    props: {
      settings,
      profiles,
      onSettings: (s: Settings) => calls.settings.push(s),
      onProfiles: (p: Profile[]) => calls.profiles.push(p),
      onView: () => {},
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

  test("New creates a profile from the form", async () => {
    const { api, calls } = show(settingsView(), []);
    await fireEvent.click(screen.getByRole("button", { name: "New profile" }));
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "DJI" } });
    await fireEvent.input(screen.getByLabelText("Folder on the card"), { target: { value: "DCIM" } });
    await fireEvent.input(screen.getByLabelText(/File types/), { target: { value: "mp4, .SRT" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(api.createProfile).toHaveBeenCalledWith({
        name: "DJI",
        folder: "DCIM",
        includeFolder: true,
        extensions: ["mp4", ".SRT"],
      }),
    );
    expect(calls.profiles).toHaveLength(1);
  });

  test("Edit fills the form and saves the changes", async () => {
    const { api } = show();
    await fireEvent.click(screen.getByRole("button", { name: "Edit Sony FX3" }));
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "Sony FX3");
    expect(screen.getByLabelText(/File types/)).toHaveProperty("value", "mp4");
    await fireEvent.input(screen.getByLabelText(/File types/), { target: { value: "" } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(api.editProfile).toHaveBeenCalledWith("fx3", {
        name: "Sony FX3",
        folder: "PRIVATE/M4ROOT/CLIP",
        includeFolder: true,
        extensions: null,
      }),
    );
  });

  test("Delete asks first", async () => {
    const { api } = show();
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "Delete Sony FX3" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(api.deleteProfile).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Delete Sony FX3" }));
    await waitFor(() => expect(api.deleteProfile).toHaveBeenCalledWith("fx3"));
  });

  test("a profile error is shown next to the form", async () => {
    const { api } = show(settingsView(), []);
    api.createProfile.mockRejectedValueOnce(new Error("The profile needs a name."));
    await fireEvent.click(screen.getByRole("button", { name: "New profile" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await screen.findByText("The profile needs a name.");
  });

  test("Done goes back", async () => {
    const { calls } = show();
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(calls.done).toBe(1);
  });
});
