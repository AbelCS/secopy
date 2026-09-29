<script lang="ts">
  // The progress view (RFD §5.3): phase and time, one bar per phase, active files, finished
  // files, Pause / Resume and Cancel.
  import { notStarted, stopMessage, stopQuestion, type JobKind } from "../lib/stopping";
  import { t } from "../lib/i18n";
  import { useApi } from "../lib/api";
  import type { ProgressView } from "../lib/bindings";
  import { formatBytes, formatDuration, formatPercent } from "../lib/format";
  import { RateMeter } from "../lib/rate";
  import type { Snippet } from "svelte";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";
  import Hint from "../lib/ui/Hint.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ProgressBar from "../lib/ui/ProgressBar.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import FinishedList from "./FinishedList.svelte";

  let {
    progress,
    checksumFile = true,
    banner,
    queue,
    mirror = false,
    checking = false,
    compared,
    check = false,
  }: {
    progress: ProgressView;
    /** The job is a mirror: its title and its stop question say so. */
    mirror?: boolean;
    /** A queue job is being checked before it starts: nothing is copied yet. */
    checking?: boolean;
    /** While checking a mirror with the deep check: files compared, of how many. */
    compared?: { done: number; total: number };
    /** A check (Verify): every listed file read again; nothing is written. */
    check?: boolean;
    /** The running job writes a checksum file (Settings). */
    checksumFile?: boolean;
    /** In a queue run: this job's place (0-based) and the number of jobs. */
    queue?: { index: number; count: number };
    /** App-wide messages, shown first. */
    banner?: Snippet;
  } = $props();

  const api = useApi();
  // The meters aren't reactive; each update recomputes the figures from them.
  const copyMeter = new RateMeter();
  const verifyMeter = new RateMeter();
  /** The whole job's work: bytes copied, plus bytes verified in Copy & Verify. */
  const jobMeter = new RateMeter();
  let copySpeed: number | null = $state(null);
  let verifySpeed: number | null = $state(null);
  /** Milliseconds left for the whole job; `null` until there is a speed to go by. */
  let timeLeft: number | null = $state(null);

  const work = $derived(check || !progress.verify ? progress.totalBytes : 2 * progress.totalBytes);
  const workDone = $derived(
    check ? progress.verifiedBytes : progress.copiedBytes + (progress.verify ? progress.verifiedBytes : 0),
  );

  $effect(() => {
    copyMeter.push(progress.elapsedMs, progress.copiedBytes);
    verifyMeter.push(progress.elapsedMs, progress.verifiedBytes);
    jobMeter.push(progress.elapsedMs, workDone);
    copySpeed = copyMeter.current();
    verifySpeed = verifyMeter.current();
    timeLeft = jobMeter.eta(work - workDone);
  });

  const phase = $derived(
    progress.phase === "done"
      ? t("progress.phase.done")
      : progress.paused
        ? t("progress.phase.paused")
        : checking
          ? t("progress.phase.checking")
          : check
          ? t("progress.phase.verifying")
          : mirror
          ? t("progress.phase.mirroring")
          : progress.phase === "verifying"
            ? t("progress.phase.verifying")
            : progress.verify
              ? t("progress.phase.copyingVerifying")
              : t("progress.phase.copying"),
  );
  const files = $derived.by(() => {
    const parts = queue ? [t("progress.status.job", { index: queue.index + 1, count: queue.count })] : [];
    if (checking) return parts.join(t("format.dot"));
    parts.push(
      t("progress.status.files", {
        done: progress.filesDone,
        files: t("progress.files", { count: progress.totalFiles }),
      }),
    );
    if (progress.filesSkipped > 0) parts.push(t("progress.status.skipped", { count: progress.filesSkipped }));
    if (progress.filesFailed > 0) parts.push(t("progress.status.failed", { count: progress.filesFailed }));
    return parts.join(t("format.dot"));
  });

  /** Done, or a mirror's removals: short, and stopping halfway would help nobody. */
  const finishing = $derived(progress.phase === "done" || progress.phase === "removing");
  /** Cancel's question is open. */
  let asking = $state(false);
  /** Also remove the files already copied (#54); off each time it opens. */
  let removeCopied = $state(false);
  const kind: JobKind = $derived(check ? "check" : mirror ? "mirror" : "copy");
  const question = $derived(queue && checking ? t("progress.stop.queueOnly") : stopQuestion(kind, !!queue));
  // A job that ends (or starts removing) while the question is open has nothing to stop.
  $effect(() => {
    if (finishing) asking = false;
  });

  /** Asks, then stops the job (and the queue) as chosen. */
  export function cancel() {
    if (progress.phase === "done" || progress.phase === "removing") return;
    removeCopied = false;
    asking = true;
  }

  function stop() {
    asking = false;
    void api.cancelJob(removeCopied);
  }

  /**
   * Space pauses and resumes; not while a field, button or the file list has focus (Space is
   * theirs), and not with a modifier.
   */
  function onKey(e: KeyboardEvent) {
    if (e.key !== " " || e.repeat || finishing || asking) return;
    if (e.shiftKey || e.metaKey || e.ctrlKey || e.altKey) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest?.('input, textarea, select, button, [contenteditable], [tabindex="0"]')) return;
    e.preventDefault();
    void (progress.paused ? api.resumeJob() : api.pauseJob());
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet activeRow(name: string, path: string, doing: string, done: number, size: number, of: string, hint = "")}
  <tr>
    {#if hint}
      <td class="name explained"><Hint text={hint}>{name}</Hint></td>
    {:else}
      <td class="name" title={path}>{name}</td>
    {/if}
    <td class="doing">{doing}</td>
    <td class="meter">
      <div
        class="track"
        role="progressbar"
        aria-label={name}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(size === 0 ? 100 : (done * 100) / size)}
      >
        <div class="fill" style:width={formatPercent(done, size).replace(" ", "")}></div>
      </div>
    </td>
    <td class="of">{of}</td>
  </tr>
{/snippet}

