import { expect, test, vi } from "vitest";
import { tauriApi, unwrap } from "./api";

test("ok results give their data", async () => {
  await expect(unwrap(Promise.resolve({ status: "ok", data: 42 }))).resolves.toBe(42);
});

test("error results throw the app's message", async () => {
  const failed = unwrap(Promise.resolve({ status: "error", error: { key: "errors.blocker.destMissing", args: {} } }));
  await expect(failed).rejects.toThrow("Destination not found.");
});

const plugin = vi.hoisted(() => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-notification", () => plugin);

test("a refused notification permission is left alone", async () => {
  plugin.isPermissionGranted.mockResolvedValue(false);
  plugin.requestPermission.mockResolvedValue("denied");
  await expect(tauriApi.notify("t", "b")).resolves.toBeUndefined();
  expect(plugin.sendNotification).not.toHaveBeenCalled();
  expect(plugin.requestPermission).toHaveBeenCalledTimes(1);
});

test("with permission, the notification is sent", async () => {
  plugin.isPermissionGranted.mockResolvedValue(true);
  await tauriApi.notify("t", "b");
  expect(plugin.sendNotification).toHaveBeenCalledWith({ title: "t", body: "b" });
});

test("questions are asked in the window, not in a system alert (#113)", async () => {
  const { asking } = await import("./confirm.svelte");
  const answer = tauriApi.confirm("124 files are deleted.", "Delete archive?", "Delete", "Keep");
  expect(asking.current?.title).toBe("Delete archive?");
  asking.current?.answer(false);
  await expect(answer).resolves.toBe(false);
});
