import { fireEvent, render, screen } from "@testing-library/svelte";
import axe from "axe-core";
import { raw } from "./test/fake-api";
import { describe, expect, test } from "vitest";
import App from "./App.svelte";
import JobProgress from "./components/JobProgress.svelte";
import CopyPresetsScreen from "./components/CopyPresetsScreen.svelte";
import QueueScreen from "./components/QueueScreen.svelte";
import MirrorPreview from "./components/MirrorPreview.svelte";
import MirrorScreen from "./components/MirrorScreen.svelte";
import VerifyScreen from "./components/VerifyScreen.svelte";
import SettingsScreen from "./components/SettingsScreen.svelte";
import ImportScreen from "./components/ImportScreen.svelte";
import ExportDialog from "./components/ExportDialog.svelte";
import Summary from "./components/Summary.svelte";
import { apiContext } from "./lib/api";
import { fakeApi, copyPreset, progressView, readyView, settingsView, summaryView, queuedJob, queueView, mirrorPreset, mirrorPreview, importView } from "./test/fake-api";

async function violations(container: HTMLElement): Promise<string[]> {
  // Colours are checked by tokens.test.ts; the test DOM can't compute them.
  const result = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
  return result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`);
}

describe("accessibility (axe-core)", () => {
  test("New copy", async () => {
    const { api } = fakeApi(readyView());
    const { container } = render(App, { props: { api } });
    await new Promise((r) => setTimeout(r, 20));
    expect(await violations(container)).toEqual([]);
  });

  test("Copying", async () => {
    const { api } = fakeApi();
    const { container } = render(JobProgress, { props: { progress: progressView({ filesDone: 3 }) }, context: apiContext(api) });
    expect(await violations(container)).toEqual([]);
  });

  test("Summary", async () => {
    const { api } = fakeApi();
    const props = { summary: summaryView(), onRetry: () => {}, onNewCopy: () => {} };
    const { container } = render(Summary, { props, context: apiContext(api) });
    expect(await violations(container)).toEqual([]);
  });

  test("Settings, with Export… and Import…", async () => {
    const { api } = fakeApi();
    const { container } = render(SettingsScreen, {
      props: { settings: settingsView(), onSettings: () => {}, onExport: () => {}, onImport: () => {}, onDone: () => {} },
      context: apiContext(api),
    });
    expect(await violations(container)).toEqual([]);
  });

  test("Import and the export dialog", async () => {
    const { container } = render(ImportScreen, { props: { view: importView(), onImport: () => {}, onBack: () => {} } });
    expect(await violations(container)).toEqual([]);
    const dialog = render(ExportDialog, { props: { copyPresets: 2, mirrorPresets: 0, onExport: () => {}, onClose: () => {} } });
    expect(await violations(dialog.container)).toEqual([]);
  });

  test("Copy presets, with and without presets", async () => {
    for (const presets of [[copyPreset()], []]) {
      const { api } = fakeApi();
      const { container, unmount } = render(CopyPresetsScreen, {
        props: { presets, onPresets: () => {}, onView: () => {}, onDone: () => {} },
        context: apiContext(api),
      });
      expect(await violations(container)).toEqual([]);
      unmount();
    }
  });

  test("Queue", async () => {
    const { api } = fakeApi();
    const { container } = render(QueueScreen, {
      props: { queue: queueView({ jobs: [queuedJob(), queuedJob({ lastError: raw("CARD isn’t connected.") })] }), onQueue: () => {}, onRun: () => {} },
      context: apiContext(api),
    });
    expect(await violations(container)).toEqual([]);
  });

  test("Mirror", async () => {
    const { api } = fakeApi();
    const { container } = render(MirrorScreen, {
      props: { presets: [mirrorPreset()], onPresets: () => {}, onPreview: () => {}, onQueue: () => {} },
      context: apiContext(api),
    });
    expect(await violations(container)).toEqual([]);
  });

  test("Cancel's question", async () => {
    const { api } = fakeApi();
    const { container } = render(JobProgress, { props: { progress: progressView() }, context: apiContext(api) });
    await fireEvent.click(screen.getByRole("button", { name: "Cancel…" }));
    screen.getByRole("dialog");
    expect(await violations(container.ownerDocument.body)).toEqual([]);
  });

  test("Verify", async () => {
    const { api } = fakeApi();
    api.pickDirectory.mockResolvedValueOnce("/Volumes/Backup/Day01");
    const { container } = render(VerifyScreen, { props: { onStart: () => {}, onQueue: () => {} }, context: apiContext(api) });
    await fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByText(/checksum files/);
    expect(await violations(container)).toEqual([]);
  });

  test("Mirror preview", async () => {
    const { api } = fakeApi();
    const { container } = render(MirrorPreview, {
      props: { preview: mirrorPreview({ guard: raw("3 of the destination’s 3 files would be removed.") }), onRun: () => {}, onQueue: () => {}, onCancel: () => {} },
      context: apiContext(api),
    });
    expect(await violations(container)).toEqual([]);
  });
});
