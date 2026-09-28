import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test } from "vitest";
import { apiContext } from "../lib/api";
import type { QueueView } from "../lib/bindings";
import { checkView, fakeApi } from "../test/fake-api";
import VerifyScreen from "./VerifyScreen.svelte";

function show() {
  const { api } = fakeApi();
  const calls = { start: [] as string[], queue: [] as QueueView[] };
  render(VerifyScreen, {
    props: { onStart: (p: string) => calls.start.push(p), onQueue: (q: QueueView) => calls.queue.push(q) },
    context: apiContext(api),
  });
  return { api, calls };
}

describe("VerifyScreen", () => {
  test("choosing a directory says what it will check", async () => {
    const { api, calls } = show();
    expect(screen.getByRole("button", { name: "Start verify" })).toHaveProperty("disabled", true);
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText("3 checksum files · 1,284 files listed · 212.4 GB · 12 files not listed");
    await fireEvent.click(screen.getByRole("button", { name: "Start verify" }));
    expect(calls.start).toEqual(["/Volumes/Backup/Day01"]);
  });

  test("no checksum files: says so, and Start stays off", async () => {
    const { api } = show();
    api.checkDirectory.mockResolvedValueOnce(checkView({ checksumFiles: 0, files: 0, notChecked: 5 }));
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText(/No checksum files here/);
    expect(screen.getByRole("button", { name: "Start verify" })).toHaveProperty("disabled", true);
  });

  test("problems in checksum files are shown before starting", async () => {
    const { api } = show();
    api.checkDirectory.mockResolvedValueOnce(checkView({ problems: ["a.xxh64:2: not a \"<checksum>  <path>\" line"] }));
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText(/a\.xxh64:2/);
  });

  test("Add to queue queues the directory", async () => {
    const { api, calls } = show();
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText(/checksum files/);
    await fireEvent.click(screen.getByRole("button", { name: "Add to queue" }));
    await waitFor(() => expect(calls.queue).toHaveLength(1));
    expect(api.addCheckToQueue).toHaveBeenCalledWith("/Volumes/Backup/Day01");
  });
});
