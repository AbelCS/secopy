<script lang="ts">
  // The app's screens: set up a copy, follow it, read the summary (RFD §5.2–§5.4).
  import { onMount } from "svelte";
  import { provideApi, tauriApi, type Api } from "./lib/api";
  import { stopMessage } from "./lib/stopping";
  import { notificationFor } from "./lib/summaryText";
  import type { Profile, ProgressView, SessionView, Settings, SummaryView } from "./lib/bindings";
  import JobProgress from "./components/JobProgress.svelte";
  import ProfilesScreen from "./components/ProfilesScreen.svelte";
  import SettingsScreen from "./components/SettingsScreen.svelte";
  import Button from "./lib/ui/Button.svelte";
  import Notice from "./lib/ui/Notice.svelte";
  import Setup from "./components/Setup.svelte";
  import Summary from "./components/Summary.svelte";

  let { api = tauriApi }: { api?: Api } = $props();
  // The app talks to one Api for its whole life; tests pass a fake one.
  // svelte-ignore state_referenced_locally
  provideApi(api);

  let screen: "setup" | "progress" | "summary" | "settings" | "profiles" = $state("setup");
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
  let settings: Settings = $state({ writeChecksumFile: true, showSystemCount: true, reportNextToChecksum: false, notifyWhenDone: true });
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

  /** Settings or Profiles, over the setup or summary screen; never during a copy. */
  function open(next: "settings" | "profiles") {
    if (screen !== "setup" && screen !== "summary") return;
    back = screen;
    screen = next;
  }

  const openSettings = () => open("settings");

  function saveMode(v: boolean) {
    void api.setMode(v).catch(() => {}); // remembered for next time; not worth an error
  }

  async function finish() {
    summary = (await run(() => api.jobSummary())) ?? null;
    if (!summary) return;
    screen = "summary";
    if (settings.notifyWhenDone && !api.windowFocused()) {
      const { title, body } = notificationFor(summary);
      void api.notify(title, body).catch(() => {}); // a notification is a courtesy, never an error
    }
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
      // Settings can't change during a copy, so these are the running job's.
      const stop = await api.confirm(stopMessage(settings.writeChecksumFile), "Stop copying and quit?");
      if (!stop) prevent();
    });
    return () => {
      unlisten.then((stop) => stop());
      unlistenSettings.then((stop) => stop());
    };
  });
</script>

<!-- App-wide messages, shown at the top of the screen's content. -->
{#snippet banner()}
  {#if error}<Notice tone="danger">{error}</Notice>{/if}
  {#if warnings.length > 0}
    <Notice tone="warning">
      {#each warnings as w (w)}<p class="warning">{w}</p>{/each}
      <Button variant="link" onclick={() => (warnings = [])}>Dismiss</Button>
    </Notice>
  {/if}
{/snippet}

{#if screen === "setup"}
  <Setup
    bind:view
    bind:verify
    {profiles}
    {settings}
    {recent}
    {banner}
    onStart={start}
    onProfiles={(p) => (profiles = p)}
    onManageProfiles={() => open("profiles")}
    onMode={saveMode}
    onSettings={openSettings}
  />
{:else if screen === "progress" && progress}
  <JobProgress {progress} checksumFile={settings.writeChecksumFile} {banner} />
{:else if screen === "summary" && summary}
  <Summary {summary} {banner} onRetry={retry} onNewCopy={newCopy} onSettings={openSettings} />
{:else if screen === "settings"}
  <SettingsScreen {settings} onSettings={(s) => (settings = s)} onDone={() => (screen = back)} />
{:else if screen === "profiles"}
  <ProfilesScreen
    {profiles}
    onProfiles={(p) => (profiles = p)}
    onView={(v) => (view = v)}
    onDone={() => (screen = back)}
  />
{/if}

<style>
  .warning {
    margin: 0 0 var(--space-1);
  }
</style>
