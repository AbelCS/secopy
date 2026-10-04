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

  /** Whether the change worked. */
  async function change(call: () => Promise<QueueView>): Promise<boolean> {
    try {
      onQueue(await call());
      error = null;
      return true;
    } catch (e) {
      error = messageOf(e);
      return false;
    }
  }

  /** A removal or move on its way: jobs are addressed by their place, so a second click
   *  before it comes back would act on another job (#117). */
  let moving = false;

  async function byPlace(call: () => Promise<QueueView>): Promise<boolean> {
    if (moving) return false;
    moving = true;
    try {
      return await change(call);
    } finally {
      moving = false;
    }
  }

  /** What VoiceOver says after a move (#205). */
  let announced = $state("");

  /** Moves a job and keeps the focus on its grip, in its new row (for the keyboard and
   *  VoiceOver), and says where it went. */
  async function move(from: number, to: number) {
    if (!(await byPlace(() => api.moveInQueue(from, to)))) return;
    await tick();
    document.querySelectorAll<HTMLElement>("[data-grip]")[to]?.focus();
    announced = t("queue.moved", { position: to + 1, count });
  }

  /** A row being dragged by its grip (#205): where it started, how far it went, where it
   *  would land, and the rows as they were laid out when it started. */
  type Drag = { from: number; startY: number; dy: number; to: number; rows: DOMRect[]; started: boolean };
  let drag: Drag | null = $state(null);

  /** Pixels a press moves before it's a drag, not a click. */
  const SLOP = 4;

  function grab(event: PointerEvent, from: number) {
    if (event.button !== 0 || count < 2) return;
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    const rows = [...document.querySelectorAll<HTMLElement>(".job")].map((row) => row.getBoundingClientRect());
    drag = { from, startY: event.clientY, dy: 0, to: from, rows, started: false };
  }

  function follow(event: PointerEvent) {
    if (!drag) return;
    const dy = event.clientY - drag.startY;
    const { from, rows } = drag;
    // It lands past every row whose middle its own middle has passed.
    const middle = (rows[from].top + rows[from].bottom) / 2 + dy;
    let to = from;
    rows.forEach((row, j) => {
      const m = (row.top + row.bottom) / 2;
      if (j < from && middle < m) to = Math.min(to, j);
      if (j > from && middle > m) to = Math.max(to, j);
    });
    drag = { ...drag, dy, to, started: drag.started || Math.abs(dy) > SLOP };
  }

  async function drop() {
    const done = drag;
    drag = null;
    if (done?.started && done.to !== done.from) await move(done.from, done.to);
  }

  /** How far row `j` is shifted while another is dragged over it, or the dragged row itself. */
  function shift(j: number): number {
    if (!drag?.started) return 0;
    const { from, to, dy, rows } = drag;
    if (j === from) return dy;
    const height = rows[from].height;
    if (from < to && j > from && j <= to) return -height;
    if (to < from && j >= to && j < from) return height;
    return 0;
  }

  /** ⌥↑ / ⌥↓ move the job; Esc lets go of a drag. */
  function key(event: KeyboardEvent, i: number) {
    if (event.key === "Escape" && drag) {
      event.preventDefault();
      drag = null;
      return;
    }
    if (!event.altKey) return;
    const to = event.key === "ArrowUp" ? i - 1 : event.key === "ArrowDown" ? i + 1 : -1;
    if (to < 0 || to >= count) return;
    event.preventDefault();
    void move(i, to);
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
      <ol class="jobs" class:dragging={drag?.started} role="list" aria-label={t("queue.jobs")}>
        {#each queue.jobs as job, i (i)}
          <li
            class="job"
            class:dragged={drag?.started && drag.from === i}
            class:settling={drag?.started && drag.from !== i}
            style:transform={shift(i) ? `translateY(${shift(i)}px)` : undefined}
          >
            <span class="grip">
              <Button
                data-grip
                icon="grip"
                aria-label={t("queue.grip", { number: i + 1 })}
                help={t("queue.gripHelp")}
                onpointerdown={(e: PointerEvent) => grab(e, i)}
                onpointermove={follow}
                onpointerup={drop}
                onpointercancel={() => (drag = null)}
                onkeydown={(e: KeyboardEvent) => key(e, i)}>{""}</Button
              >
            </span>
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
              <Button aria-label={t("queue.remove", { number: i + 1 })} onclick={() => byPlace(() => api.removeFromQueue(i))}>✕</Button>
            </div>
          </li>
        {/each}
      </ol>
      <p class="visually-hidden" aria-live="polite">{announced}</p>
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
    grid-template-columns: auto 24px minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: start;
    padding: var(--space-2) 0;
  }

  .job + .job {
    border-top: 1px solid var(--border);
  }

  /* The row being dragged follows the pointer, above the others; they make room for it. */
  .job.dragged {
    position: relative;
    z-index: 1;
    background: var(--surface-raised);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.35);
  }

  .job.settling {
    transition: transform 120ms ease;
  }

  /* A quiet handle: the icon alone, an outline when the pointer or the focus is on it. */
  .grip :global(.button) {
    background: none;
    border-color: transparent;
    color: var(--text-muted);
    padding: 0 var(--space-1);
    cursor: grab;
    touch-action: none;
  }

  .grip :global(.button:hover) {
    border-color: var(--border);
    color: var(--text);
  }

  .job.dragged .grip :global(.button) {
    cursor: grabbing;
  }

  /* No help tags while a job is carried: they'd sit over the rows it moves through. */
  .jobs.dragging :global(.tip) {
    display: none;
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
