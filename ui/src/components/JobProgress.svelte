<script lang="ts">
  // The progress view (RFD §5.3): phase and time, one bar per phase, active files, finished
  // files, Pause / Resume and Cancel.
  import { stopMessage } from "../lib/stopping";
  import { useApi } from "../lib/api";
  import type { ProgressView } from "../lib/bindings";
  import { formatBytes, formatCount, formatDuration, formatPercent, plural } from "../lib/format";
  import { RateMeter } from "../lib/rate";
  import type { Snippet } from "svelte";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";
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
    title,
    checking = false,
  }: {
    progress: ProgressView;
    /** What the job is called while it runs ("Mirroring"); by default Copying. */
    title?: string;
    /** A queue job is being checked before it starts: nothing is copied yet. */
    checking?: boolean;
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

  const work = $derived(progress.verify ? 2 * progress.totalBytes : progress.totalBytes);
  const workDone = $derived(progress.copiedBytes + (progress.verify ? progress.verifiedBytes : 0));

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
      ? "Done"
      : progress.paused
        ? "Paused"
        : checking
          ? "Checking…"
          : title
          ? title
          : progress.phase === "verifying"
            ? "Verifying"
            : progress.verify
              ? "Copying & verifying"
              : "Copying",
  );
  const files = $derived.by(() => {
    const parts = queue ? [`Job ${queue.index + 1} of ${queue.count}`] : [];
    if (checking) return parts.join(" · ");
    parts.push(`${formatCount(progress.filesDone)} / ${plural(progress.totalFiles, "file")}`);
    if (progress.filesSkipped > 0) parts.push(`${formatCount(progress.filesSkipped)} skipped`);
    if (progress.filesFailed > 0) parts.push(`${formatCount(progress.filesFailed)} failed`);
    return parts.join(" · ");
  });

  /** Done, or a mirror's removals: short, and stopping halfway would help nobody. */
  const finishing = $derived(progress.phase === "done" || progress.phase === "removing");
  /** Cancel's question is open. */
  let asking = $state(false);
  /** Also remove the files already copied (#54); off each time it opens. */
  let removeCopied = $state(false);
  const question = $derived(
    queue ? "Stop copying and stop the queue?" : title === "Mirroring" ? "Stop mirroring?" : "Stop copying?",
  );
  // A job that ends while the question is open has nothing left to stop.
  $effect(() => {
    if (progress.phase === "done") asking = false;
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
    const t = e.target as HTMLElement | null;
    if (t?.closest?.('input, textarea, select, button, [contenteditable], [tabindex="0"]')) return;
    e.preventDefault();
    void (progress.paused ? api.resumeJob() : api.pauseJob());
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet activeRow(name: string, path: string, doing: string, done: number, size: number)}
  <tr>
    <td class="name" title={path}>{name}</td>
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
    <td class="of">{formatBytes(done)} of {formatBytes(size)}</td>
  </tr>
{/snippet}

<AppShell>
  {#snippet header()}
    <ScreenHeader title={phase}>
      {#snippet trailing()}
        {formatDuration(progress.elapsedMs)} elapsed
      {/snippet}
    </ScreenHeader>
  {/snippet}

  {@render banner?.()}
  <p class="visually-hidden" aria-live="polite">{phase}</p>
  {#if progress.fatal}<Notice tone="danger">Stopped: {progress.fatal}</Notice>{/if}

  {#if checking}
    <Section title="Progress">
      <p class="muted">Looking at the source and the destination before this job starts…</p>
    </Section>
  {:else}
    <Section title="Progress">
      {#snippet aside()}
        <span class="overall">
          <strong>{formatPercent(workDone, work)}</strong>
          {#if progress.phase !== "done"}
            ·
            {#if timeLeft === null}
              <span>Estimating…</span>
            {:else}
              <span class="visually-hidden">Time left:</span><span title="Time left">{formatDuration(timeLeft)}</span>
            {/if}
          {/if}
        </span>
      {/snippet}
      <ProgressBar label="Copied" done={progress.copiedBytes} total={progress.totalBytes} speed={copySpeed} />
      {#if progress.verify}
        <ProgressBar label="Verified" done={progress.verifiedBytes} total={progress.totalBytes} speed={verifySpeed} />
      {/if}
      {#if progress.phase === "removing"}
        <p class="muted">{progress.archiving ? "Archiving" : "Deleting"} {plural(progress.removing, "file")}</p>
      {/if}
    </Section>

    <Section title="Active">
      {#if progress.active.length === 0 && !progress.smallFiles && progress.filesDone === 0}
        <p class="muted">Starting…</p>
      {/if}
      <table>
        <tbody>
          <!-- Verifying is a new row: one bar never runs from 100 % back to 0. -->
          {#each progress.active as f (`${f.id}-${f.verifying}`)}
            {@render activeRow(f.name, f.path, f.verifying ? "Verifying" : "Copying", f.bytesDone, f.size)}
          {/each}
          {#if progress.smallFiles}
            {@const small = progress.smallFiles}
            {@render activeRow(`+ ${plural(small.count, "small file")}`, "", "", small.bytesDone, small.size)}
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
          <Button onclick={() => api.resumeJob()}>Resume</Button>
        {:else}
          <Button onclick={() => api.pauseJob()} disabled={finishing || checking}>Pause</Button>
        {/if}
        <Button variant="danger" onclick={cancel} disabled={finishing}>Cancel</Button>
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

{#if asking}
  <Dialog title={question} onClose={() => (asking = false)}>
    <p>
      {removeCopied ? "The file in progress and the files already copied are removed." : stopMessage(checksumFile)}
    </p>
    <Checkbox label="Also remove the files already copied" checked={removeCopied} onChange={(on) => (removeCopied = on)}>
      {#snippet help()}
        The destination goes back to how it was. Files this job replaced come back only from a mirror's archive.
      {/snippet}
    </Checkbox>
    {#snippet actions()}
      <Button data-autofocus onclick={() => (asking = false)}>Continue</Button>
      <Button variant="danger" onclick={stop}>Stop</Button>
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
