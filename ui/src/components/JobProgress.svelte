<script lang="ts">
  // The progress view (RFD §5.3): phase and time, one bar per phase, active files, finished
  // files, Pause / Resume and Cancel.
  import { doing, NOT_STARTED, stopMessage, type JobKind } from "../lib/stopping";
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
    title,
    checking = false,
    compared,
    check = false,
  }: {
    progress: ProgressView;
    /** What the job is called while it runs ("Mirroring"); by default Copying. */
    title?: string;
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
      ? "Done"
      : progress.paused
        ? "Paused"
        : checking
          ? "Checking…"
          : check
          ? "Verifying"
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
  const kind: JobKind = $derived(check ? "check" : title === "Mirroring" ? "mirror" : "copy");
  const question = $derived(
    queue && checking
      ? "Stop the queue?"
      : queue
        ? `Stop ${doing(kind)} and stop the queue?`
        : `Stop ${doing(kind)}?`,
  );
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
    const t = e.target as HTMLElement | null;
    if (t?.closest?.('input, textarea, select, button, [contenteditable], [tabindex="0"]')) return;
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
        {#if !checking}{formatDuration(progress.elapsedMs)} elapsed{/if}
      {/snippet}
    </ScreenHeader>
  {/snippet}

  {@render banner?.()}
  <p class="visually-hidden" aria-live="polite">{phase}</p>
  {#if progress.fatal}<Notice tone="danger">Stopped: {progress.fatal}</Notice>{/if}

  {#if checking}
    <Section title="Progress">
      <p class="muted">
        {compared
          ? `Comparing contents: ${formatCount(compared.done)} of ${plural(compared.total, "file")}`
          : "Looking at the source and the destination before this job starts…"}
      </p>
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
      {#if check}
        <ProgressBar label="Checked" done={progress.verifiedBytes} total={progress.totalBytes} speed={verifySpeed} />
      {:else}
        <ProgressBar label="Copied" done={progress.copiedBytes} total={progress.totalBytes} speed={copySpeed} />
      {/if}
      {#if progress.verify && !check}
        <ProgressBar label="Verified" done={progress.verifiedBytes} total={progress.totalBytes} speed={verifySpeed} />
      {/if}
      {#if progress.phase === "removing" && progress.undoing}
        <p class="muted">Putting the destination back as it was…</p>
      {:else if progress.phase === "removing"}
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
            {@render activeRow(
              f.name,
              f.path,
              f.verifying ? "Verifying" : "Copying",
              f.bytesDone,
              f.size,
              `${formatBytes(f.bytesDone)} of ${formatBytes(f.size)}`,
            )}
          {/each}
          <!-- Small files: one steady row for the whole job, counted in files, so it never
               jumps back or vanishes while many finish at once. -->
          {#if progress.smallFiles}
            {@const small = progress.smallFiles}
            {@render activeRow(
              "Small files",
              "",
              "",
              small.done,
              small.total,
              `${formatCount(small.done)} of ${formatCount(small.total)}`,
              "Files under 8 MB. Many are copied at once, so they're counted together instead of getting a bar each.",
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
          <Button help="Carries on from where it paused (Space)." onclick={() => api.resumeJob()}>Resume</Button>
        {:else}
          <!-- Pause holds the job at the next buffer it reads or writes (FR-22). -->
          <Button
            help="Stops {check ? 'reading' : 'reading and writing'} until you resume (Space)."
            onclick={() => api.pauseJob()}
            disabled={finishing || checking}>Pause</Button
          >
        {/if}
        <!-- ⌘. asks the same question (App's File menu). -->
        <Button
          variant="danger"
          help="Asks first, then stops {queue ? 'this job and the queue' : 'the job'} (⌘.)."
          onclick={cancel}
          disabled={finishing}>Cancel</Button
        >
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

{#if asking}
  <Dialog title={question} onClose={() => (asking = false)}>
    <p>
      {checking
        ? NOT_STARTED
        : check
          ? "Nothing was changed: the files checked so far are in the summary."
          : removeCopied
            ? "The file in progress and the files already copied are removed."
            : stopMessage(kind, checksumFile)}
    </p>
    {#if !checking && !check}
      <Checkbox label="Also remove the files already copied" checked={removeCopied} onChange={(on) => (removeCopied = on)}>
        {#snippet help()}
          The destination goes back to how it was. Files this job replaced come back only from a mirror's archive.
        {/snippet}
      </Checkbox>
    {/if}
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
