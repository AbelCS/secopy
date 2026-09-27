<script lang="ts">
  // The app's screens: set up a copy, follow it, read the summary (RFD §5.2–§5.4).
  import { onMount } from "svelte";
  import { provideApi, tauriApi, type Api } from "./lib/api";
  import type { ProgressView, SessionView, SummaryView } from "./lib/bindings";
  import JobProgress from "./components/JobProgress.svelte";
  import Setup from "./components/Setup.svelte";
  import Summary from "./components/Summary.svelte";

  let { api = tauriApi }: { api?: Api } = $props();
  // The app talks to one Api for its whole life; tests pass a fake one.
  // svelte-ignore state_referenced_locally
  provideApi(api);

  let screen: "setup" | "progress" | "summary" = $state("setup");
  let view: SessionView = $state({
    source: null,
    selectedFiles: 0,
    selectedBytes: 0,
    destination: null,
    conflicts: "keepBoth",
    plan: null,
    stale: false,
  });
  let verify = $state(true);
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
    void run(() => api.sessionView()).then((v) => v && (view = v));
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
    };
  });
</script>

<main>
  <h1>Secopy</h1>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if screen === "setup"}
    <Setup bind:view bind:verify onStart={start} />
  {:else if screen === "progress" && progress}
    <JobProgress {progress} />
  {:else if screen === "summary" && summary}
    <Summary {summary} onRetry={retry} onNewCopy={newCopy} />
  {/if}
</main>

<style>
  main {
    max-width: 960px;
    margin: 0 auto;
    padding: 20px 24px;
  }

  h1 {
    margin: 0 0 14px;
    font-size: 18px;
  }

  .error {
    color: var(--danger);
  }
</style>
