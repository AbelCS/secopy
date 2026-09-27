import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { Profile } from "../lib/bindings";
import { fakeApi, profile } from "../test/fake-api";
import ProfilesScreen from "./ProfilesScreen.svelte";

function show(profiles: Profile[] = [profile(), profile({ id: "dji", name: "DJI Mini 4", folder: "DCIM", extensions: null })]) {
  const { api } = fakeApi();
  const calls = { profiles: [] as Profile[][], done: 0 };
  render(ProfilesScreen, {
    props: {
      profiles,
      onProfiles: (p: Profile[]) => calls.profiles.push(p),
      onView: () => {},
      onDone: () => calls.done++,
    },
    context: apiContext(api),
  });
  return { api, calls };
}

const save = () => screen.getByRole("button", { name: "Save" });
const typeInput = () => screen.getByLabelText("Add a file type");

describe("ProfilesScreen", () => {
  test("with no profiles it explains what they are", async () => {
    show([]);
    screen.getByText(/A profile remembers where the clips are on a card/);
    await fireEvent.click(screen.getByRole("button", { name: "+ New profile" }));
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "");
  });

  test("the first profile is shown in the editor, and the list switches it", async () => {
    show();
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "Sony FX3");
    expect(screen.getByLabelText("Directory on the card")).toHaveProperty("value", "PRIVATE/M4ROOT/CLIP");
    expect(screen.getByLabelText("Only these")).toHaveProperty("checked", true);
    screen.getByText(".mp4");
    await fireEvent.click(screen.getByRole("button", { name: "DJI Mini 4" }));
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "DJI Mini 4");
    expect(screen.getByLabelText("All types")).toHaveProperty("checked", true);
    screen.getByLabelText("Include the “DCIM” directory");
  });

  test("Save is only active once something changed", async () => {
    const { api } = show();
    expect(save()).toHaveProperty("disabled", true);
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "FX3 A-cam" } });
    expect(save()).toHaveProperty("disabled", false);
    await fireEvent.click(save());
    await waitFor(() =>
      expect(api.editProfile).toHaveBeenCalledWith("fx3", {
        name: "FX3 A-cam",
        folder: "PRIVATE/M4ROOT/CLIP",
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
    await waitFor(() => expect(api.editProfile).toHaveBeenLastCalledWith("fx3", expect.objectContaining({ extensions: ["mov"] })));
    await fireEvent.click(screen.getByLabelText("All types"));
    await fireEvent.click(save());
    await waitFor(() => expect(api.editProfile).toHaveBeenLastCalledWith("fx3", expect.objectContaining({ extensions: null })));
  });

  test("only these, with no types, can't be saved", async () => {
    show();
    await fireEvent.click(screen.getByRole("button", { name: "Remove .mp4" }));
    screen.getByText("Add at least one file type.");
    expect(save()).toHaveProperty("disabled", true);
  });

  test("Choose… fills the folder relative to the card", async () => {
    const { api } = show();
    api.pickCardFolder.mockResolvedValueOnce("/Volumes/CARD_A/DCIM/100MSDCF");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(screen.getByLabelText("Directory on the card")).toHaveProperty("value", "DCIM/100MSDCF"));
    api.pickCardFolder.mockResolvedValueOnce("/Users/me/Desktop");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText("Choose a directory on a card or drive.");
  });

  test("a problem is shown next to its field", async () => {
    const { api } = show();
    api.editProfile.mockRejectedValueOnce(new Error("There is already a profile called “DJI Mini 4”."));
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "DJI Mini 4" } });
    await fireEvent.click(save());
    const name = await screen.findByText("There is already a profile called “DJI Mini 4”.");
    expect(name.id).toBe("name-problem");
    api.editProfile.mockRejectedValueOnce(new Error("The directory can't contain “..” or “.”."));
    await fireEvent.click(save());
    expect((await screen.findByText("The directory can't contain “..” or “.”.")).id).toBe("folder-problem");
  });

  test("a new profile is created and selected", async () => {
    const { api, calls } = show();
    await fireEvent.click(screen.getByRole("button", { name: "+ New profile" }));
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "GoPro" } });
    await fireEvent.input(screen.getByLabelText("Directory on the card"), { target: { value: "DCIM" } });
    await fireEvent.click(save());
    await waitFor(() =>
      expect(api.createProfile).toHaveBeenCalledWith({ name: "GoPro", folder: "DCIM", includeFolder: true, extensions: null }),
    );
    expect(calls.profiles).toHaveLength(1);
  });

  test("Delete asks first", async () => {
    const { api } = show();
    api.confirm.mockResolvedValueOnce(false);
    await fireEvent.click(screen.getByRole("button", { name: "Delete…" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalled());
    expect(api.deleteProfile).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Delete…" }));
    await waitFor(() => expect(api.deleteProfile).toHaveBeenCalledWith("fx3"));
  });

  test("Back goes back", async () => {
    const { calls } = show();
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(calls.done).toBe(1);
  });

  test("leaving a profile with unsaved changes asks first", async () => {
    const { api } = show();
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "FX3 A-cam" } });
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
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "FX3 A-cam");
    api.confirm.mockResolvedValueOnce(true);
    await fireEvent.click(screen.getByRole("button", { name: "DJI Mini 4" }));
    await waitFor(() => expect(screen.getByLabelText("Name")).toHaveProperty("value", "DJI Mini 4"));
  });

  test("Back with unsaved changes asks first", async () => {
    const { api, calls } = show();
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "FX3 A-cam" } });
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
});
