import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import { apiContext } from "../lib/api";
import type { SessionView } from "../lib/bindings";
import { destinationView, fakeApi, readyView, sessionView, sourceView } from "../test/fake-api";
import Setup from "./Setup.svelte";

function setup(view: SessionView = sessionView(), answer: SessionView = view) {
  const { api, state } = fakeApi(answer);
  const started: number[] = [];
  const result = render(Setup, {
    props: { view, verify: true, onStart: () => started.push(1) },
    context: apiContext(api),
  });
  return { api, state, started, ...result };
}

const start = () => screen.getByRole("button", { name: /copy & verify|start copy/i });

describe("Setup", () => {
  test("Start stays disabled until there is something to copy", () => {
    setup();
    expect(start()).toHaveProperty("disabled", true);
  });

  test("a ready session enables Start and says what it will do", async () => {
    const { started } = setup(readyView());
    const button = screen.getByRole("button", { name: "Copy & verify 1,284 files · 212.4 GB" });
    expect(button).toHaveProperty("disabled", false);
    await fireEvent.click(button);
    expect(started).toHaveLength(1);
  });

  test("Choose folder scans the picked folder", async () => {
    const { api } = setup(sessionView(), readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Choose folder…" }));
    await waitFor(() => expect(api.scanSource).toHaveBeenCalledWith(["/Volumes/CARD/DCIM"], false));
    await screen.findByText("1,284 files · 212.4 GB · 37 hidden items skipped");
  });

  test("switching to 'only what's inside' rescans with contents only", async () => {
    const { api } = setup(sessionView(), readyView());
    await fireEvent.click(screen.getByRole("button", { name: "Choose folder…" }));
    await fireEvent.click(await screen.findByLabelText("Copy only what's inside"));
    await waitFor(() => expect(api.scanSource).toHaveBeenLastCalledWith(["/Volumes/CARD/DCIM"], true));
  });

  test("a chip turned off narrows the filter; All selects everything again", async () => {
    const { api } = setup(readyView());
    await fireEvent.click(screen.getByRole("button", { name: /\.xml/ }));
    expect(api.setFilter).toHaveBeenLastCalledWith(["mov", "wav"]);
    await fireEvent.click(screen.getByRole("button", { name: "All" }));
    expect(api.setFilter).toHaveBeenLastCalledWith(null);
  });

  test("a Finder drop on FROM scans it, a drop on TO sets the destination", async () => {
    const { api, state, container } = setup(readyView());
    await waitFor(() => expect(state.drop).not.toBeNull());
    state.drop!(["/Volumes/CARD2"], container.querySelector('[data-drop="from"] p'));
    await waitFor(() => expect(api.scanSource).toHaveBeenCalledWith(["/Volumes/CARD2"], false));
    state.drop!(["/Volumes/Backup"], container.querySelector('[data-drop="to"]'));
    await waitFor(() => expect(api.setDestination).toHaveBeenCalledWith("/Volumes/Backup"));
  });

  test("a blocker is shown and Start stays disabled", () => {
    setup(
      readyView({
        destination: destinationView({ blocker: "The destination is the source folder or inside it" }),
        plan: null,
      }),
    );
    expect(screen.getByRole("alert").textContent).toContain("inside it");
    expect(start()).toHaveProperty("disabled", true);
  });

  test("not enough space blocks Start", () => {
    setup(
      readyView({
        plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, blocker: "Not enough free space" },
      }),
    );
    expect(screen.getByRole("alert").textContent).toBe("Not enough free space");
    expect(screen.getByRole("button", { name: /copy & verify/i })).toHaveProperty("disabled", true);
  });

  test("a non-empty copy root is a warning, not a block", () => {
    setup(readyView({ destination: destinationView({ existingItems: 1204 }) }));
    screen.getByText(/already contains 1,204 items/);
    expect(start()).toHaveProperty("disabled", false);
  });

  test("files that will fail are listed with their reasons", () => {
    setup(
      readyView({
        destination: destinationView({
          problems: [{ path: "DCIM/a:b.mov", reason: "the name contains \":\", which this drive doesn't allow" }],
          problemCount: 3,
        }),
      }),
    );
    screen.getByText("3 files will fail");
    screen.getByText("DCIM/a:b.mov");
    screen.getByText("and 2 more");
  });

  test("identical files, different files and leftovers are explained", async () => {
    const { api } = setup(
      readyView({ destination: destinationView({ identical: 284, differs: 12, stalePartials: 2 }) }),
    );
    screen.getByText("284 identical files will be skipped (not checked).");
    screen.getByText("2 unfinished files from an interrupted copy will be replaced.");
    expect(screen.getByLabelText("Keep both")).toHaveProperty("checked", true);
    await fireEvent.click(screen.getByLabelText("Overwrite"));
    expect(api.setConflicts).toHaveBeenCalledWith("overwrite");
  });

  test("Copy mode changes the Start label", async () => {
    setup(readyView());
    await fireEvent.click(screen.getByLabelText("Copy"));
    screen.getByRole("button", { name: "Copy 1,284 files · 212.4 GB" });
  });

  test("a scan in progress says so, and Start waits for it", async () => {
    const { api } = setup(readyView());
    let finishScan = (_v: SessionView) => {};
    api.scanSource.mockImplementationOnce(() => new Promise((resolve) => (finishScan = resolve)));
    await fireEvent.click(screen.getByRole("button", { name: "Choose folder…" }));
    await screen.findByText("Scanning…");
    expect(start()).toHaveProperty("disabled", true);
    // A quicker destination check finishing meanwhile doesn't enable Start.
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(api.setDestination).toHaveBeenCalled());
    await Promise.resolve();
    expect(start()).toHaveProperty("disabled", true);
    finishScan(readyView());
    await waitFor(() => expect(screen.queryByText("Scanning…")).toBeNull());
    expect(start()).toHaveProperty("disabled", false);
  });

  test("a scan replaced by a newer one doesn't change the view", async () => {
    const { api } = setup(readyView(), sessionView({ stale: true }));
    await fireEvent.click(screen.getByRole("button", { name: "Choose folder…" }));
    await waitFor(() => expect(api.scanSource).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByText("Scanning…")).toBeNull());
    screen.getByText("/Volumes/CARD/DCIM");
    expect(start()).toHaveProperty("disabled", false);
  });

  test("the folder choice rescans the source's folder", async () => {
    const { api } = setup(readyView());
    await fireEvent.click(screen.getByLabelText("Copy only what's inside"));
    await waitFor(() => expect(api.scanSource).toHaveBeenLastCalledWith(["/Volumes/CARD/DCIM"], true));
  });

  test("copying only what's inside still names the folder", () => {
    setup(readyView({ source: sourceView({ contentsOnly: true, rootDir: null }) }));
    screen.getByLabelText("Copy the folder “DCIM” itself");
  });

  test("a retry has no folder choice or filter to change", () => {
    setup(readyView({ source: sourceView({ isRetry: true, label: "Retry: 3 failed files" }) }));
    screen.getByText("Retry: 3 failed files");
    expect(screen.queryByLabelText("Copy only what's inside")).toBeNull();
    expect(screen.queryByRole("button", { name: /\.xml/ })).toBeNull();
  });

  test("a command error is shown where it happened", async () => {
    const { api } = setup(sessionView({ source: sourceView() }));
    api.setDestination.mockRejectedValueOnce(new Error("Can't write to the destination"));
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText("Can't write to the destination");
  });
});
