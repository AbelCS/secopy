<script lang="ts">
  // The queue (RFD §5.7): the jobs saved with Add to queue, in the order they run, what to do
  // when one fails, and Run queue.
  import type { Snippet } from "svelte";
  import { useApi } from "../lib/api";
  import type { OnFailure, QueueView } from "../lib/bindings";
  import { plural } from "../lib/format";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import EmptyState from "../lib/ui/EmptyState.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import RadioGroup from "../lib/ui/RadioGroup.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";

  let {
    queue,
    onQueue,
    onRun,
    onSettings,
    banner,
  }: {
    queue: QueueView;
    /** The queue changed (and was saved). */
    onQueue: (queue: QueueView) => void;
    onRun: () => void;
    onSettings?: () => void;
    /** App-wide messages, shown first. */
    banner?: Snippet;
  } = $props();

  const api = useApi();
  let error: string | null = $state(null);
  const count = $derived(queue.jobs.length);

  async function change(call: () => Promise<QueueView>) {
    try {
      onQueue(await call());
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function clear() {
    const sure = await api.confirm("Every job in the queue is removed.", "Clear the queue?", "Clear", "Keep");
    if (sure) await change(() => api.clearQueue());
  }
</script>

<AppShell>
  {#snippet header()}
    <ScreenHeader title="Queue">
      {#snippet trailing()}
        {#if onSettings}<Button icon="settings" onclick={onSettings}>Settings</Button>{/if}
      {/snippet}
    </ScreenHeader>
  {/snippet}

  {@render banner?.()}
  {#if error}<Notice tone="danger">{error}</Notice>{/if}

  <Section title="Queue">
    {#if count === 0}
      <EmptyState>
        <p>Nothing queued. Set up a copy or a mirror and press Add to queue: it runs here, one after another with the others.</p>
      </EmptyState>
    {:else}
      <ol class="jobs">
        {#each queue.jobs as job, i (i)}
          <li class="job">
            <span class="number">{i + 1}</span>
            <div class="what">
              {#if job.supported}
                <span class="mode">{job.kind === "mirror" ? `Mirror · ${job.name ?? ""}` : job.verify ? "Copy & Verify" : "Copy"}</span>
                <!-- A long path keeps its end visible; <bdi> keeps its slashes in place. -->
                <span class="path mono" title={job.source}><bdi>{job.source}</bdi></span>
                <span class="arrow" aria-hidden="true">→</span>
                <span class="path mono" title={job.destination}><bdi>{job.destination}</bdi></span>
              {:else}
                <span class="mode">A job for a newer Secopy</span>
              {/if}
              {#if job.lastError}<Notice tone="danger">{job.lastError}</Notice>{/if}
            </div>
            <div class="buttons">
              <Button aria-label="Move up" disabled={i === 0} onclick={() => change(() => api.moveInQueue(i, i - 1))}>↑</Button>
              <Button aria-label="Move down" disabled={i === count - 1} onclick={() => change(() => api.moveInQueue(i, i + 1))}>↓</Button>
              <Button aria-label="Remove" onclick={() => change(() => api.removeFromQueue(i))}>✕</Button>
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  </Section>

  {#if count > 0}
    <Section title="If a job fails">
      <RadioGroup
        legend="If a job fails"
        hideLegend
        options={[
          { value: "continue" as OnFailure, label: "Continue with the next job" },
          { value: "stop" as OnFailure, label: "Stop the queue" },
        ]}
        value={queue.onFailure}
        onChange={(v) => change(() => api.setQueueOnFailure(v))}
      />
    </Section>
  {/if}

  {#snippet actions()}
    <ActionBar status={count > 0 ? plural(count, "job") : ""}>
      {#snippet start()}
        <Button variant="danger" disabled={count === 0} onclick={clear}>Clear queue…</Button>
      {/snippet}
      {#snippet end()}
        <Button variant="primary" disabled={count === 0} onclick={onRun}>Run queue</Button>
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .jobs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .job {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: start;
    padding: var(--space-2) 0;
  }

  .job + .job {
    border-top: 1px solid var(--border);
  }

  .number {
    color: var(--text-muted);
    padding-top: 4px;
  }

  .what {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-2);
    min-width: 0;
    padding-top: 4px;
  }

  .mode {
    font-weight: 600;
  }

  .path {
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }

  .arrow {
    color: var(--text-muted);
  }

  .what :global(.notice) {
    flex-basis: 100%;
    margin: 0;
  }

  .buttons {
    display: flex;
    gap: var(--space-1);
  }
</style>
