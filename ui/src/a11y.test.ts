import { render } from "@testing-library/svelte";
import axe from "axe-core";
import { describe, expect, test } from "vitest";
import App from "./App.svelte";
import JobProgress from "./components/JobProgress.svelte";
import ProfilesScreen from "./components/ProfilesScreen.svelte";
import QueueScreen from "./components/QueueScreen.svelte";
import SettingsScreen from "./components/SettingsScreen.svelte";
import Summary from "./components/Summary.svelte";
import { apiContext } from "./lib/api";
import { fakeApi, profile, progressView, readyView, settingsView, summaryView, queuedJob, queueView } from "./test/fake-api";

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
    const props = { summary: summaryView(), onRetry: () => {}, onNewCopy: () => {}, onSettings: () => {} };
    const { container } = render(Summary, { props, context: apiContext(api) });
    expect(await violations(container)).toEqual([]);
  });

  test("Settings", async () => {
    const { api } = fakeApi();
    const { container } = render(SettingsScreen, { props: { settings: settingsView(), onSettings: () => {}, onDone: () => {} }, context: apiContext(api) });
    expect(await violations(container)).toEqual([]);
  });

  test("Profiles, with and without profiles", async () => {
    for (const profiles of [[profile()], []]) {
      const { api } = fakeApi();
      const { container, unmount } = render(ProfilesScreen, {
        props: { profiles, onProfiles: () => {}, onView: () => {}, onDone: () => {} },
        context: apiContext(api),
      });
      expect(await violations(container)).toEqual([]);
      unmount();
    }
  });

  test("Queue", async () => {
    const { api } = fakeApi();
    const { container } = render(QueueScreen, {
      props: { queue: queueView({ jobs: [queuedJob(), queuedJob({ lastError: "CARD isn't connected." })] }), onQueue: () => {}, onRun: () => {} },
      context: apiContext(api),
    });
    expect(await violations(container)).toEqual([]);
  });
});
