<script lang="ts">
  // The main window (RFD §5.2): FROM, TO, what pre-flight found, mode, Start.
  import { onMount } from "svelte";
  import { useApi } from "../lib/api";
  import type { ConflictPolicy, Profile, ProfilesView, QueueView, SessionView, Settings } from "../lib/bindings";
  import { formatBytes, plural } from "../lib/format";
  import type { Snippet } from "svelte";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import SegmentedControl from "../lib/ui/SegmentedControl.svelte";
  import FormRow from "../lib/ui/FormRow.svelte";
  import Hint from "../lib/ui/Hint.svelte";
  import ExtensionChips from "./ExtensionChips.svelte";
  import PreflightPanel from "./PreflightPanel.svelte";
  import ProfileBar from "./ProfileBar.svelte";

  let {
    view = $bindable(),
    verify = $bindable(true),
    profiles,
    settings,
    recent,
    onStart,
    onProfiles,
    onManageProfiles,
    onMode,
    onSettings,
    banner,
    ready = $bindable(false),
    onQueued,
  }: {
    view: SessionView;
    verify: boolean;
    profiles: Profile[];
    settings: Settings;
    /** Recent destinations that still exist (spec B7). */
    recent: string[];
    onStart: () => void;
    onProfiles: (profiles: Profile[]) => void;
    onManageProfiles: () => void;
    /** Copy or Copy & Verify was chosen; remembered for next time (FR-36). */
    onMode: (verify: boolean) => void;
    onSettings?: () => void;
    /** App-wide messages, shown first. */
    banner?: Snippet;
    /** Start is enabled (for the File menu's Start Copy). */
    ready?: boolean;
    /** Add to queue saved this setup; the queue as it is now. */
    onQueued?: (queue: QueueView) => void;
  } = $props();

  const api = useApi();

  /** Scans and destination checks still running; Start waits for all of them. */
  let scanning = $state(0);
  let checking = $state(0);
  let sourceError: string | null = $state(null);
  let destError: string | null = $state(null);

  const source = $derived(view.source);
  /** "1,284 files · 212.4 GB · 37 system files skipped" (FR-3, FR-13, FR-24) */
  const sourceSummary = $derived.by((): { text: string; hint?: string }[] => {
    if (!source) return [];
    const parts: { text: string; hint?: string }[] = [
      { text: plural(source.files, "file") },
      { text: formatBytes(source.bytes) },
    ];
    if (settings.showSystemCount && source.skippedSystem > 0)
      parts.push({
        text: `${plural(source.skippedSystem, "system file")} skipped`,
        hint: "Files computers leave on a card, like .DS_Store, ._ files and Thumbs.db. They're never copied.",
      });
    if (source.skippedSymlinks > 0) parts.push({ text: `${plural(source.skippedSymlinks, "symlink")} skipped` });
    return parts;
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

  /** Why Start can't be used yet, or where the files will go. */
  /** "Added to the queue (3 jobs).", for a few seconds after Add to queue. */
  let queuedNote: string | null = $state(null);

  async function addToQueue() {
    try {
      const queue = await api.addToQueue(verify);
      onQueued?.(queue);
      queuedNote = `Added to the queue (${plural(queue.jobs.length, "job")}).`;
      setTimeout(() => (queuedNote = null), 3000);
      await update(() => api.clearSource(), (e) => (sourceError = e));
    } catch (e) {
      sourceError = e instanceof Error ? e.message : String(e);
    }
  }

  const startStatus = $derived.by(() => {
    if (queuedNote) return queuedNote;
    if (scanning > 0) return "Waiting for the scan…";
    if (checking > 0) return "Waiting for the destination check…";
    if (!source) return "Pick what to copy.";
    if (!destination) return "Choose where to copy to.";
    if (destination.blocker || view.plan?.blocker) return "Something above blocks the copy.";
    if (!view.plan || view.plan.filesToWrite === 0) return "Nothing to copy.";
    // Ready: what Start will copy.
    return `${plural(view.plan.filesToWrite, "file")} · ${formatBytes(view.plan.bytesToWrite)}`;
  });

  function setMode(mode: "copy" | "verify") {
    verify = mode === "verify";
    onMode(verify);
  }

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

  function selectProfile(id: string | null) {
    return update(() => api.selectProfile(id), (e) => (sourceError = e), "scan");
  }

  function profilesApplied(result: ProfilesView) {
    onProfiles(result.profiles);
    view = result.session;
  }

  function chooseRecent(menu: HTMLSelectElement) {
    const path = menu.value;
    menu.value = "";
    if (path) void setDestination(path);
  }

  $effect(() => {
    ready = canStart;
  });

  /** Start from the File menu: only what the Start button would start. */
  export function startIfReady() {
    if (canStart) onStart();
  }

  /** The last part of a path: "/Volumes/CARD/DCIM" → "DCIM". */
  const baseName = (path: string) => path.split("/").filter(Boolean).pop() ?? path;

  export async function chooseSource() {
    const paths = await api.pickSource();
    if (paths) await scan(paths);
  }

  function setDestination(path: string) {
    return update(() => api.setDestination(path), (e) => (destError = e));
  }

  export async function chooseDestination() {
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

<AppShell>
  {#snippet header()}
    <ScreenHeader title="New copy">
      {#snippet trailing()}
        {#if onSettings}<Button icon="settings" onclick={onSettings}>Settings</Button>{/if}
      {/snippet}
    </ScreenHeader>
  {/snippet}

  {@render banner?.()}

  <Section title="From" data-drop="from">
    <FormRow label="Source">
      {#if scanning > 0}<p class="muted" role="status">Scanning…</p>{/if}
      {#if view.pickProblem}<Notice tone="danger">{view.pickProblem}</Notice>{/if}
      {#if source}
        <div>
          <p class="path mono">{source.label}</p>
          <p class="muted">
            {#each sourceSummary as part, i (i)}{#if i > 0}{" · "}{/if}{#if part.hint}<Hint text={part.hint}
                  >{part.text}</Hint
                >{:else}{part.text}{/if}{/each}
          </p>
        </div>
        {#if source.problemCount > 0}
          <details class="unreadable">
            <summary>{plural(source.problemCount, "item")} couldn't be read</summary>
            <ul>
              {#each source.problems as p (p)}<li class="mono">{p}</li>{/each}
            </ul>
          </details>
        {/if}
      {:else}
        <p class="muted">Drop a directory or files here, or choose them.</p>
      {/if}
      {#if sourceError}<Notice tone="danger">{sourceError}</Notice>{/if}
      {#snippet aside()}<Button onclick={chooseSource}>Choose…</Button>{/snippet}
    </FormRow>
    <FormRow label="Profile">
      <ProfileBar
        {view}
        {profiles}
        busy={scanning > 0}
        onSelect={selectProfile}
        onApplied={profilesApplied}
        onManage={onManageProfiles}
      />
    </FormRow>

    <!-- A retry copies exactly the files that failed: nothing to choose there. -->
    {#if source?.folder && !source.isRetry}
      {@const folder = source.folder}
      <FormRow label="Options">
        <!-- On: DEST/DCIM/…; off: only what's inside, straight into DEST (FR-4). -->
        <Checkbox
          label="Include the “{baseName(folder)}” directory"
          checked={!source.contentsOnly}
          disabled={scanning > 0}
          onChange={setIncludeFolder}
        />
      </FormRow>
      {#if source.extensions.length > 0}
        <FormRow label="File types">
          <ExtensionChips extensions={source.extensions} selected={source.selectedExtensions} onChange={setFilter} />
          {#snippet aside()}
            <Button variant="link" onclick={() => setFilter(null)}>All</Button>
            <Button variant="link" onclick={() => setFilter([])}>None</Button>
          {/snippet}
        </FormRow>
      {/if}
    {/if}
  </Section>

  <Section title="To" data-drop="to">
    <FormRow label="Destination">
      {#if checking > 0}<p class="muted" role="status">Checking…</p>{/if}
      {#if destination}
        <div>
          <p class="path mono">{destination.path}</p>
          {#if !destination.blocker}
            <p class="muted">{formatBytes(destination.freeBytes)} free · {destination.fsKind}</p>
          {/if}
        </div>
      {:else}
        <p class="muted">Drop the destination directory here, or choose it.</p>
      {/if}
      {#if destError}<Notice tone="danger">{destError}</Notice>{/if}
      {#if destination?.blocker && !source}<Notice tone="danger">{destination.blocker}</Notice>{/if}
      {#snippet aside()}
        {#if recent.length > 0}
          <select aria-label="Recent destinations" value="" onchange={(e) => chooseRecent(e.currentTarget)}>
            <option value="" disabled>Recent…</option>
            {#each recent as r (r)}<option value={r}>{r}</option>{/each}
          </select>
        {/if}
        <Button onclick={chooseDestination}>Choose…</Button>
      {/snippet}
    </FormRow>
    {#if destination && source}
      <PreflightPanel {destination} plan={view.plan} conflicts={view.conflicts} onConflicts={setConflicts} />
    {/if}
  </Section>

  {#snippet actions()}
    <ActionBar status={startStatus}>
      {#snippet start()}
        <SegmentedControl
          label="Mode"
          options={[
            { value: "copy", label: "Copy" },
            { value: "verify", label: "Copy & Verify" },
          ]}
          value={verify ? "verify" : "copy"}
          onChange={setMode}
        />
        <Hint
          above
          label="About verifying"
          text="Copy & Verify reads every copy back from the destination and checks it against the source's checksum; a copy that doesn't match is copied again. Copy only copies: faster, but nothing is checked."
        />
      {/snippet}
      {#snippet end()}
        <!-- The mode is chosen next to it; the figures are in the status. -->
        <Button disabled={!canStart || !!source?.isRetry} onclick={addToQueue}>Add to queue</Button>
        <Button variant="primary" disabled={!canStart} onclick={onStart}>Start copy</Button>
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>


  p {
    margin: 0;
  }

  select {
    max-width: 220px;
  }

  .path {
    word-break: break-all;
  }

  .muted {
    color: var(--text-muted);
  }

  .unreadable summary {
    color: var(--warning);
    cursor: pointer;
  }
</style>
