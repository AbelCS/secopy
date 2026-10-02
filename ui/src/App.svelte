<script lang="ts">
  // The app's screens: set up a copy, follow it, read the summary (RFD §5.2–§5.4), and the
  // queue (§5.7), with the sections as tabs at the top.
  import { messageOf } from "./lib/format";
  import { onMount } from "svelte";
  import { provideApi, tauriApi, type Api } from "./lib/api";
  import { finishingMessage, notStarted, quitQuestion, stopMessage, type JobKind } from "./lib/stopping";
  import { t } from "./lib/i18n";
  import { say } from "./lib/message";
  import { newest } from "./lib/session";
  import { notificationFor, queueNotification } from "./lib/summaryText";
  import type {
    CopyPreset,
    ExportWhat,
    ImportChoices,
    ImportView,
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
  import CopyPresetsScreen from "./components/CopyPresetsScreen.svelte";
  import ConfirmHost from "./components/ConfirmHost.svelte";
  import { asking } from "./lib/confirm.svelte";
  import ExportDialog from "./components/ExportDialog.svelte";
  import ImportScreen from "./components/ImportScreen.svelte";
  import QueueScreen from "./components/QueueScreen.svelte";
  import QueueSummary from "./components/QueueSummary.svelte";
  import MirrorPreview from "./components/MirrorPreview.svelte";
  import MirrorScreen from "./components/MirrorScreen.svelte";
  import VerifyScreen from "./components/VerifyScreen.svelte";
  import SettingsScreen from "./components/SettingsScreen.svelte";
  import Button from "./lib/ui/Button.svelte";
  import Notice from "./lib/ui/Notice.svelte";
  import TabBar from "./lib/ui/TabBar.svelte";
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
    | "copy-presets"
    | "queue"
    | "queue-summary"
    | "queue-job"
    | "mirror"
    | "mirror-preview"
    | "mirror-summary"
    | "verify"
    | "verify-summary"
    | "import";
  let screen = $state<Screen>("setup");
  /** Where Settings and Copy presets go back to. */
  let back: "setup" | "summary" | "queue" | "mirror" | "mirror-summary" | "verify" | "verify-summary" = "setup";
  let queue: QueueView = $state({ jobs: [], onFailure: "continue" });
  /** The Copy section's screen to return to: New copy, or the last summary. */
  let copyScreen: "setup" | "summary" = "setup";
  $effect(() => {
    if (screen === "setup" || screen === "summary") copyScreen = screen;
  });
  const queueScreens: Screen[] = ["queue", "queue-summary", "queue-job"];
  const mirrorScreens: Screen[] = ["mirror", "mirror-preview", "mirror-summary"];
  const verifyScreens: Screen[] = ["verify", "verify-summary"];
  const section = $derived(
    queueScreens.includes(screen)
      ? "queue"
      : mirrorScreens.includes(screen)
        ? "mirror"
        : verifyScreens.includes(screen)
          ? "verify"
          : "copy",
  );
  /** The tabs show on the sections' own screens; not while jobs run, nor on Settings. */
  const showTabs = $derived(
    screen === "setup" ||
      screen === "summary" ||
      queueScreens.includes(screen) ||
      mirrorScreens.includes(screen) ||
      verifyScreens.includes(screen),
  );
  let mirrorPresets: MirrorPreset[] = $state([]);
  /** The preview the Mirror section's Preview… worked out; the preview's Start runs it. */
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
  /** The job running is a check (Verify). */
  let checkRunning = $state(false);
  /** The last check's summary, shown on the Verify tab. */
  let verifySummary: SummaryView | null = $state(null);
  let verifyScreen: "verify" | "verify-summary" = "verify";
  $effect(() => {
    if (screen === "verify" || screen === "verify-summary") verifyScreen = screen;
  });
  $effect(() => {
    if (screen !== "progress") mirrorRunning = checkRunning = false;
  });
  /** What the running job does, for its screen and the stop questions. */
  const runningKind: JobKind = $derived.by(() =>
    checkRunning || queueRun?.kind === "check"
      ? "check"
      : mirrorRunning || queueRun?.kind === "mirror"
        ? "mirror"
        : "copy",
  );
  /** The job of the queue summary whose own summary is open. */
  let openedJob: number | null = $state(null);

  /** The mirror screen with changes that aren't saved: asked about before leaving it (#117). */
  async function mayLeaveMirror(): Promise<boolean> {
    return screen !== "mirror" || !mirrorsScreen || (await mirrorsScreen.mayLeave());
  }

  async function go(next: "copy" | "mirror" | "verify" | "queue") {
    if (!showTabs) return;
    if (next !== "mirror" && !(await mayLeaveMirror())) return;
    screen =
      next === "queue" ? "queue" : next === "mirror" ? mirrorScreen : next === "verify" ? verifyScreen : copyScreen;
  }
  /** Saved files that couldn't be read, shown once. */
  let warnings: string[] = $state([]);
  let view: SessionView = $state({
    revision: 0,
    source: null,
    selectedFiles: 0,
    selectedBytes: 0,
    destination: null,
    conflicts: "keepBoth",
    plan: null,
    presetId: null,
    presetChanged: false,
    pickProblem: null,
    stale: false,
    jobIgnore: [],
  });
  let verify = $state(true);
  let copyPresets: CopyPreset[] = $state([]);
  let settings: Settings = $state({ writeChecksumFile: true, showSystemCount: true, reportNextToChecksum: false, notifyWhenDone: true, keepInMenuBar: true, writeMhl: false, ignore: [".DS_Store", "._*", "Thumbs.db"] });
  let recent: string[] = $state([]);
  let progress: ProgressView | null = $state(null);
  let summary: SummaryView | null = $state(null);
  let error: string | null = $state(null);
  /** A short confirmation ("Exported 3 copy presets."), shown on the screen it was said on. */
  let info: { text: string; on: Screen } | null = $state(null);
  /** File › Export…'s dialog is open. */
  let exporting = $state(false);
  /** The Import screen's file, and the screen it goes back to. */
  let importing: ImportView | null = $state(null);
  let importBack: Screen = "setup";
  let settingsScreen: SettingsScreen | undefined = $state();
  let copyPresetsScreen: CopyPresetsScreen | undefined = $state();
  let mirrorsScreen: MirrorScreen | undefined = $state();
  let setupScreen: Setup | undefined = $state();
  let progressScreen: JobProgress | undefined = $state();
  /** Start is enabled on New copy. */
  let setupReady = $state(false);

  // The File menu offers only what applies here (spec §3). Derived, so a progress update that
  // changes nothing here doesn't send it again.
  const onSetup = $derived(screen === "setup");
  const canStart = $derived(onSetup && setupReady);
  // Any job or the queue, even when Cancel can't stop it now (a mirror's removals) or between
  // a queue's jobs: Import… waits for all of it (#83).
  const jobRuns = $derived(screen === "progress" || queueRun !== null);
  // A mirror's removals can't be cancelled, as on the progress screen.
  const cancellable = $derived.by(
    () => screen === "progress" && progress?.phase !== "done" && progress?.phase !== "removing",
  );
  $effect(() => {
    void api.setMenuState(onSetup, canStart, cancellable, jobRuns).catch(() => {});
  });

  function onMenu(item: string) {
    if (item === "choose-source" && screen === "setup") void setupScreen?.chooseSource();
    else if (item === "choose-destination" && screen === "setup") void setupScreen?.chooseDestination();
    else if (item === "start-copy" && screen === "setup") setupScreen?.startIfReady();
    else if (item === "cancel-copy" && screen === "progress") void progressScreen?.cancel();
    else if (item === "export-file") exporting = true;
    else if (item === "import-file") void chooseImport();
    else if (item === "show-copy") go("copy");
    else if (item === "show-mirror") go("mirror");
    else if (item === "show-verify") go("verify");
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
    undoing: false,
    recording: null,
  });

  /** A queue job before its first update: none of New copy's figures, which aren't its own. */
  const queueWaiting = (jobVerifies: boolean): ProgressView => ({
    ...waiting(),
    verify: jobVerifies,
    totalFiles: 0,
    totalBytes: 0,
  });

  /** Export's dialog answered: where to save, then save. */
  async function exportChosen(what: ExportWhat) {
    exporting = false;
    const today = new Date().toISOString().slice(0, 10);
    const path = await api.pickExportPath(t("app.exportName", { date: today }));
    if (!path) return;
    const said = await run(() => api.exportAll(path, what));
    if (said) info = { text: said, on: screen };
  }

  /** Import (#77): reads the file and shows what's in it; nothing changes yet. */
  async function showImport(path: string) {
    // Import opens over the screen: its unsaved changes are asked about first.
    const leaving = screen === "settings" ? settingsScreen : screen === "copy-presets" ? copyPresetsScreen : screen === "mirror" ? mirrorsScreen : undefined;
    if (leaving && !(await leaving.mayLeave())) return;
    const v = await run(() => api.openImport(path));
    if (!v) return;
    if (screen !== "import") importBack = screen;
    importing = v;
    screen = "import";
  }

  async function chooseImport() {
    const path = await api.pickImportFile();
    if (path) await showImport(path);
  }

  /** New copy as the app has it now, e.g. after a setting changed its plan. */
  async function refreshView() {
    view = newest(view, await api.sessionView().catch(() => view));
  }

  async function doImport(choices: ImportChoices) {
    const done = await run(() => api.applyImport(choices));
    if (!done) return;
    settings = done.settings;
    copyPresets = done.copyPresets;
    mirrorPresets = done.mirrorPresets;
    // The selected preset was replaced: New copy shows the imported one (#116).
    if (done.session) view = newest(view, done.session);
    else void refreshView();
    importing = null;
    screen = importBack;
    if (done.failed) error = say(done.message);
    else info = { text: say(done.message), on: screen };
  }

  /** A .secopy file opened from Finder, also at launch. */
  async function openedFromFinder() {
    const path = await api.takeOpenedFile();
    if (path) await showImport(path);
  }

  async function run<T>(action: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await action();
      error = null;
      return result;
    } catch (e) {
      error = messageOf(e);
      return undefined;
    }
  }

  /** A new job replaces the backend's last one: no old mirror summary acts on it. */
  function forgetMirrorSummary() {
    mirrorSummary = null;
    mirrorScreen = "mirror";
    verifySummary = null;
    verifyScreen = "verify";
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
    if (started === undefined) {
      screen = "setup";
      // Start checks the destination again: show it as it is now, with the reason (#112).
      view = newest(view, await api.sessionView().catch(() => view));
    } else recent = (await run(() => api.recentDestinations())) ?? recent;
  }

  /** Settings or Copy presets, over a section's screen; never during a copy. */
  async function open(next: "settings" | "copy-presets") {
    const sections: Screen[] = ["setup", "summary", "queue", "mirror", "mirror-summary", "verify", "verify-summary"];
    if (!sections.includes(screen)) return;
    if (!(await mayLeaveMirror())) return;
    back = screen as typeof back;
    screen = next;
  }

  const openSettings = () => open("settings");

  function saveMode(v: boolean) {
    void api.setMode(v).catch(() => {}); // remembered for next time; not worth an error
  }

  async function finish() {
    const done = (await run(() => api.jobSummary())) ?? null;
    if (!done) {
      // Back to the job's section with the reason, not stuck on Done (#117).
      screen = checkRunning ? "verify" : mirrorRunning ? "mirror" : "setup";
      return;
    }
    if (done.check) {
      verifySummary = done;
      screen = "verify-summary";
    } else if (done.mirror) {
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

  /** Verify (plan 8): the Verifying screen, then the check's summary on the Verify tab. */
  async function runCheck(path: string) {
    // The check replaces the backend's last job: no old Summary acts on it.
    summary = null;
    copyScreen = "setup";
    forgetMirrorSummary();
    progress = { ...waiting(), verify: true, totalFiles: 0, totalBytes: 0 };
    checkRunning = true;
    screen = "progress";
    const started = await run(() =>
      api.startCheck(path, (p) => {
        progress = p;
        if (p.phase === "done") void finish();
      }),
    );
    if (started === undefined) screen = "verify";
  }

  /** Runs the previewed mirror (FR-47): the Mirroring screen, then its summary. */
  async function runMirror(preview: MirrorPreviewView) {
    // The mirror replaces the backend's last copy: no old Summary acts on it.
    summary = null;
    copyScreen = "setup";
    verifySummary = null;
    verifyScreen = "verify";
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
      copyPresets = start.copyPresets;
      verify = start.verify;
      recent = start.recentDestinations;
      warnings = start.warnings.map(say);
      // The copy preset last used is loaded again when its source is there (FR-36).
      if (start.lastPreset) {
        const id = start.lastPreset;
        void run(() => api.selectCopyPreset(id)).then((next) => {
          if (next) view = newest(view, next);
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
    // Listening first, then the file Finder opened Secopy with: none is missed in between.
    const unlistenOpen = api.onOpenFile(() => void openedFromFinder());
    void unlistenOpen.then(() => openedFromFinder());
    // Closing during a copy asks first; if closed anyway, the app stops the copy cleanly.
    const unlisten = api.onCloseRequested(async (prevent) => {
      // A question is open (the quit question itself, or another): it's answered first, in a
      // window that stays in front (#113).
      if (asking.current) {
        prevent();
        return;
      }
      // During a job, with the setting on, Rust hides the window behind the menu bar icon (#80).
      // If that check fails, closing asks as before rather than doing nothing.
      if (await api.hideToMenuBar().catch(() => false)) {
        prevent();
        return;
      }
      if (!(await api.jobRunning())) return;
      // Settings can't change during a copy, so these are the running job's.
      const quit = quitQuestion(runningKind);
      const removing = progress?.phase === "removing";
      const stop = removing
        ? await api.confirm(
            finishingMessage(progress!.undoing, progress!.archiving),
            t("progress.quit.whenDone"),
            t("progress.quit.quit"),
            t("progress.quit.keepOpen"),
          )
        : queueRun?.checking
        ? await api.confirm(
            notStarted(),
            t("progress.quit.queueQuestion"),
            t("progress.quit.queueStop"),
            t("progress.quit.queueKeep"),
          )
        : await api.confirm(stopMessage(runningKind, settings.writeChecksumFile), quit.title, quit.stop, quit.keep);
      if (!stop) prevent();
    });
    return () => {
      unlisten.then((stop) => stop());
      unlistenSettings.then((stop) => stop());
      unlistenMenu.then((stop) => stop());
      unlistenOpen.then((stop) => stop());
    };
  });
</script>

<!-- App-wide messages, shown at the top of the screen's content. -->
{#snippet banner()}
  {#if info && info.on === screen}<Notice tone="success">{info.text}</Notice>{/if}
  {#if error}<Notice tone="danger">{error}</Notice>{/if}
  {#if warnings.length > 0}
    <Notice tone="warning">
      {#each warnings as w (w)}<p class="warning">{w}</p>{/each}
      <Button variant="link" onclick={() => (warnings = [])}>{t("app.dismiss")}</Button>
    </Notice>
  {/if}
{/snippet}

<div class="app">
  {#if showTabs}
    <TabBar
      items={[
        { id: "copy", label: t("app.tabs.copy") },
        { id: "mirror", label: t("app.tabs.mirror") },
        { id: "verify", label: t("app.tabs.verify") },
      ]}
      queue={{ count: queue.jobs.length }}
      selected={section}
      onSelect={(id) => go(id as "copy" | "mirror" | "verify" | "queue")}
    >
      {#snippet trailing()}<Button icon="settings" onclick={openSettings}>{t("app.settings")}</Button>{/snippet}
    </TabBar>
  {/if}
  <div class="screen">
    {#if screen === "setup"}
      <Setup
        bind:this={setupScreen}
        bind:ready={setupReady}
        bind:view
        bind:verify
        presets={copyPresets}
        {settings}
        {recent}
        {banner}
        onStart={start}
        onPresets={(p) => (copyPresets = p)}
        onManagePresets={() => open("copy-presets")}
        onMode={saveMode}
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
          mirror={runningKind === "mirror"}
          check={runningKind === "check"}
        />
      {/key}
    {:else if screen === "summary" && summary}
      <Summary {summary} {banner} onRetry={retry} onNewCopy={newCopy} />
    {:else if screen === "settings"}
      <SettingsScreen
        bind:this={settingsScreen}
        {settings}
        {banner}
        onSettings={(s) => {
          settings = s;
          // ASC MHL changes New copy's plan (#154): show it as it is now.
          void refreshView();
        }}
        onExport={() => (exporting = true)}
        onImport={chooseImport}
        onDone={() => (screen = back)}
      />
    {:else if screen === "import" && importing}
      <!-- Another file: a fresh screen, with its own ticks. -->
      {#key importing}
        <ImportScreen
          view={importing}
          {banner}
          onImport={doImport}
          onBack={() => {
            importing = null;
            screen = importBack;
          }}
        />
      {/key}
    {:else if screen === "copy-presets"}
      <CopyPresetsScreen
        bind:this={copyPresetsScreen}
        presets={copyPresets}
        onPresets={(p) => (copyPresets = p)}
        onView={(v) => (view = v)}
        onDone={() => (screen = back)}
      />
    {:else if screen === "mirror"}
      <MirrorScreen
        bind:this={mirrorsScreen}
        presets={mirrorPresets}
        {banner}
        onPresets={(p) => (mirrorPresets = p)}
        onPreview={async (v) => {
          // Only if still here: a long preview may end after you went elsewhere; changes made
          // meanwhile are asked about first (#117).
          if (screen !== "mirror" || !(await mayLeaveMirror()) || screen !== "mirror") return;
          mirrorPreview = v;
          screen = "mirror-preview";
        }}
        onQueue={(q) => (queue = q)}
      />
    {:else if screen === "mirror-preview" && mirrorPreview}
      <MirrorPreview
        preview={mirrorPreview}
        onRun={() => mirrorPreview && runMirror(mirrorPreview)}
        onQueue={(q) => (queue = q)}
        onCancel={() => (screen = "mirror")}
      />
    {:else if screen === "mirror-summary" && mirrorSummary}
      <Summary summary={mirrorSummary} {banner} onDone={() => (screen = "mirror")} />
    {:else if screen === "verify"}
      <VerifyScreen {banner} onStart={runCheck} onQueue={(q) => (queue = q)} />
    {:else if screen === "verify-summary" && verifySummary}
      <Summary summary={verifySummary} {banner} onDone={() => (screen = "verify")} />
    {:else if screen === "queue"}
      <QueueScreen {queue} {banner} onQueue={(q) => (queue = q)} onRun={runQueue} />
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

<ConfirmHost />

{#if exporting}
  <ExportDialog
    copyPresets={copyPresets.length}
    mirrorPresets={mirrorPresets.length}
    onExport={exportChosen}
    onClose={() => (exporting = false)}
  />
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .screen {
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

  .warning {
    margin: 0 0 var(--space-1);
  }
</style>
