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
  import Notice from "../lib/ui/Notice.svelte";
  import ProgressBar from "../lib/ui/ProgressBar.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import FinishedList from "./FinishedList.svelte";

  let {
    progress,
    checksumFile = true,
    banner,
  }: {
    progress: ProgressView;
    /** The running job writes a checksum file (Settings). */
    checksumFile?: boolean;
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
        : progress.phase === "verifying"
          ? "Verifying"
          : progress.verify
            ? "Copying & verifying"
            : "Copying",
  );
  const files = $derived.by(() => {
    const parts = [`${formatCount(progress.filesDone)} / ${plural(progress.totalFiles, "file")}`];
    if (progress.filesSkipped > 0) parts.push(`${formatCount(progress.filesSkipped)} skipped`);
    if (progress.filesFailed > 0) parts.push(`${formatCount(progress.filesFailed)} failed`);
    return parts.join(" · ");
  });

  export async function cancel() {
    const stop = await api.confirm(stopMessage(checksumFile), "Stop copying?");
    if (stop) await api.cancelJob();
  }

  /**
   * Space pauses and resumes; not while a field, button or the file list has focus (Space is
   * theirs), and not with a modifier.
   */
  function onKey(e: KeyboardEvent) {
    if (e.key !== " " || e.repeat || progress.phase === "done") return;
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
      {#snippet trailing()}{formatDuration(progress.elapsedMs)} elapsed{/snippet}
    </ScreenHeader>
  {/snippet}

  {@render banner?.()}
  <p class="visually-hidden" aria-live="polite">{phase}</p>
  {#if progress.fatal}<Notice tone="danger">Stopped: {progress.fatal}</Notice>{/if}

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
  </Section>

  <Section title="Active">
    {#if progress.active.length === 0 && !progress.smallFiles && progress.filesDone === 0}
      <p class="muted">Starting…</p>
    {/if}
    <table>
      <tbody>
        {#each progress.active as f (f.id)}
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

  {#snippet actions()}
    <ActionBar status={files}>
      {#snippet start()}
        {#if progress.paused}
          <Button onclick={() => api.resumeJob()}>Resume</Button>
        {:else}
          <Button onclick={() => api.pauseJob()} disabled={progress.phase === "done"}>Pause</Button>
        {/if}
        <Button variant="danger" onclick={cancel} disabled={progress.phase === "done"}>Cancel</Button>
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

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
