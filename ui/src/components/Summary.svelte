<script lang="ts">
  // The summary (RFD §5.4): what happened, the figures, what failed and why, and what to do next.
  import { useApi } from "../lib/api";
  import type { SummaryView } from "../lib/bindings";
  import { formatBytes, formatCount, formatDuration, formatSpeed, plural } from "../lib/format";
  import { headline } from "../lib/headline";
  import type { Snippet } from "svelte";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import Stats from "../lib/ui/Stats.svelte";
  import FinishedList from "./FinishedList.svelte";

  let {
    summary,
    onRetry,
    onNewCopy,
    onSettings,
    banner,
  }: {
    summary: SummaryView;
    onRetry: () => void;
    onNewCopy: () => void;
    onSettings?: () => void;
    /** App-wide messages, shown first. */
    banner?: Snippet;
  } = $props();

  const api = useApi();
  let actionError: string | null = $state(null);
  const ok = $derived(summary.outcome === "complete");
  const stats = $derived.by(() => {
    const speed = formatSpeed(summary.millis > 0 ? (summary.bytesWritten * 1000) / summary.millis : null);
    const items = [
      plural(summary.files, "file"),
      `${formatBytes(summary.bytesWritten)} written`,
      `took ${formatDuration(summary.millis)}`,
      `${speed} average`,
    ];
    if (summary.skippedIdentical > 0)
      items.push(`${formatCount(summary.skippedIdentical)} already at the destination, not checked`);
    if (summary.skippedDifferent > 0) items.push(`${plural(summary.skippedDifferent, "different file")} left as they were`);
    if (summary.notStarted > 0) items.push(`${formatCount(summary.notStarted)} not started`);
    return items;
  });

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

<AppShell>
  {#snippet header()}
    <ScreenHeader title="Summary">
      {#snippet trailing()}
        {#if onSettings}<Button icon="settings" onclick={onSettings}>Settings</Button>{/if}
      {/snippet}
    </ScreenHeader>
  {/snippet}

  {@render banner?.()}
  <div class="result">
    <h2 class:ok class:bad={!ok} role="status"><Icon name={ok ? "check" : "x"} size={20} /> {headline(summary)}</h2>
    <Stats items={stats} />
    {#if summary.checksumOff}<p class="muted">No checksum file (off in Settings)</p>{/if}
    {#if summary.checksumError}
      <Notice tone="danger">The checksum file could not be written: {summary.checksumError}</Notice>
    {/if}
    {#if summary.reportError}<Notice tone="danger">The report could not be saved: {summary.reportError}</Notice>{/if}
    {#if actionError}<Notice tone="danger">{actionError}</Notice>{/if}
  </div>

  {#if summary.failures.length > 0}
    <Section title="Failed">
      <ul class="failures">
        {#each summary.failures as f (f.id)}
          <li><span class="mono">{f.path}</span>: {f.reason}</li>
        {/each}
        {#if summary.failed > summary.failures.length}
          <li class="muted">and {formatCount(summary.failed - summary.failures.length)} more (see the report)</li>
        {/if}
      </ul>
    </Section>
  {/if}

  <!-- Every file with its status and checksum, as during the copy (RFD §5.4). -->
  <FinishedList title="Files" total={summary.finished} failedTotal={summary.failed} updated={0} />

  {#snippet actions()}
    <ActionBar>
      {#snippet start()}
        <Button onclick={() => act(() => api.reveal(summary.copyRoot))}>Reveal in Finder</Button>
        {#if summary.checksumFile}
          <Button onclick={() => act(() => api.openFile(summary.checksumFile!))}>Open checksum file</Button>
        {/if}
        <Button onclick={saveReport}>Save report…</Button>
        {#if summary.failed > 0}<Button onclick={onRetry}>Retry failed</Button>{/if}
      {/snippet}
      {#snippet end()}<Button variant="primary" onclick={onNewCopy}>New copy</Button>{/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .result {
    padding: var(--space-1) 0 var(--space-2);
  }

  h2 {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 0 var(--space-2);
    font-size: var(--text-xl);
  }

  h2.ok {
    color: var(--success);
  }

  h2.bad {
    color: var(--danger);
  }

  .muted {
    color: var(--text-muted);
  }

  .failures {
    margin: 0;
    padding-left: var(--space-4);
  }
</style>
