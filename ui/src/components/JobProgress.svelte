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
  type Figures = { speed: number | null; average: number | null; eta: number | null };
  const unknown: Figures = { speed: null, average: null, eta: null };
  let copy: Figures = $state(unknown);
  let verify: Figures = $state(unknown);

  $effect(() => {
    copyMeter.push(progress.elapsedMs, progress.copiedBytes);
    verifyMeter.push(progress.elapsedMs, progress.verifiedBytes);
    const left = (done: number) => progress.totalBytes - done;
    copy = {
      speed: copyMeter.current(),
      average: copyMeter.average(),
      eta: copyMeter.eta(left(progress.copiedBytes)),
    };
    verify = {
      speed: verifyMeter.current(),
      average: verifyMeter.average(),
      eta: verifyMeter.eta(left(progress.verifiedBytes)),
    };
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
    <ProgressBar label="Copied" done={progress.copiedBytes} total={progress.totalBytes} {...copy} />
    {#if progress.verify}
      <ProgressBar label="Verified" done={progress.verifiedBytes} total={progress.totalBytes} {...verify} />
    {/if}
  </Section>

  <Section title="Active">
    {#if progress.active.length === 0 && !progress.smallFiles}
      <p class="muted">—</p>
    {/if}
    <table>
      <tbody>
        {#each progress.active as f (f.id)}
          <tr>
            <td class="name" title={f.path}>{f.name}</td>
            <td>{f.verifying ? "Verifying" : "Copying"}</td>
            <td>{formatBytes(f.size)}</td>
            <td>{formatBytes(f.bytesDone)}</td>
            <td>{formatPercent(f.bytesDone, f.size)}</td>
          </tr>
        {/each}
        {#if progress.smallFiles}
          <tr>
            <td class="name">+ {plural(progress.smallFiles.count, "small file")}</td>
            <td></td>
            <td>{formatBytes(progress.smallFiles.size)}</td>
            <td>{formatBytes(progress.smallFiles.bytesDone)}</td>
            <td>{formatPercent(progress.smallFiles.bytesDone, progress.smallFiles.size)}</td>
          </tr>
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
</style>
