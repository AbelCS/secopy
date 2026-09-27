<script lang="ts">
  // The app's screens: set up a copy, follow it, read the summary (RFD §5.2–§5.4).
  import { onMount } from "svelte";
  import { provideApi, tauriApi, type Api } from "./lib/api";
  import type { Profile, ProgressView, SessionView, Settings, SummaryView } from "./lib/bindings";
  import JobProgress from "./components/JobProgress.svelte";
  import SettingsScreen from "./components/SettingsScreen.svelte";
  import Setup from "./components/Setup.svelte";
  import Summary from "./components/Summary.svelte";

  let { api = tauriApi }: { api?: Api } = $props();
  // The app talks to one Api for its whole life; tests pass a fake one.
  // svelte-ignore state_referenced_locally
  provideApi(api);

  let screen: "setup" | "progress" | "summary" | "settings" = $state("setup");
  /** Where Settings' Done goes back to. */
  let back: "setup" | "summary" = "setup";
  /** Saved files that couldn't be read, shown once. */
  let warnings: string[] = $state([]);
  let view: SessionView = $state({
    source: null,
    selectedFiles: 0,
    selectedBytes: 0,
    destination: null,
    conflicts: "keepBoth",
    plan: null,
    profileId: null,
    profileChanged: false,
    pickProblem: null,
    suggestedFolder: "",
    stale: false,
  });
  let verify = $state(true);
  let profiles: Profile[] = $state([]);
  let settings: Settings = $state({ writeChecksumFile: true, showHiddenCount: true, reportNextToChecksum: false });
  let recent: string[] = $state([]);
  let progress: ProgressView | null = $state(null);
  let summary: SummaryView | null = $state(null);
  let error: string | null = $state(null);

  /** What the progress view shows before the first update arrives. */
  const waiting = (): ProgressView => ({
    phase: "copying",
    elapsedMs: 0,
    paused: false,
    verify,
    totalFiles: view.selectedFiles,
    totalBytes: view.plan?.bytesToWrite ?? 0,
    copiedBytes: 0,
    verifiedBytes: 0,
    filesDone: 0,
    filesSkipped: 0,
    filesFailed: 0,
    active: [],
    smallFiles: null,
    fatal: null,
  });

  async function run<T>(action: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await action();
      error = null;
      return result;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      return undefined;
    }
  }

  async function start() {
    progress = waiting();
    screen = "progress";
    const started = await run(() =>
      api.startJob(verify, (p) => {
        progress = p;
        if (p.phase === "done") void finish();
      }),
    );
    if (started === undefined) screen = "setup";
    else recent = (await run(() => api.recentDestinations())) ?? recent;
  }

  function openSettings() {
    if (screen === "progress" || screen === "settings") return; // not during a copy
    back = screen;
    screen = "settings";
  }

  function saveMode(v: boolean) {
    void api.setMode(v).catch(() => {}); // remembered for next time; not worth an error
  }

  async function finish() {
    summary = (await run(() => api.jobSummary())) ?? null;
    if (summary) screen = "summary";
  }

  async function retry() {
    const next = await run(() => api.retryFailed());
    if (next) {
      view = next;
      screen = "setup";
    }
  }

  async function newCopy() {
    const next = await run(() => api.clearSource());
    if (next) {
      view = next;
      screen = "setup";
    }
  }

  onMount(() => {
    void run(() => api.appStart()).then((start) => {
      if (!start) return;
      view = start.session;
      settings = start.settings;
      profiles = start.profiles;
      verify = start.verify;
      recent = start.recentDestinations;
      warnings = start.warnings;
    });
    const unlistenSettings = api.onOpenSettings(openSettings);
    // Closing during a copy asks first; if closed anyway, the app stops the copy cleanly.
    const unlisten = api.onCloseRequested(async (prevent) => {
      if (!(await api.jobRunning())) return;
      const stop = await api.confirm(
        "Files already copied stay and are listed in the checksum file; the file in progress is removed.",
        "Stop copying and quit?",
      );
      if (!stop) prevent();
    });
    return () => {
      unlisten.then((stop) => stop());
      unlistenSettings.then((stop) => stop());
    };
  });
</script>

<main>
  <header>
    <h1>Secopy</h1>
    {#if screen === "setup" || screen === "summary"}
      <button type="button" class="gear" aria-label="Settings" onclick={openSettings}>⚙</button>
    {/if}
  </header>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if warnings.length > 0 && screen === "setup"}
    <div class="warnings" role="alert">
      {#each warnings as w (w)}<p>{w}</p>{/each}
      <button type="button" onclick={() => (warnings = [])}>Dismiss</button>
    </div>
  {/if}
  {#if screen === "setup"}
    <Setup
      bind:view
      bind:verify
      {profiles}
      {settings}
      {recent}
      onStart={start}
      onProfiles={(p) => (profiles = p)}
      onManageProfiles={openSettings}
      onMode={saveMode}
    />
  {:else if screen === "progress" && progress}
    <JobProgress {progress} />
  {:else if screen === "summary" && summary}
    <Summary {summary} onRetry={retry} onNewCopy={newCopy} />
  {:else if screen === "settings"}
    <SettingsScreen
      {settings}
      {profiles}
      onSettings={(s) => (settings = s)}
      onProfiles={(p) => (profiles = p)}
      onView={(v) => (view = v)}
      onDone={() => (screen = back)}
    />
  {/if}
</main>

<style>
  main {
    max-width: 960px;
    margin: 0 auto;
    padding: 20px 24px;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 14px;
  }

  h1 {
    margin: 0;
    font-size: 18px;
  }

  .warnings {
    color: var(--warning);
    margin-bottom: var(--gap);
  }

  .error {
    color: var(--danger);
  }
</style>
