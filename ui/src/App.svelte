<script lang="ts">
  // The app's screens: set up a copy, follow it, read the summary (RFD §5.2–§5.4), and the
  // queue (§5.7), with the sections in a sidebar.
  import { onMount } from "svelte";
  import { provideApi, tauriApi, type Api } from "./lib/api";
  import { stopMessage } from "./lib/stopping";
  import { notificationFor, queueNotification } from "./lib/summaryText";
  import type {
    Profile,
    ProgressView,
    MirrorPreset,
    MirrorPreviewView,
    QueueSummaryView,
    QueueView,
    SessionView,
    Settings,
    SummaryView,
  } from "./lib/bindings";
  import JobProgress from "./components/JobProgress.svelte";
  import ProfilesScreen from "./components/ProfilesScreen.svelte";
  import QueueScreen from "./components/QueueScreen.svelte";
  import QueueSummary from "./components/QueueSummary.svelte";
  import MirrorPreview from "./components/MirrorPreview.svelte";
  import MirrorScreen from "./components/MirrorScreen.svelte";
  import SettingsScreen from "./components/SettingsScreen.svelte";
  import Button from "./lib/ui/Button.svelte";
  import Notice from "./lib/ui/Notice.svelte";
  import Sidebar from "./lib/ui/Sidebar.svelte";
  import Setup from "./components/Setup.svelte";
  import Summary from "./components/Summary.svelte";

  let { api = tauriApi }: { api?: Api } = $props();
  // The app talks to one Api for its whole life; tests pass a fake one.
  // svelte-ignore state_referenced_locally
  provideApi(api);

  type Screen =
    | "setup"
    | "progress"
    | "summary"
    | "settings"
    | "profiles"
    | "queue"
    | "queue-summary"
    | "queue-job"
    | "mirror"
    | "mirror-preview"
    | "mirror-summary";
  let screen = $state<Screen>("setup");
  /** Where Settings and Profiles go back to. */
  let back: "setup" | "summary" | "queue" | "mirror" | "mirror-summary" = "setup";
  let queue: QueueView = $state({ jobs: [], onFailure: "continue", running: false });
  /** The Copy section's screen to return to: New copy, or the last summary. */
  let copyScreen: "setup" | "summary" = "setup";
  $effect(() => {
    if (screen === "setup" || screen === "summary") copyScreen = screen;
  });
  const queueScreens: Screen[] = ["queue", "queue-summary", "queue-job"];
  const mirrorScreens: Screen[] = ["mirror", "mirror-preview", "mirror-summary"];
  const section = $derived(
    queueScreens.includes(screen) ? "queue" : mirrorScreens.includes(screen) ? "mirror" : "copy",
  );
  /** The sidebar shows on the sections' own screens; not while jobs run, nor on Settings. */
  const showSidebar = $derived(
    screen === "setup" || screen === "summary" || queueScreens.includes(screen) || mirrorScreens.includes(screen),
  );
  let mirrorPresets: MirrorPreset[] = $state([]);
  /** The preview the Mirror section's Preview… worked out; Run mirror runs it. */
  let mirrorPreview: MirrorPreviewView | null = $state(null);
  /** The last mirror run's summary, shown in the Mirror section. */
  let mirrorSummary: SummaryView | null = $state(null);
  /** The Mirror section's screen to return to: the presets, or the last run's summary. */
  let mirrorScreen: "mirror" | "mirror-summary" = "mirror";
  $effect(() => {
    if (screen === "mirror" || screen === "mirror-summary") mirrorScreen = screen;
  });
  /** The queue run in progress: this job's place, the number of jobs, and whether it is still
   * being checked; `kind` is the job's ("copy", "mirror") once it starts. */
  let queueRun: {
    index: number;
    count: number;
    checking: boolean;
    kind: string | null;
    /** A queued mirror's deep check: files compared, of how many. */
    compared?: { done: number; total: number };
  } | null = $state(null);
  let queueSummary: QueueSummaryView | null = $state(null);
  /** The job running is a mirror. */
  let mirrorRunning = $state(false);
  $effect(() => {
    if (screen !== "progress") mirrorRunning = false;
  });
  /** The job of the queue summary whose own summary is open. */
  let openedJob: number | null = $state(null);

  function go(next: "copy" | "mirror" | "queue") {
    if (!showSidebar) return;
    screen = next === "queue" ? "queue" : next === "mirror" ? mirrorScreen : copyScreen;
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
    else if (item === "show-mirror") go("mirror");
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
    removing: 0,
    archiving: false,
  });

  /** A queue job before its first update: none of New copy's figures, which aren't its own. */
  const queueWaiting = (jobVerifies: boolean): ProgressView => ({
    ...waiting(),
    verify: jobVerifies,
    totalFiles: 0,
    totalBytes: 0,
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

  /** A new job replaces the backend's last one: no old mirror summary acts on it. */
  function forgetMirrorSummary() {
    mirrorSummary = null;
    mirrorScreen = "mirror";
  }

  async function start() {
    forgetMirrorSummary();
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
    if (screen !== "setup" && screen !== "summary" && screen !== "queue" && screen !== "mirror" && screen !== "mirror-summary")
      return;
    back = screen;
    screen = next;
  }

  const openSettings = () => open("settings");

  function saveMode(v: boolean) {
    void api.setMode(v).catch(() => {}); // remembered for next time; not worth an error
  }

  async function finish() {
    const done = (await run(() => api.jobSummary())) ?? null;
    if (!done) return;
    if (done.mirror) {
      mirrorSummary = done;
      screen = "mirror-summary";
    } else {
      summary = done;
      screen = "summary";
    }
    if (settings.notifyWhenDone && !api.windowFocused()) {
      const { title, body } = notificationFor(done);
      void api.notify(title, body).catch(() => {}); // a notification is a courtesy, never an error
    }
  }

  /** Runs the previewed mirror (FR-47): the Mirroring screen, then its summary. */
  async function runMirror(preview: MirrorPreviewView) {
    // The mirror replaces the backend's last copy: no old Summary acts on it.
    summary = null;
    copyScreen = "setup";
    progress = {
      ...waiting(),
      verify: true,
      totalFiles: preview.newFiles + preview.changedFiles,
      totalBytes: preview.newBytes + preview.changedBytes,
    };
    mirrorRunning = true;
    screen = "progress";
    const started = await run(() =>
      api.runMirror(preview.presetId, (p) => {
        progress = p;
        if (p.phase === "done") void finish();
      }),
    );
    if (started === undefined) screen = "mirror-preview";
  }

  /** Runs the queue: one Copying screen per job, then the queue summary (FR-40..FR-43). */
  async function runQueue() {
    // The queue's jobs replace the backend's last copy: the Copy section starts afresh, so no
    // old Summary acts on a queue job.
    summary = null;
    copyScreen = "setup";
    forgetMirrorSummary();
    queueRun = { index: 0, count: queue.jobs.length, checking: true, kind: null };
    progress = queueWaiting(false);
    screen = "progress";
    const started = await run(() =>
      api.runQueue((e) => {
        if (e.type === "jobChecking") {
          queueRun = { index: e.index, count: e.count, checking: true, kind: null };
          progress = queueWaiting(false);
        } else if (e.type === "compared") {
          if (queueRun) queueRun = { ...queueRun, compared: { done: e.done, total: e.total } };
        } else if (e.type === "jobStarted") {
          queueRun = { index: e.index, count: e.count, checking: false, kind: e.job.kind };
          progress = queueWaiting(e.job.kind === "mirror" || e.job.verify);
        } else if (e.type === "progress") {
          progress = e.view;
        } else {
          queueRun = null;
          queueSummary = e.summary;
          screen = "queue-summary";
          void run(() => api.queue()).then((q) => {
            if (q) queue = q;
          });
          if (settings.notifyWhenDone && !api.windowFocused()) {
            const { title, body } = queueNotification(e.summary);
            void api.notify(title, body).catch(() => {}); // a courtesy, never an error
          }
        }
      }),
    );
    if (started === undefined) {
      queueRun = null;
      screen = "queue";
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
    void run(() => api.mirrorPresets()).then((m) => {
      if (m) mirrorPresets = m;
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
        { id: "mirror", label: "Mirror" },
        { id: "queue", label: "Queue", count: queue.jobs.length },
      ]}
      selected={section}
      onSelect={(id) => go(id as "copy" | "mirror" | "queue")}
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
      <!-- A new screen for each queue job: its own file list, speed and time left. -->
      {#key queueRun?.index}
        <JobProgress
          bind:this={progressScreen}
          {progress}
          checksumFile={settings.writeChecksumFile}
          {banner}
          queue={queueRun ?? undefined}
          checking={queueRun?.checking ?? false}
          compared={queueRun?.compared}
          title={mirrorRunning || queueRun?.kind === "mirror" ? "Mirroring" : undefined}
        />
      {/key}
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
    {:else if screen === "mirror"}
      <MirrorScreen
        presets={mirrorPresets}
        {banner}
        onPresets={(p) => (mirrorPresets = p)}
        onPreview={(v) => {
          // Only if still here: a long preview may end after you went elsewhere.
          if (screen !== "mirror") return;
          mirrorPreview = v;
          screen = "mirror-preview";
        }}
        onQueue={(q) => (queue = q)}
        onSettings={openSettings}
      />
    {:else if screen === "mirror-preview" && mirrorPreview}
      <MirrorPreview
        preview={mirrorPreview}
        onRun={() => mirrorPreview && runMirror(mirrorPreview)}
        onQueue={(q) => (queue = q)}
        onCancel={() => (screen = "mirror")}
      />
    {:else if screen === "mirror-summary" && mirrorSummary}
      <Summary summary={mirrorSummary} {banner} onDone={() => (screen = "mirror")} onSettings={openSettings} />
    {:else if screen === "queue"}
      <QueueScreen {queue} {banner} onQueue={(q) => (queue = q)} onRun={runQueue} onSettings={openSettings} />
    {:else if screen === "queue-summary" && queueSummary}
      <QueueSummary
        summary={queueSummary}
        onOpen={(i) => {
          openedJob = i;
          screen = "queue-job";
        }}
        onDone={() => (screen = "queue")}
      />
    {:else if screen === "queue-job" && queueSummary && openedJob !== null && queueSummary.results[openedJob]?.summary}
      <Summary
        summary={queueSummary.results[openedJob].summary!}
        {banner}
        queueIndex={openedJob}
        onBack={() => (screen = "queue-summary")}
      />
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
