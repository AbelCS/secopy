<script lang="ts">
  // The end of a queue run (FR-43): one row per job with its result, each opening that job's
  // own summary.
  import { t } from "../lib/i18n";
  import type { QueuedJobView, QueueResult, QueueSummaryView } from "../lib/bindings";
  import { formatDuration } from "../lib/format";
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
    complete: t("queue.result.complete"),
    failed: t("queue.result.failed"),
    cancelled: t("queue.result.cancelled"),
    notRun: t("queue.result.notRun"),
  };

  /** A job in words, for its Summary button's name: "Verify · /Volumes/Backup". "to", not
   *  "→", so VoiceOver doesn't read "right arrow". */
  function jobName(job: QueuedJobView): string {
    if (job.kind === "check") return t("queue.name.check", { source: job.source });
    const kind =
      job.kind === "mirror"
        ? t("queue.name.mirror", { name: job.name ?? "" }).trim()
        : t(job.verify ? "queue.mode.copyVerify" : "queue.mode.copy");
    return t("queue.name.job", { kind, source: job.source, destination: job.destination });
  }
</script>

<AppShell>
  {#snippet header()}<ScreenHeader title={t("queue.title")} />{/snippet}

  <div class="result">
    <div role="status">
      <h2 class:ok class:bad={!ok}>
        <Icon name={ok ? "check" : "x"} size={20} />
        {t("queue.done.headline", { complete: summary.complete, jobs: t("queue.count", { count: summary.count }) })}
      </h2>
    </div>
    <p class="muted">{t("queue.done.took", { time: formatDuration(summary.millis) })}</p>
    {#if summary.saveError}<Notice tone="danger">{summary.saveError}</Notice>{/if}
  </div>

  <Section title={t("queue.done.jobs")}>
    <ol class="jobs" role="list" aria-label={t("queue.done.results")}>
      {#each summary.results as r, i (i)}
        <li class="job {r.result}">
          <span class="icon" aria-hidden="true">{r.result === "complete" ? "✓" : r.result === "notRun" ? "–" : "✗"}</span>
          <div class="what">
            <span class="word">{word[r.result]}</span>
            {#if !r.job.supported}
              <span>{t(r.job.kind === "mirror" ? "queue.unsupported.mirror" : "queue.unsupported.newer")}</span>
            {:else}
              {#if r.job.kind === "check"}
                <span>{t("queue.mode.verify")}</span>
                <span class="mono path"><bdi>{r.job.source}</bdi></span>
              {:else}
                {#if r.job.kind === "mirror"}<span>{t("queue.mode.mirror", { name: r.job.name ?? "" })}</span>{/if}
                <span class="mono path"><bdi>{r.job.source}</bdi></span>
                <span class="muted" aria-hidden="true">→</span>
                <span class="mono path"><bdi>{r.job.destination}</bdi></span>
              {/if}
            {/if}
            <span class="line">{r.summary ? headline(r.summary) : (r.reason ?? "")}</span>
          </div>
          {#if r.summary}
            <Button aria-label={t("queue.done.summaryOf", { number: i + 1, name: jobName(r.job) })} onclick={() => onOpen(i)}
              >{t("queue.done.summary")}</Button
            >
          {/if}
        </li>
      {/each}
    </ol>
  </Section>

  {#snippet actions()}
    <ActionBar>
      {#snippet end()}<Button variant="primary" onclick={onDone}>{t("queue.done.done")}</Button>{/snippet}
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
