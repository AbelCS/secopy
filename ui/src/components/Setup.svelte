<script lang="ts">
  // The main window (RFD §5.2): FROM, TO, what pre-flight found, mode, Start.
  import { onMount } from "svelte";
  import { useApi } from "../lib/api";
  import type { ConflictPolicy, Profile, ProfilesView, SessionView, Settings } from "../lib/bindings";
  import { formatBytes, formatCount, plural } from "../lib/format";
  import type { Snippet } from "svelte";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import SegmentedControl from "../lib/ui/SegmentedControl.svelte";
  import DrivesRow from "./DrivesRow.svelte";
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
    if (settings.showHiddenCount && source.skippedHidden > 0) parts.push(`${formatCount(source.skippedHidden)} hidden items skipped`);
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

  /** Why Start can't be used yet, or where the files will go. */
  const startStatus = $derived.by(() => {
    if (scanning > 0) return "Waiting for the scan…";
    if (checking > 0) return "Waiting for the destination check…";
    if (!source) return "Pick what to copy.";
    if (!destination) return "Choose where to copy to.";
    if (destination.blocker || view.plan?.blocker) return "Something above blocks the copy.";
    if (!view.plan || view.plan.filesToWrite === 0) return "Nothing to copy.";
    return `To ${destination.copyRoot}`;
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
    <div class="rows">
      <div class="pick" role="group" aria-label="Source">
        <DrivesRow source={source?.folder ?? null} onPick={(path) => scan([path])} />
        <Button onclick={chooseSource}>Choose…</Button>
      </div>
      <ProfileBar
        {view}
        {profiles}
        busy={scanning > 0}
        onSelect={selectProfile}
        onApplied={profilesApplied}
        onManage={onManageProfiles}
      />
      <div>
        {#if scanning > 0}<p class="muted" role="status">Scanning…</p>{/if}
        {#if view.pickProblem}<Notice tone="danger">{view.pickProblem}</Notice>{/if}
        {#if source}
          <p class="path mono">{source.label}</p>
          <p class="muted">{sourceSummary}</p>
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
      </div>

      <!-- A retry copies exactly the files that failed: nothing to choose there. -->
      {#if source?.folder && !source.isRetry}
        {@const folder = source.folder}
        <!-- On: DEST/DCIM/…; off: only what's inside, straight into DEST (FR-4). -->
        <Checkbox
          label="Include the “{baseName(folder)}” directory"
          checked={!source.contentsOnly}
          disabled={scanning > 0}
          onChange={setIncludeFolder}
        />
        {#if source.extensions.length > 0}
          <ExtensionChips extensions={source.extensions} selected={source.selectedExtensions} onChange={setFilter} />
        {/if}
      {/if}
    </div>
  </Section>

  <Section title="To" data-drop="to">
    <div class="rows">
      <div>
        {#if checking > 0}<p class="muted" role="status">Checking…</p>{/if}
        {#if destination}
          <p class="path mono">{destination.path}</p>
          {#if !destination.blocker}
            <p class="muted">{formatBytes(destination.freeBytes)} free · {destination.fsKind}</p>
          {/if}
        {:else}
          <p class="muted">Drop the destination directory here, or choose it.</p>
        {/if}
      </div>
      <div class="pick">
        <Button onclick={chooseDestination}>Choose…</Button>
        {#if recent.length > 0}
          <select aria-label="Recent destinations" value="" onchange={(e) => chooseRecent(e.currentTarget)}>
            <option value="" disabled>Recent…</option>
            {#each recent as r (r)}<option value={r}>{r}</option>{/each}
          </select>
        {/if}
      </div>
      {#if destError}<Notice tone="danger">{destError}</Notice>{/if}
      {#if destination && source}
        <p>Files will go to: <span class="mono">{destination.copyRoot}</span></p>
        <PreflightPanel {destination} plan={view.plan} conflicts={view.conflicts} onConflicts={setConflicts} />
      {:else if destination?.blocker}
        <Notice tone="danger">{destination.blocker}</Notice>
      {/if}
    </div>
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
      {/snippet}
      {#snippet end()}
        <Button variant="primary" disabled={!canStart} onclick={onStart}>
          {#if view.plan && view.plan.filesToWrite > 0}
            {verify ? "Copy & verify" : "Copy"}
            {plural(view.plan.filesToWrite, "file")} · {formatBytes(view.plan.bytesToWrite)}
          {:else}
            Start copy
          {/if}
        </Button>
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .pick {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  p {
    margin: 0 0 var(--space-1);
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
