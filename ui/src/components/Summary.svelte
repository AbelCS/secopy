<script lang="ts">
  // The summary (RFD §5.4): what happened, the figures, what failed and why, and what to do next.
  import { useApi } from "../lib/api";
  import type { SummaryView } from "../lib/bindings";
  import { formatBytes, formatCount, formatDuration, formatSpeed, plural } from "../lib/format";
  import { headline } from "../lib/headline";

  let {
    summary,
    onRetry,
    onNewCopy,
  }: {
    summary: SummaryView;
    onRetry: () => void;
    onNewCopy: () => void;
  } = $props();

  const api = useApi();
  let actionError: string | null = $state(null);
  const ok = $derived(summary.outcome === "complete");

  async function act(action: () => Promise<unknown>) {
    try {
      await action();
      actionError = null;
    } catch (e) {
      actionError = e instanceof Error ? e.message : String(e);
    }
  }

  async function saveReport() {
    const name = summary.reportFile?.split("/").pop() ?? "secopy_report.txt";
    const path = await api.pickReportPath(name);
    if (path) await act(() => api.saveReport(path));
  }
</script>

<section class="card">
  <h2 class:ok class:bad={!ok} role="status">{ok ? "✓" : "✗"} {headline(summary)}</h2>
  <ul class="stats">
    <li>{plural(summary.files, "file")}</li>
    <li>{formatBytes(summary.bytesWritten)} written</li>
    <li>{formatDuration(summary.millis)}</li>
    <li>
      {formatSpeed(summary.millis > 0 ? (summary.bytesWritten * 1000) / summary.millis : null)} average
    </li>
    {#if summary.skippedIdentical > 0}
      <li>{formatCount(summary.skippedIdentical)} already at the destination, not checked</li>
    {/if}
    {#if summary.skippedDifferent > 0}
      <li>{plural(summary.skippedDifferent, "different file")} left as they were</li>
    {/if}
    {#if summary.notStarted > 0}
      <li>{formatCount(summary.notStarted)} not started</li>
    {/if}
  </ul>
  {#if summary.checksumError}
    <p class="danger" role="alert">The checksum file could not be written: {summary.checksumError}</p>
  {/if}

  <div class="actions">
    <button type="button" onclick={() => act(() => api.reveal(summary.copyRoot))}>Reveal in Finder</button>
    {#if summary.checksumFile}
      <button type="button" onclick={() => act(() => api.openFile(summary.checksumFile!))}>
        Open checksum file
      </button>
    {/if}
    <button type="button" onclick={saveReport}>Save report…</button>
    {#if summary.failed > 0}
      <button type="button" onclick={onRetry}>Retry failed</button>
    {/if}
    <button type="button" class="primary" onclick={onNewCopy}>New copy</button>
  </div>
  {#if actionError}<p class="danger" role="alert">{actionError}</p>{/if}
</section>

{#if summary.failures.length > 0}
  <section class="card" aria-labelledby="failures-title">
    <h3 id="failures-title">Failed</h3>
    <ul class="failures">
      {#each summary.failures as f (f.id)}
        <li><span class="mono">{f.path}</span>: {f.reason}</li>
      {/each}
      {#if summary.failed > summary.failures.length}
        <li class="muted">and {formatCount(summary.failed - summary.failures.length)} more (see the report)</li>
      {/if}
    </ul>
  </section>
{/if}

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: var(--gap);
  }

  h2 {
    margin: 0 0 10px;
    font-size: 20px;
  }

  h2.ok {
    color: var(--success);
  }

  h2.bad {
    color: var(--danger);
  }

  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .stats {
    list-style: none;
    padding: 0;
    margin: 0 0 12px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .failures {
    margin: 0;
    padding-left: 18px;
    max-height: 280px;
    overflow-y: auto;
  }

  .danger {
    color: var(--danger);
  }

  .muted {
    color: var(--text-muted);
  }
</style>
