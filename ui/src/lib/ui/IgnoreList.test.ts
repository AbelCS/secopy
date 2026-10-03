import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import IgnoreList from "./IgnoreList.svelte";

function show(props: Partial<{ patterns: string[]; global: string[]; onRestore: () => void }> = {}) {
  const onChange = vi.fn();
  render(IgnoreList, { props: { label: "Also ignore", patterns: [".gitkeep"], onChange, ...props } });
  return { onChange };
}

const add = async (value: string) => {
  await fireEvent.input(screen.getByLabelText("Name pattern"), { target: { value } });
  await fireEvent.click(screen.getByRole("button", { name: "Add" }));
};

describe("IgnoreList (#164)", () => {
  test("adds and removes, one pattern per row, with how many", async () => {
    const { onChange } = show();
    within(screen.getByRole("list", { name: "Also ignore" })).getByText(".gitkeep");
    screen.getByText("1 pattern");
    await add("*.LRF");
    expect(onChange).toHaveBeenLastCalledWith([".gitkeep", "*.LRF"]);
    await fireEvent.click(screen.getByRole("button", { name: "Remove .gitkeep" }));
    expect(onChange).toHaveBeenLastCalledWith([]);
  });

  test("refuses a /, a repeat, and one Settings has already", async () => {
    const { onChange } = show({ global: [".DS_Store"] });
    await add("a/b");
    screen.getByText("A pattern is a name: it can’t contain /.");
    await add(".GITKEEP");
    screen.getByText("It’s already in the list.");
    await add(".ds_store");
    screen.getByText("It’s already in Settings › Always ignore when copying.");
    expect(onChange).not.toHaveBeenCalled();
  });

  test("refuses the 129th pattern", async () => {
    show({ patterns: Array.from({ length: 128 }, (_, i) => `p${i}`) });
    await add("one more");
    screen.getByText("The list can have up to 128 patterns.");
  });

  test("Restore defaults only where it's given", async () => {
    show();
    expect(screen.queryByRole("button", { name: "Restore defaults" })).toBeNull();
  });
});
