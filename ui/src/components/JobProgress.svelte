<script lang="ts">
  // The progress view (RFD §5.3): phase and time, one bar per phase, active files, finished
  // files, Pause / Resume and Cancel.
  import { useApi } from "../lib/api";
  import type { ProgressView } from "../lib/bindings";
  import { formatBytes, formatCount, formatDuration, formatPercent, plural } from "../lib/format";
  import { RateMeter } from "../lib/rate";
  import FinishedList from "./FinishedList.svelte";
  import ProgressBar from "./ProgressBar.svelte";

  let { progress }: { progress: ProgressView } = $props();

  const api = useApi();
  // The meters aren't reactive; each update recomputes the figures from them.
  const copyMeter = new RateMeter();
  const verifyMeter = new RateMeter();
  type Figures = { speed: number | null; average: number | null; eta: number | null };
  const unknown: Figures = { speed: null, average: null, eta: null };
  let copy: Figures = $state(unknown);
  let verify: Figures = $state(unknown);

  $effect(() => {
    copyMeter.push(progress.elapsedMs, progress.copiedBytes);
    verifyMeter.push(progress.elapsedMs, progress.verifiedBytes);
    const left = (done: number) => progress.totalBytes - done;
    copy = {
      speed: copyMeter.current(),
      average: copyMeter.average(),
      eta: copyMeter.eta(left(progress.copiedBytes)),
    };
    verify = {
      speed: verifyMeter.current(),
      average: verifyMeter.average(),
      eta: verifyMeter.eta(left(progress.verifiedBytes)),
    };
  });

  const phase = $derived(
    progress.phase === "done"
      ? "Done"
      : progress.paused
        ? "Paused"
        : progress.phase === "verifying"
          ? "Verifying"
          : progress.verify
            ? "Copying & verifying"
            : "Copying",
  );
  const files = $derived.by(() => {
    const parts = [`${formatCount(progress.filesDone)} / ${plural(progress.totalFiles, "file")}`];
    if (progress.filesSkipped > 0) parts.push(`${formatCount(progress.filesSkipped)} skipped`);
    if (progress.filesFailed > 0) parts.push(`${formatCount(progress.filesFailed)} failed`);
    return parts.join(" · ");
  });

  async function cancel() {
    const stop = await api.confirm(
      "Files already copied stay and are listed in the checksum file; the file in progress is removed.",
      "Stop copying?",
    );
    if (stop) await api.cancelJob();
  }
</script>

<section class="card" aria-labelledby="phase">
  <header>
    <h2 id="phase">{phase}</h2>
    <span class="muted">{formatDuration(progress.elapsedMs)} elapsed</span>
  </header>

  {#if progress.fatal}
    <p class="banner" role="alert">Stopped: {progress.fatal}</p>
  {/if}

  <ProgressBar label="Copied" done={progress.copiedBytes} total={progress.totalBytes} {...copy} />
  {#if progress.verify}
    <ProgressBar label="Verified" done={progress.verifiedBytes} total={progress.totalBytes} {...verify} />
  {/if}
  <p class="muted files">{files}</p>

  <div class="controls">
    {#if progress.paused}
      <button type="button" onclick={() => api.resumeJob()}>Resume</button>
    {:else}
      <button type="button" onclick={() => api.pauseJob()} disabled={progress.phase === "done"}>Pause</button>
    {/if}
    <button type="button" onclick={cancel} disabled={progress.phase === "done"}>Cancel</button>
  </div>
</section>

<section class="card" aria-labelledby="active-title">
  <h3 id="active-title">Active</h3>
  {#if progress.active.length === 0 && !progress.smallFiles}
    <p class="muted">—</p>
  {/if}
  <table>
    <tbody>
      {#each progress.active as f (f.id)}
        <tr>
          <td class="name" title={f.path}>{f.name}</td>
          <td>{f.verifying ? "Verifying" : "Copying"}</td>
          <td>{formatBytes(f.size)}</td>
          <td>{formatBytes(f.bytesDone)}</td>
          <td>{formatPercent(f.bytesDone, f.size)}</td>
        </tr>
      {/each}
      {#if progress.smallFiles}
        <tr>
          <td class="name">+ {plural(progress.smallFiles.count, "small file")}</td>
          <td></td>
          <td>{formatBytes(progress.smallFiles.size)}</td>
          <td>{formatBytes(progress.smallFiles.bytesDone)}</td>
          <td>{formatPercent(progress.smallFiles.bytesDone, progress.smallFiles.size)}</td>
        </tr>
      {/if}
    </tbody>
  </table>
  <FinishedList
    total={progress.filesDone}
    failedTotal={progress.filesFailed}
    updated={progress.elapsedMs}
  />
</section>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: var(--gap);
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  h2 {
    margin: 0;
    font-size: 16px;
  }

  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .muted {
    color: var(--text-muted);
  }

  .banner {
    color: var(--danger);
    border: 1px solid var(--danger);
    border-radius: var(--radius);
    padding: 8px 12px;
  }

  .files {
    margin: 4px 0 0 82px;
  }

  .controls {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  td {
    padding: 3px 6px;
    white-space: nowrap;
  }

  td.name {
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
