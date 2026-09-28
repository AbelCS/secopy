<script lang="ts">
  // The summary (RFD §5.4): what happened, the figures, what failed and why, and what to do next.
  import { useApi } from "../lib/api";
  import type { SummaryView } from "../lib/bindings";
  import { formatCount, messageOf, plural } from "../lib/format";
  import { headline } from "../lib/headline";
  import { summaryStats } from "../lib/summaryText";
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
    banner,
    queueIndex,
    onBack,
    onDone,
  }: {
    summary: SummaryView;
    onRetry?: () => void;
    onNewCopy?: () => void;
    /** A job of the queue run: its files and report, no Retry, and Back instead of New copy. */
    queueIndex?: number;
    onBack?: () => void;
    /** A mirror's summary: Done goes back to Mirror instead of New copy. */
    onDone?: () => void;
    /** App-wide messages, shown first. */
    banner?: Snippet;
  } = $props();

  const api = useApi();
  let actionError: string | null = $state(null);
  const ok = $derived(summary.outcome === "complete");
  const stats = $derived(summaryStats(summary));

  async function act(action: () => Promise<unknown>) {
    try {
      await action();
      actionError = null;
    } catch (e) {
      actionError = messageOf(e);
    }
  }

  async function saveReport() {
    const name = summary.reportFile?.split("/").pop() ?? "secopy_report.txt";
    const path = await api.pickReportPath(name);
    if (path) await act(() => (queueIndex === undefined ? api.saveReport(path) : api.queueSaveReport(queueIndex, path)));
  }
</script>

<AppShell>
  {#snippet header()}
    <ScreenHeader title="Summary" />
  {/snippet}

  {@render banner?.()}
  <div class="result">
    <div role="status">
      <h2 class:ok class:bad={!ok}><Icon name={ok ? "check" : "x"} size={20} /> {headline(summary)}</h2>
    </div>
    <Stats items={stats} />
    {#if summary.undone && summary.undone.notRestored > 0}
      <Notice tone="warning">
        {plural(summary.undone.notRestored, "file")} this job replaced couldn't be brought back: their new versions stay.
      </Notice>
    {/if}
    {#if summary.undone && summary.undone.failed > 0}
      <Notice tone="danger">{plural(summary.undone.failed, "file")} couldn't be removed (see the report).</Notice>
    {/if}
    <!-- After removing the copies, "left in the destination" would only confuse. -->
    {#if summary.mirror?.nothingRemoved && !summary.undone}<Notice tone="danger">{summary.mirror.nothingRemoved}</Notice>{/if}
    {#if summary.checksumOff && !summary.mirror && !summary.check}<p class="muted">No checksum file (off in Settings)</p>{/if}
    {#if summary.checksumError}
      <Notice tone="danger">The checksum file could not be written: {summary.checksumError}</Notice>
    {/if}
    {#if summary.durabilityError}
      <Notice tone="danger">
        The destination reported an error while the files were saved to disk: {summary.durabilityError}. Check
        the drive and run the copy again.
      </Notice>
    {/if}
    {#if summary.reportError}<Notice tone="danger">The report could not be saved: {summary.reportError}</Notice>{/if}
    {#if actionError}<Notice tone="danger">{actionError}</Notice>{/if}
  </div>

  {#if summary.failures.length > 0}
    <Section title="Failed">
      <ul class="failures">
        <!-- By place: unread items, directories and files each count their own ids. -->
        {#each summary.failures as f, i (i)}
          <li><span class="mono">{f.path}</span>: {f.reason}</li>
        {/each}
        {#if summary.failed + summary.unread + summary.dirErrors > summary.failures.length}
          <li class="muted">
            and {formatCount(summary.failed + summary.unread + summary.dirErrors - summary.failures.length)} more (see
            the report)
          </li>
        {/if}
      </ul>
    </Section>
  {/if}

  {#if summary.check && summary.check.problems.length > 0}
    <Section title="Problems">
      <ul class="failures">
        {#each summary.check.problems as p, i (i)}<li class="mono">{p}</li>{/each}
      </ul>
    </Section>
  {/if}

  {#if summary.mirror && summary.mirror.removalFailures.length > 0}
    <Section title="Not removed">
      <ul class="failures">
        {#each summary.mirror.removalFailures as f, i (i)}
          <li><span class="mono">{f.path}</span>: {f.reason}</li>
        {/each}
      </ul>
    </Section>
  {/if}

  <!-- Every file with its status and checksum, as during the copy (RFD §5.4). -->
  <FinishedList
    title="Files"
    total={summary.finished}
    failedTotal={summary.failed}
    updated={0}
    fetchPage={queueIndex === undefined
      ? undefined
      : (offset, limit, failedOnly) => api.queueFinishedPage(queueIndex, offset, limit, failedOnly)}
  />

  {#snippet actions()}
    <ActionBar>
      {#snippet start()}
        <!-- What you'd do next comes first. -->
        <!-- Not after a cancel that removed the copied files: a retry would copy only a few. -->
        {#if summary.failed > 0 && onRetry && !summary.undone && !summary.check}<Button onclick={onRetry}>Retry failed</Button>{/if}
        <Button onclick={() => act(() => api.reveal(summary.copyRoot))}>Show in Finder</Button>
        {#if summary.checksumFile}
          <Button onclick={() => act(() => api.openFile(summary.checksumFile!))}>Open checksum file</Button>
        {/if}
        <Button onclick={saveReport}>Save report…</Button>
      {/snippet}
      {#snippet end()}
        {#if onBack}
          <Button variant="primary" onclick={onBack}>Back</Button>
        {:else if onDone}
          <Button variant="primary" onclick={onDone}>Done</Button>
        {:else}
          <Button variant="primary" onclick={onNewCopy}>New copy</Button>
        {/if}
      {/snippet}
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
