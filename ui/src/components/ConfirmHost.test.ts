import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import { ask } from "../lib/confirm.svelte";
import ConfirmHost from "./ConfirmHost.svelte";

describe("ConfirmHost", () => {
  test("a risky question focuses the safe answer; Esc answers no", async () => {
    render(ConfirmHost);
    const answer = ask("124 files are deleted.", "Delete archive?", "Delete", "Keep");
    const dialog = await screen.findByRole("dialog", { name: "Delete archive?" });
    within(dialog).getByText("124 files are deleted.");
    const keep = within(dialog).getByRole("button", { name: "Keep" });
    // Return presses the focused button: the safe one (#113).
    expect(document.activeElement).toBe(keep);
    const buttons = within(dialog).getAllByRole("button");
    expect(buttons.map((b) => b.textContent?.trim())).toEqual(["Keep", "Delete"]);
    expect(within(dialog).getByRole("button", { name: "Delete" }).className).toContain("danger");
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(await answer).toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("Esc answers the question only, not the screen behind it", async () => {
    const behind = vi.fn();
    window.addEventListener("keydown", behind);
    render(ConfirmHost);
    const answer = ask("Discard your changes?", "Discard?", "Discard", "Keep editing");
    await screen.findByRole("dialog");
    await fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(await answer).toBe(false);
    expect(behind).not.toHaveBeenCalled();
    window.removeEventListener("keydown", behind);
  });

  test("the risky answer says yes; the safe one no", async () => {
    render(ConfirmHost);
    let answer = ask("Clear the queue?", "Clear", "Clear", "Keep");
    await fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    expect(await answer).toBe(true);
    answer = ask("Clear the queue?", "Clear", "Clear", "Keep");
    await fireEvent.click(await screen.findByRole("button", { name: "Keep" }));
    expect(await answer).toBe(false);
  });
});