<AppShell>
  {#snippet header()}
    <ScreenHeader title={phase}>
      {#snippet trailing()}
        {#if !checking}{t("progress.elapsed", { time: formatDuration(progress.elapsedMs) })}{/if}
      {/snippet}
    </ScreenHeader>
  {/snippet}

  {@render banner?.()}
  <p class="visually-hidden" aria-live="polite">{phase}</p>
  {#if progress.fatal}<Notice tone="danger">{t("progress.stopped", { why: progress.fatal })}</Notice>{/if}

  {#if checking}
    <Section title={t("progress.title")}>
      <p class="muted">
        {compared
          ? t("progress.comparing", { done: compared.done, files: t("progress.files", { count: compared.total }) })
          : t("progress.lookingAtSource")}
      </p>
    </Section>
  {:else}
    <Section title={t("progress.title")}>
      {#snippet aside()}
        <span class="overall">
          <strong>{formatPercent(workDone, work)}</strong>
          {#if progress.phase !== "done"}
            ·
            {#if timeLeft === null}
              <span>{t("progress.estimating")}</span>
            {:else}
              <span class="visually-hidden">{t("progress.timeLeftLabel")}</span><span title={t("progress.timeLeft")}>{formatDuration(timeLeft)}</span>
            {/if}
          {/if}
        </span>
      {/snippet}
      {#if check}
        <ProgressBar label={t("progress.bar.checked")} done={progress.verifiedBytes} total={progress.totalBytes} speed={verifySpeed} />
      {:else}
        <ProgressBar label={t("progress.bar.copied")} done={progress.copiedBytes} total={progress.totalBytes} speed={copySpeed} />
      {/if}
      {#if progress.verify && !check}
        <ProgressBar label={t("progress.bar.verified")} done={progress.verifiedBytes} total={progress.totalBytes} speed={verifySpeed} />
      {/if}
      {#if progress.phase === "removing" && progress.undoing}
        <p class="muted">{t("progress.undoing")}</p>
      {:else if progress.phase === "removing"}
        <p class="muted">
          {t(progress.archiving ? "progress.archiving" : "progress.deleting", { count: progress.removing })}
        </p>
      {/if}
    </Section>

    <Section title={t("progress.active")}>
      {#if progress.active.length === 0 && !progress.smallFiles && progress.filesDone === 0}
        <p class="muted">{t("progress.starting")}</p>
      {/if}
      <table>
        <tbody>
          <!-- Verifying is a new row: one bar never runs from 100 % back to 0. -->
          {#each progress.active as f (`${f.id}-${f.verifying}`)}
            {@render activeRow(
              f.name,
              f.path,
              f.verifying ? t("progress.file.verifying") : t("progress.file.copying"),
              f.bytesDone,
              f.size,
              t("progress.file.of", { done: formatBytes(f.bytesDone), size: formatBytes(f.size) }),
            )}
          {/each}
          <!-- Small files: one steady row for the whole job, counted in files, so it never
               jumps back or vanishes while many finish at once. -->
          {#if progress.smallFiles}
            {@const small = progress.smallFiles}
            {@render activeRow(
              t("progress.smallFiles.name"),
              "",
              "",
              small.done,
              small.total,
              t("progress.smallFiles.of", { done: small.done, total: small.total }),
              t("progress.smallFiles.help"),
            )}
          {/if}
        </tbody>
      </table>
    </Section>

    <FinishedList total={progress.filesDone} failedTotal={progress.filesFailed} updated={progress.elapsedMs} />
  {/if}

  {#snippet actions()}
    <ActionBar status={files}>
      {#snippet start()}
        {#if progress.paused}
          <Button help={t("progress.resumeHelp")} onclick={() => api.resumeJob()}>{t("progress.resume")}</Button>
        {:else}
          <!-- Pause holds the job at the next buffer it reads or writes (FR-22). -->
          <Button
            help={t(check ? "progress.pauseHelp.check" : "progress.pauseHelp.copy")}
            onclick={() => api.pauseJob()}
            disabled={finishing || checking}>{t("progress.pause")}</Button
          >
        {/if}
        <!-- ⌘. asks the same question (App's File menu). -->
        <Button
          variant="danger"
          help={t(queue ? "progress.cancelHelp.queue" : "progress.cancelHelp.job")}
          onclick={cancel}
          disabled={finishing}>{t("progress.cancel")}</Button
        >
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

{#if asking}
  <Dialog title={question} onClose={() => (asking = false)}>
    <p>
      {checking
        ? notStarted()
        : check
          ? t("progress.stop.checkedSoFar")
          : removeCopied
            ? t("progress.stop.removeCopied")
            : stopMessage(kind, checksumFile)}
    </p>
    {#if !checking && !check}
      <Checkbox label={t("progress.stop.alsoRemove")} checked={removeCopied} onChange={(on) => (removeCopied = on)}>
        {#snippet help()}
          {t("progress.stop.alsoRemoveHelp")}
        {/snippet}
      </Checkbox>
    {/if}
    {#snippet actions()}
      <Button data-autofocus onclick={() => (asking = false)}>{t("progress.stop.continue")}</Button>
      <Button variant="danger" onclick={stop}>{t("progress.stop.stop")}</Button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .muted {
    color: var(--text-muted);
    margin: 0;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-sm);
  }

  td {
    padding: 3px var(--space-2) 3px 0;
    white-space: nowrap;
  }

  td.name {
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Room for the explanation to open outside the cell. */
  td.name.explained {
    overflow: visible;
  }

  td.doing {
    width: 80px;
    color: var(--text-muted);
  }

  td.meter {
    width: 30%;
  }

  td.of {
    width: 150px;
    text-align: right;
    color: var(--text-muted);
  }

  .track {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-raised);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.5s linear;
  }

  .overall {
    font-size: var(--text-md);
    color: var(--text-muted);
  }

  .overall strong {
    font-size: var(--text-lg);
    color: var(--text);
  }
</style>
