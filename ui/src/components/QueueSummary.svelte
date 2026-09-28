<script lang="ts">
  // The end of a queue run (FR-43): one row per job with its result, each opening that job's
  // own summary.
  import type { QueueResult, QueueSummaryView } from "../lib/bindings";
  import { formatDuration, plural } from "../lib/format";
  import { headline } from "../lib/headline";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";

  let {
    summary,
    onOpen,
    onDone,
  }: {
    summary: QueueSummaryView;
    /** Opens the summary of the job at this index. */
    onOpen: (index: number) => void;
    onDone: () => void;
  } = $props();

  const ok = $derived(summary.complete === summary.count);
  const word: Record<QueueResult, string> = {
    complete: "Complete",
    failed: "Failed",
    cancelled: "Cancelled",
    notRun: "Not run",
  };
</script>

<AppShell>
  {#snippet header()}<ScreenHeader title="Queue" />{/snippet}

  <div class="result">
    <div role="status">
      <h2 class:ok class:bad={!ok}>
        <Icon name={ok ? "check" : "x"} size={20} /> Queue done: {summary.complete} of {plural(summary.count, "job")} complete
      </h2>
    </div>
    <p class="muted">took {formatDuration(summary.millis)}</p>
    {#if summary.saveError}<Notice tone="danger">{summary.saveError}</Notice>{/if}
  </div>

  <Section title="Jobs">
    <ol class="jobs" role="list">
      {#each summary.results as r, i (i)}
        <li class="job {r.result}">
          <span class="icon" aria-hidden="true">{r.result === "complete" ? "✓" : r.result === "notRun" ? "–" : "✗"}</span>
          <div class="what">
            <span class="word">{word[r.result]}</span>
            {#if !r.job.supported}
              <span>{r.job.kind === "mirror" ? "A mirror that was deleted" : "A job for a newer Secopy"}</span>
            {:else}
              {#if r.job.kind === "check"}
                <span>Verify</span>
                <span class="mono path"><bdi>{r.job.source}</bdi></span>
              {:else}
                {#if r.job.kind === "mirror"}<span>Mirror · {r.job.name ?? ""}</span>{/if}
                <span class="mono path"><bdi>{r.job.source}</bdi></span>
                <span class="muted" aria-hidden="true">→</span>
                <span class="mono path"><bdi>{r.job.destination}</bdi></span>
              {/if}
            {/if}
            <span class="line">{r.summary ? headline(r.summary) : (r.reason ?? "")}</span>
          </div>
          {#if r.summary}<Button onclick={() => onOpen(i)}>Summary</Button>{/if}
        </li>
      {/each}
    </ol>
  </Section>

  {#snippet actions()}
    <ActionBar>
      {#snippet end()}<Button variant="primary" onclick={onDone}>Done</Button>{/snippet}
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
    margin: 0;
  }

  .jobs {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .job {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-2) 0;
  }

  .job + .job {
    border-top: 1px solid var(--border);
  }

  .complete .icon {
    color: var(--success);
  }

  .failed .icon,
  .cancelled .icon {
    color: var(--danger);
  }

  .notRun .icon {
    color: var(--text-muted);
  }

  .what {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-2);
    min-width: 0;
  }

  .word {
    font-weight: 600;
  }

  .path {
    max-width: 35%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }

  .line {
    flex-basis: 100%;
    color: var(--text-muted);
  }
</style>
