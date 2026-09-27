<script lang="ts">
  // The main window (RFD §5.2): FROM, TO, what pre-flight found, mode, Start.
  import { onMount } from "svelte";
  import { useApi } from "../lib/api";
  import type { ConflictPolicy, SessionView } from "../lib/bindings";
  import { formatBytes, formatCount, plural } from "../lib/format";
  import ExtensionChips from "./ExtensionChips.svelte";
  import PreflightPanel from "./PreflightPanel.svelte";

  let {
    view = $bindable(),
    verify = $bindable(true),
    onStart,
  }: {
    view: SessionView;
    verify: boolean;
    onStart: () => void;
  } = $props();

  const api = useApi();

  /** Scans and destination checks still running; Start waits for all of them. */
  let scanning = $state(0);
  let checking = $state(0);
  let sourceError: string | null = $state(null);
  let destError: string | null = $state(null);

  const source = $derived(view.source);
  /** "1,284 files · 212.4 GB · 37 hidden items skipped" (FR-3, FR-13, FR-24) */
  const sourceSummary = $derived.by(() => {
    if (!source) return "";
    const parts = [plural(source.files, "file"), formatBytes(source.bytes)];
    if (source.skippedHidden > 0) parts.push(`${formatCount(source.skippedHidden)} hidden items skipped`);
    if (source.skippedSymlinks > 0) parts.push(`${plural(source.skippedSymlinks, "symlink")} skipped`);
    return parts.join(" · ");
  });
  const destination = $derived(view.destination);
  const canStart = $derived(
    scanning === 0 &&
      checking === 0 &&
      !!view.plan &&
      !view.plan.blocker &&
      !destination?.blocker &&
      view.plan.filesToWrite > 0,
  );

  /** Runs a session command; a newer scan's result replaces this one (`stale`). */
  async function update(
    call: () => Promise<SessionView>,
    setError: (e: string | null) => void,
    kind: "scan" | "check" = "check",
  ): Promise<void> {
    if (kind === "scan") scanning++;
    else checking++;
    try {
      const next = await call();
      if (!next.stale) view = next;
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (kind === "scan") scanning--;
      else checking--;
    }
  }

  function scan(paths: string[]) {
    return update(() => api.scanSource(paths), (e) => (sourceError = e), "scan");
  }

  function setIncludeFolder(include: boolean) {
    return update(() => api.setIncludeFolder(include), (e) => (sourceError = e), "scan");
  }

  /** The last part of a path: "/Volumes/CARD/DCIM" → "DCIM". */
  const baseName = (path: string) => path.split("/").filter(Boolean).pop() ?? path;

  async function chooseSource() {
    const paths = await api.pickSource();
    if (paths) await scan(paths);
  }

  function setDestination(path: string) {
    return update(() => api.setDestination(path), (e) => (destError = e));
  }

  async function chooseDestination() {
    const path = await api.pickDestination();
    if (path) await setDestination(path);
  }

  function setFilter(selected: (string | null)[] | null) {
    return update(() => api.setFilter(selected), (e) => (sourceError = e));
  }

  function setConflicts(policy: ConflictPolicy) {
    return update(() => api.setConflicts(policy), (e) => (destError = e));
  }

  onMount(() => {
    // Finder drops land on FROM or TO, whichever is under the pointer (FR-1, FR-2, FR-15).
    const unlisten = api.onDrop((paths, target) => {
      const zone = target?.closest("[data-drop]")?.getAttribute("data-drop");
      if (zone === "from") scan(paths);
      else if (zone === "to" && paths.length > 0) setDestination(paths[0]);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

<section class="card" data-drop="from" aria-labelledby="from-title">
  <h2 id="from-title">From</h2>
  {#if scanning > 0}<p class="muted" role="status">Scanning…</p>{/if}
  {#if view.pickProblem}<p class="danger" role="alert">{view.pickProblem}</p>{/if}
  {#if source}
    <p class="path mono">{source.label}</p>
    <p class="muted">{sourceSummary}</p>
    {#if source.problemCount > 0}
      <details class="warning">
        <summary>{plural(source.problemCount, "item")} couldn't be read</summary>
        <ul>
          {#each source.problems as p (p)}<li class="mono">{p}</li>{/each}
        </ul>
      </details>
    {/if}
  {:else}
    <p class="muted">Drop a folder or files here, or choose them.</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={chooseSource}>Choose…</button>
  </div>
  {#if sourceError}<p class="danger" role="alert">{sourceError}</p>{/if}

  <!-- A retry copies exactly the files that failed: nothing to choose there. -->
  {#if source?.folder && !source.isRetry}
    {@const folder = source.folder}
    <!-- On: DEST/DCIM/…; off: only what's inside, straight into DEST (FR-4). -->
    <label class="include">
      <input
        type="checkbox"
        checked={!source.contentsOnly}
        onchange={(e) => setIncludeFolder(e.currentTarget.checked)}
      />
      Include the “{baseName(folder)}” folder
    </label>
    {#if source.extensions.length > 0}
      <ExtensionChips
        extensions={source.extensions}
        selected={source.selectedExtensions}
        onChange={setFilter}
      />
    {/if}
  {/if}
</section>

<section class="card" data-drop="to" aria-labelledby="to-title">
  <h2 id="to-title">To</h2>
  {#if checking > 0}<p class="muted" role="status">Checking…</p>{/if}
  {#if destination}
    <p class="path mono">{destination.path}</p>
    {#if !destination.blocker}
      <p class="muted">{formatBytes(destination.freeBytes)} free · {destination.fsKind}</p>
    {/if}
  {:else}
    <p class="muted">Drop the destination folder here, or choose it.</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={chooseDestination}>Choose…</button>
  </div>
  {#if destError}<p class="danger" role="alert">{destError}</p>{/if}
  {#if destination && source}
    <p>Files will go to: <span class="mono">{destination.copyRoot}</span></p>
    <PreflightPanel
      {destination}
      plan={view.plan}
      conflicts={view.conflicts}
      onConflicts={setConflicts}
    />
  {:else if destination?.blocker}
    <p class="danger" role="alert">{destination.blocker}</p>
  {/if}
</section>

<footer class="start">
  <div class="segmented" role="radiogroup" aria-label="Mode">
    <label class:on={!verify}>
      <input type="radio" name="mode" checked={!verify} onchange={() => (verify = false)} />
      Copy
    </label>
    <label class:on={verify}>
      <input type="radio" name="mode" checked={verify} onchange={() => (verify = true)} />
      Copy & Verify
    </label>
  </div>
  <button type="button" class="primary" disabled={!canStart} onclick={onStart}>
    {#if view.plan && view.plan.filesToWrite > 0}
      {verify ? "Copy & verify" : "Copy"}
      {plural(view.plan.filesToWrite, "file")} · {formatBytes(view.plan.bytesToWrite)}
    {:else}
      Start copy
    {/if}
  </button>
</footer>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: var(--gap);
  }

  h2 {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  p {
    margin: 4px 0;
  }

  .path {
    word-break: break-all;
  }

  .muted {
    color: var(--text-muted);
  }

  .danger {
    color: var(--danger);
  }

  .warning {
    color: var(--warning);
  }

  .actions {
    display: flex;
    gap: 8px;
    margin: 8px 0;
  }

  .include {
    display: block;
    margin: 8px 0;
  }

  .start {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .segmented {
    display: flex;
    gap: 4px;
  }

  .segmented label {
    position: relative;
    padding: 6px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
  }

  .segmented label.on {
    border-color: var(--accent);
  }

  .segmented input {
    position: absolute;
    opacity: 0;
  }
</style>
