<script lang="ts">
  // The app's screens: set up a copy, follow it, read the summary (RFD §5.2–§5.4), and the
  // queue (§5.7), with the sections in a sidebar.
  import { onMount } from "svelte";
  import { provideApi, tauriApi, type Api } from "./lib/api";
  import { stopMessage } from "./lib/stopping";
  import { notificationFor } from "./lib/summaryText";
  import type { Profile, ProgressView, QueueView, SessionView, Settings, SummaryView } from "./lib/bindings";
  import JobProgress from "./components/JobProgress.svelte";
  import ProfilesScreen from "./components/ProfilesScreen.svelte";
  import SettingsScreen from "./components/SettingsScreen.svelte";
  import Button from "./lib/ui/Button.svelte";
  import Notice from "./lib/ui/Notice.svelte";
  import AppShell from "./lib/ui/AppShell.svelte";
  import ScreenHeader from "./lib/ui/ScreenHeader.svelte";
  import Sidebar from "./lib/ui/Sidebar.svelte";
  import Setup from "./components/Setup.svelte";
  import Summary from "./components/Summary.svelte";

  let { api = tauriApi }: { api?: Api } = $props();
  // The app talks to one Api for its whole life; tests pass a fake one.
  // svelte-ignore state_referenced_locally
  provideApi(api);

  type Screen = "setup" | "progress" | "summary" | "settings" | "profiles" | "queue";
  let screen = $state<Screen>("setup");
  /** Where Settings and Profiles go back to. */
  let back: "setup" | "summary" | "queue" = "setup";
  let queue: QueueView = $state({ jobs: [], onFailure: "continue", running: false });
  /** The Copy section's screen to return to: New copy, or the last summary. */
  let copyScreen: "setup" | "summary" = "setup";
  $effect(() => {
    if (screen === "setup" || screen === "summary") copyScreen = screen;
  });
  const section = $derived(screen === "queue" ? "queue" : "copy");
  /** The sidebar shows on the sections' own screens; not while jobs run, nor on Settings. */
  const showSidebar = $derived(screen === "setup" || screen === "summary" || screen === "queue");

  function go(next: "copy" | "queue") {
    if (!showSidebar) return;
    screen = next === "queue" ? "queue" : copyScreen;
  }
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
    stale: false,
  });
  let verify = $state(true);
  let profiles: Profile[] = $state([]);
  let settings: Settings = $state({ writeChecksumFile: true, showSystemCount: true, reportNextToChecksum: false, notifyWhenDone: true });
  let recent: string[] = $state([]);
  let progress: ProgressView | null = $state(null);
  let summary: SummaryView | null = $state(null);
  let error: string | null = $state(null);
  let setupScreen: Setup | undefined = $state();
  let progressScreen: JobProgress | undefined = $state();
  /** Start is enabled on New copy. */
  let setupReady = $state(false);

  // The File menu offers only what applies here (spec §3).
  $effect(() => {
    const copying = screen === "progress" && progress?.phase !== "done";
    void api.setMenuState(screen === "setup", screen === "setup" && setupReady, copying).catch(() => {});
  });

  function onMenu(item: string) {
    if (item === "choose-source" && screen === "setup") void setupScreen?.chooseSource();
    else if (item === "choose-destination" && screen === "setup") void setupScreen?.chooseDestination();
    else if (item === "start-copy" && screen === "setup") setupScreen?.startIfReady();
    else if (item === "cancel-copy" && screen === "progress") void progressScreen?.cancel();
    else if (item === "show-copy") go("copy");
    else if (item === "show-queue") go("queue");
  }

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

  /** Settings or Profiles, over a section's screen; never during a copy. */
  function open(next: "settings" | "profiles") {
    if (screen !== "setup" && screen !== "summary" && screen !== "queue") return;
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
      // The profile last used is loaded again when its source is there (FR-36).
      if (start.lastProfile) {
        const id = start.lastProfile;
        void run(() => api.selectProfile(id)).then((next) => {
          if (next) view = next;
        });
      }
    });
    void run(() => api.queue()).then((q) => {
      if (q) queue = q;
    });
    const unlistenSettings = api.onOpenSettings(openSettings);
    const unlistenMenu = api.onMenu(onMenu);
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
      unlistenMenu.then((stop) => stop());
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

<div class="app" class:with-sidebar={showSidebar}>
  {#if showSidebar}
    <Sidebar
      items={[
        { id: "copy", label: "Copy" },
        { id: "queue", label: "Queue", count: queue.jobs.length },
      ]}
      selected={section}
      onSelect={(id) => go(id as "copy" | "queue")}
    />
  {/if}
  <div class="screen">
    {#if screen === "setup"}
      <Setup
        bind:this={setupScreen}
        bind:ready={setupReady}
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
        onQueued={(q) => (queue = q)}
      />
    {:else if screen === "progress" && progress}
      <JobProgress bind:this={progressScreen} {progress} checksumFile={settings.writeChecksumFile} {banner} />
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
    {:else if screen === "queue"}
      <AppShell>
        {#snippet header()}<ScreenHeader title="Queue" />{/snippet}
        {@render banner()}
      </AppShell>
    {/if}
  </div>
</div>

<style>
  .app.with-sidebar {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr);
  }

  .screen {
    min-width: 0;
  }

  .warning {
    margin: 0 0 var(--space-1);
  }
</style>
