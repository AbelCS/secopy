import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import { describe, expect, test, vi } from "vitest";
import ActionBar from "./ActionBar.svelte";
import AppShell from "./AppShell.svelte";
import Button from "./Button.svelte";
import Checkbox from "./Checkbox.svelte";
import Chip from "./Chip.svelte";
import FormRow from "./FormRow.svelte";
import Hint from "./Hint.svelte";
import { hintOf } from "../../test/hint";
import Notice from "./Notice.svelte";
import RadioGroup from "./RadioGroup.svelte";
import ScreenHeader from "./ScreenHeader.svelte";
import Section from "./Section.svelte";
import SegmentedControl from "./SegmentedControl.svelte";
import Select from "./Select.svelte";
import TabBar from "./TabBar.svelte";
import Stats from "./Stats.svelte";
import TextField from "./TextField.svelte";

const text = (s: string) => createRawSnippet(() => ({ render: () => `<span>${s}</span>` }));

describe("design system", () => {
  test("Button: a variant, and a disabled one can't be clicked", async () => {
    const onclick = vi.fn();
    render(Button, { props: { variant: "primary", disabled: true, onclick, children: text("Start") } });
    const button = screen.getByRole("button", { name: "Start" });
    expect(button.className).toContain("primary");
    await fireEvent.click(button);
    expect(onclick).not.toHaveBeenCalled();
  });

  test("Section: a region named by its title", () => {
    render(Section, { props: { title: "From", children: text("content") } });
    screen.getByRole("region", { name: "From" });
    screen.getByRole("heading", { name: "From" });
  });

  test("AppShell: header, scrolling content and the action bar", () => {
    render(AppShell, { props: { header: text("head"), actions: text("act"), children: text("body") } });
    expect(screen.getByRole("banner").textContent).toBe("head");
    expect(screen.getByRole("main").textContent).toBe("body");
    expect(screen.getByRole("contentinfo").textContent).toBe("act");
  });

  test("ScreenHeader: the title; Back lives in the action bar with the other buttons", () => {
    render(ScreenHeader, { props: { title: "Settings" } });
    screen.getByRole("heading", { level: 1, name: "Settings" });
    expect(screen.queryByRole("button", { name: "Back" })).toBeNull();
  });

  test("ActionBar: other actions, status, and the main action", () => {
    render(ActionBar, { props: { start: text("Pause"), status: "40 / 106 files", end: text("Start") } });
    const bar = screen.getByRole("group", { name: "Actions" });
    expect(bar.textContent).toContain("Pause");
    expect(bar.textContent).toContain("40 / 106 files");
    expect(bar.lastElementChild?.textContent).toBe("Start");
  });

  test("SegmentedControl: radios, the chosen one checked", async () => {
    const onChange = vi.fn();
    render(SegmentedControl, {
      props: { label: "Mode", options: [{ value: "copy", label: "Copy" }, { value: "verify", label: "Copy & Verify" }], value: "verify", onChange },
    });
    screen.getByRole("radiogroup", { name: "Mode" });
    expect(screen.getByLabelText("Copy & Verify")).toHaveProperty("checked", true);
    await fireEvent.click(screen.getByLabelText("Copy"));
    expect(onChange).toHaveBeenCalledWith("copy");
  });

  test("Checkbox: its help text describes it", async () => {
    const onChange = vi.fn();
    render(Checkbox, { props: { label: "Write the checksum file", checked: true, help: text("Lists every file"), onChange } });
    const box = screen.getByLabelText("Write the checksum file");
    const help = screen.getByText("Lists every file").parentElement!;
    expect(box.getAttribute("aria-describedby")).toBe(help.id);
    await fireEvent.click(box);
    expect(onChange).toHaveBeenCalledWith(false);
  });

  test("RadioGroup: a legend and one choice", async () => {
    const onChange = vi.fn();
    render(RadioGroup, {
      props: { legend: "File types", options: [{ value: true, label: "All types" }, { value: false, label: "Only these" }], value: false, onChange },
    });
    screen.getByRole("group", { name: "File types" });
    expect(screen.getByLabelText("Only these")).toHaveProperty("checked", true);
    await fireEvent.click(screen.getByLabelText("All types"));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  test("TextField: a visible label, and an error tied to the field", () => {
    render(TextField, { props: { label: "Name", value: "FX3", error: "The profile needs a name." } });
    const input = screen.getByLabelText("Name");
    expect(input).toHaveProperty("value", "FX3");
    const error = screen.getByRole("alert");
    expect(error.textContent).toBe("The profile needs a name.");
    expect(input.getAttribute("aria-describedby")).toBe(error.id);
    expect(input.getAttribute("aria-invalid")).toBe("true");
  });

  test("Select: a labelled menu", async () => {
    const onChange = vi.fn();
    render(Select, { props: { label: "Profile", value: "", options: [{ value: "", label: "None" }, { value: "fx3", label: "Sony FX3" }], onChange } });
    await fireEvent.change(screen.getByLabelText("Profile"), { target: { value: "fx3" } });
    expect(onChange).toHaveBeenCalledWith("fx3", expect.anything());
  });

  test("Chip: a toggle shows its state; a removable one has a remove button", async () => {
    const onToggle = vi.fn();
    const onRemove = vi.fn();
    const { unmount } = render(Chip, { props: { label: ".mp4", meta: "106 · 180 GB", selected: true, onToggle } });
    const chip = screen.getByRole("button", { name: /\.mp4/ });
    expect(chip.getAttribute("aria-pressed")).toBe("true");
    await fireEvent.click(chip);
    expect(onToggle).toHaveBeenCalled();
    unmount();
    render(Chip, { props: { label: ".mov", onRemove } });
    await fireEvent.click(screen.getByRole("button", { name: "Remove .mov" }));
    expect(onRemove).toHaveBeenCalled();
  });

  test("Notice: never colour alone, and only problems interrupt", () => {
    const { unmount } = render(Notice, { props: { tone: "danger", children: text("Stopped") } });
    const alert = screen.getByRole("alert");
    expect(alert.querySelector("svg")).not.toBeNull();
    unmount();
    render(Notice, { props: { tone: "warning", children: text("Not empty") } });
    expect(screen.queryByRole("alert")).toBeNull();
    screen.getByRole("status");
  });

  test("Stats: figures in one line", () => {
    render(Stats, { props: { items: ["3 files", "7.0 GB written", "took 0:06"] } });
    const items = screen.getAllByRole("listitem").map((li) => li.textContent);
    expect(items).toEqual(["3 files", "7.0 GB written", "took 0:06"]);
  });

  test("FormRow: a labelled row, with its actions apart", () => {
    render(FormRow, { props: { label: "Source", children: text("CARD_A"), aside: text("Choose…") } });
    const row = screen.getByRole("group", { name: "Source" });
    expect(row.textContent).toContain("CARD_A");
    expect(row.lastElementChild?.textContent).toBe("Choose…");
  });

  test("Select: its label can be hidden when a row already names it", () => {
    render(Select, { props: { label: "Profile", hideLabel: true, value: "", options: [{ value: "", label: "None" }], onChange: () => {} } });
    const label = screen.getByText("Profile");
    expect(label.className).toContain("visually-hidden");
    screen.getByLabelText("Profile");
  });

  test("ScreenHeader takes focus, so a new screen is announced by its title", () => {
    render(ScreenHeader, { props: { title: "Summary" } });
    expect(document.activeElement).toBe(screen.getByRole("heading", { name: "Summary" }));
  });

  test("TabBar: the kinds of job as tabs, the selected one, and what goes on the right", async () => {
    const onSelect = vi.fn();
    render(TabBar, {
      props: {
        items: [{ id: "copy", label: "Copy" }, { id: "mirror", label: "Mirror" }],
        selected: "copy",
        onSelect,
        trailing: text("Settings"),
      },
    });
    screen.getByText("Settings");
    const nav = screen.getByRole("navigation", { name: "Sections" });
    expect(within(nav).getByRole("button", { name: "Copy" }).getAttribute("aria-current")).toBe("page");
    await fireEvent.click(within(nav).getByRole("button", { name: "Mirror" }));
    expect(onSelect).toHaveBeenCalledWith("mirror");
  });

  test("TabBar: the Queue is apart from the tabs, with its count, and marked while open", async () => {
    const onSelect = vi.fn();
    const props = { items: [{ id: "copy", label: "Copy" }], selected: "copy", onSelect, queue: { count: 3 } };
    const { rerender } = render(TabBar, { props });
    const nav = screen.getByRole("navigation", { name: "Sections" });
    expect(within(nav).queryByRole("button", { name: /Queue/ })).toBeNull();
    const queue = screen.getByRole("button", { name: "Queue, 3 jobs" });
    expect(queue.getAttribute("aria-current")).toBeNull();
    await fireEvent.click(queue);
    expect(onSelect).toHaveBeenCalledWith("queue");
    await rerender({ ...props, selected: "queue", queue: { count: 1 } });
    expect(screen.getByRole("button", { name: "Queue, 1 job" }).getAttribute("aria-current")).toBe("page");
    await rerender({ ...props, queue: { count: 0 } });
    screen.getByRole("button", { name: "Queue" });
  });

  test("Hint: a term or an ⓘ mark explains itself on hover and keyboard focus", () => {
    render(Hint, { props: { text: "Files under 8 MB.", children: text("Small files") } });
    const term = screen.getByText("Small files");
    expect(term.closest("[tabindex='0']")).not.toBeNull();
    expect(hintOf(term)).toBe("Files under 8 MB.");
    render(Hint, { props: { text: "Reads every copy back.", label: "About verifying" } });
    const mark = screen.getByRole("button", { name: "About verifying" });
    expect(hintOf(mark)).toBe("Reads every copy back.");
  });

  test("Stats: a figure can explain itself", () => {
    render(Stats, { props: { items: ["3 files", { text: "2 not started", hint: "The job stopped first." }] } });
    expect(hintOf(screen.getByText("2 not started"))).toBe("The job stopped first.");
  });

  test("FormRow: a label can explain itself", () => {
    render(FormRow, { props: { label: "Existing files", hint: "Same name, different file.", children: text("x") } });
    expect(hintOf(screen.getByText("Existing files"))).toBe("Same name, different file.");
  });
});
