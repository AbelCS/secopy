<script lang="ts">
  // The queue (RFD §5.7): the jobs saved with Add to queue, in the order they run, what to do
  // when one fails, and Start.
  import { t } from "../lib/i18n";
  import { say } from "../lib/message";
  import { tick, type Snippet } from "svelte";
  import { useApi } from "../lib/api";
  import type { OnFailure, QueueView } from "../lib/bindings";
  import { messageOf } from "../lib/format";
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
    banner,
  }: {
    queue: QueueView;
    /** The queue changed (and was saved). */
    onQueue: (queue: QueueView) => void;
    onRun: () => void;
    /** App-wide messages, shown first. */
    banner?: Snippet;
  } = $props();

  const api = useApi();
  let error: string | null = $state(null);
  const count = $derived(queue.jobs.length);

  /** A change on its way: jobs are removed and moved by their place, so a second click
   *  before it comes back would act on another job (#117). */
  let changing = false;

  /** Whether the change worked. */
  async function change(call: () => Promise<QueueView>): Promise<boolean> {
    if (changing) return false;
    changing = true;
    try {
      onQueue(await call());
      error = null;
      return true;
    } catch (e) {
      error = messageOf(e);
      return false;
    } finally {
      changing = false;
    }
  }

  /** Moves a job and keeps the focus on it, in its new row (for the keyboard and VoiceOver). */
  async function move(from: number, to: number) {
    if (!(await change(() => api.moveInQueue(from, to)))) return;
    await tick();
    const row = document.querySelectorAll<HTMLElement>(".job")[to];
    const buttons = [...(row?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const same = buttons[to > from ? 1 : 0];
    (same && !same.disabled ? same : buttons.find((b) => !b.disabled))?.focus();
  }

  async function clear() {
    const sure = await api.confirm(t("queue.clear.message"), t("queue.clear.title"), t("queue.clear.clear"), t("queue.clear.keep"));
    if (sure) await change(() => api.clearQueue());
  }
</script>

<AppShell>
  {#snippet header()}
    <ScreenHeader title={t("queue.title")} />
  {/snippet}

  {@render banner?.()}
  {#if error}<Notice tone="danger">{error}</Notice>{/if}

  <Section title={t("queue.title")}>
    {#if count === 0}
      <EmptyState>
        <p>{t("queue.empty")}</p>
      </EmptyState>
    {:else}
      <!-- role="list": Safari drops list semantics when the bullets are hidden. -->
      <ol class="jobs" role="list" aria-label={t("queue.jobs")}>
        {#each queue.jobs as job, i (i)}
          <li class="job">
            <span class="number">{i + 1}</span>
            <div class="what">
              {#if job.supported && job.kind === "check"}
                <span class="mode">{t("queue.mode.verify")}</span>
                <span class="path mono" title={say(job.source)}><bdi>{say(job.source)}</bdi></span>
              {:else if job.supported}
                <span class="mode"
                  >{job.kind === "mirror"
                    ? t("queue.mode.mirror", { name: job.name ?? "" })
                    : t(job.verify ? "queue.mode.copyVerify" : "queue.mode.copy")}</span
                >
                <!-- A long path keeps its end visible; <bdi> keeps its slashes in place. -->
                <span class="path mono" title={say(job.source)}><bdi>{say(job.source)}</bdi></span>
                <span class="arrow" aria-hidden="true">→</span>
                <span class="path mono" title={job.destination}><bdi>{job.destination}</bdi></span>
              {:else}
                <span class="mode">{t(job.kind === "mirror" ? "queue.unsupported.mirror" : "queue.unsupported.newer")}</span>
              {/if}
              {#if job.lastError}<Notice tone="danger">{say(job.lastError)}</Notice>{/if}
            </div>
            <div class="buttons">
              <Button aria-label={t("queue.moveUp", { number: i + 1 })} disabled={i === 0} onclick={() => move(i, i - 1)}>↑</Button>
              <Button aria-label={t("queue.moveDown", { number: i + 1 })} disabled={i === count - 1} onclick={() => move(i, i + 1)}>↓</Button>
              <Button aria-label={t("queue.remove", { number: i + 1 })} onclick={() => change(() => api.removeFromQueue(i))}>✕</Button>
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  </Section>

  {#if count > 0}
    <Section title={t("queue.onFailure.title")}>
      <RadioGroup
        legend={t("queue.onFailure.title")}
        hideLegend
        options={[
          { value: "continue" as OnFailure, label: t("queue.onFailure.continue") },
          { value: "stop" as OnFailure, label: t("queue.onFailure.stop") },
        ]}
        value={queue.onFailure}
        onChange={(v) => change(() => api.setQueueOnFailure(v))}
      />
    </Section>
  {/if}

  {#snippet actions()}
    <ActionBar status={count > 0 ? t("queue.count", { count }) : ""}>
      {#snippet start()}
        <Button
          variant="danger"
          disabled={count === 0}
          help={t("queue.clearHelp")}
          onclick={clear}>{t("queue.clearButton")}</Button
        >
      {/snippet}
      {#snippet end()}
        <Button
          variant="primary"
          disabled={count === 0}
          help={t("queue.startHelp", { count })}
          onclick={onRun}>{t("queue.start")}</Button
        >
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
