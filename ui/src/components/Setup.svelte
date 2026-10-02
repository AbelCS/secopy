<script lang="ts">
  // The main window (RFD §5.2): FROM, TO, what pre-flight found, mode, Start.
  import IgnoreList from "../lib/ui/IgnoreList.svelte";
  import { t, type Key } from "../lib/i18n";
  import { say } from "../lib/message";
  import { onMount } from "svelte";
  import { useApi } from "../lib/api";
  import type {
    ConflictPolicy,
    CopyPreset,
    CopyPresetsView,
    DestinationView,
    QueueView,
    SessionView,
    Settings,
  } from "../lib/bindings";
  import { baseName, formatBytes, messageOf } from "../lib/format";
  import { newest } from "../lib/session";
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
  import PresetBar from "./PresetBar.svelte";

  let {
    view = $bindable(),
    verify = $bindable(true),
    presets,
    settings,
    recent,
    onStart,
    onPresets,
    onManagePresets,
    onMode,
    banner,
    ready = $bindable(false),
    onQueued,
  }: {
    view: SessionView;
    verify: boolean;
    presets: CopyPreset[];
    settings: Settings;
    /** Recent destinations that still exist (spec B7). */
    recent: string[];
    onStart: () => void;
    onPresets: (presets: CopyPreset[]) => void;
    onManagePresets: () => void;
    /** Copy or Copy & Verify was chosen; remembered for next time (FR-36). */
    onMode: (verify: boolean) => void;
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
  /** "1,284 files · 212.4 GB · 37 ignored" (FR-3, FR-13, FR-24, #158) */
  const sourceSummary = $derived.by((): { text: string; hint?: string }[] => {
    if (!source) return [];
    const parts: { text: string; hint?: string }[] = [
      { text: t("copy.files", { count: source.files }) },
      { text: formatBytes(source.bytes) },
    ];
    if (settings.showSystemCount && source.ignored > 0)
      parts.push({
        text: t("copy.ignored", { count: source.ignored }),
        hint: t("copy.ignoredHint", { patterns: [...settings.ignore, ...view.jobIgnore].join(t("format.comma")) }),
      });
    if (source.skippedSymlinks > 0) parts.push({ text: t("copy.symlinksSkipped", { count: source.skippedSymlinks }) });
    if (source.skippedSpecial > 0)
      parts.push({ text: t("copy.specialSkipped", { count: source.skippedSpecial }), hint: t("copy.specialHint") });
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

  /** "Added to the queue (3 jobs).", for a few seconds after Add to queue. */
  let queuedNote: string | null = $state(null);

  /** Add to queue on its way: a second click doesn't add the job twice (#117). */
  let queueing = false;
  /** The view a job was added from: not added again until New copy changes, even when
   *  clearing after it failed (#138). */
  const queued: { at: number | null } = { at: null };

  async function addToQueue() {
    if (queueing || queued.at === view.revision) return;
    queueing = true;
    // The view the job is added from, as it was when asked (#138).
    const from = view.revision;
    try {
      const queue = await api.addToQueue(verify);
      queued.at = from;
      onQueued?.(queue);
      queuedNote = t("copy.queued", { count: queue.jobs.length });
      setTimeout(() => (queuedNote = null), 3000);
      await update(() => api.clearSource(), (e) => (sourceError = e));
    } catch (e) {
      sourceError = messageOf(e);
    } finally {
      queueing = false;
    }
  }

  /** Why Start can't be used yet, or what it will copy. */
  const startStatus = $derived.by(() => {
    if (queuedNote) return queuedNote;
    if (scanning > 0) return t("copy.status.scanning");
    if (checking > 0) return t("copy.status.checking");
    if (!source) return t("copy.status.noSource");
    if (!destination) return t("copy.status.noDestination");
    if (destination.blocker || view.plan?.blocker) return t("copy.status.blocked");
    if (!view.plan || view.plan.filesToWrite === 0) return t("copy.status.nothing");
    // Ready: what Start will copy, and what it replaces (#112).
    const ready = t("copy.status.ready", {
      files: t("copy.files", { count: view.plan.filesToWrite }),
      size: formatBytes(view.plan.bytesToWrite),
    });
    const replaces = view.plan.overwrites;
    return replaces > 0 ? ready + t("format.dot") + t("copy.status.replaces", { count: replaces }) : ready;
  });

  /** Start's help: the figures and the path the screen shows, and the File menu's shortcut. */
  const startHelp = $derived(
    view.plan && destination
      ? t(verify ? "copy.start.helpVerify" : "copy.start.help", {
          files: t("copy.files", { count: view.plan.filesToWrite }),
          size: formatBytes(view.plan.bytesToWrite),
          path: destination.copyRoot,
        })
      : "",
  );

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
      // The newest view, not the last to arrive: answers can come back out of order (#138).
      view = newest(view, next);
      setError(null);
    } catch (e) {
      setError(messageOf(e));
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

  /** Also ignore's row is open: its editor is shown. */
  let alsoOpen = $state(false);

  /** This copy's Also ignore (#164): the source is scanned again with it. */
  function setJobIgnore(list: string[]) {
    return update(() => api.setJobIgnore(list), (e) => (sourceError = e), "scan");
  }

  function selectCopyPreset(id: string | null) {
    return update(() => api.selectCopyPreset(id), (e) => (sourceError = e), "scan");
  }

  function presetsApplied(result: CopyPresetsView) {
    onPresets(result.presets);
    view = newest(view, result.session);
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

  /** The destination's file system, by its code from the app. */
  const FS: Record<string, Key> = {
    apfs: "copy.fs.apfs",
    hfs: "copy.fs.hfs",
    exfat: "copy.fs.exfat",
    fat32: "copy.fs.fat32",
    ntfs: "copy.fs.ntfs",
    smb: "copy.fs.smb",
    nfs: "copy.fs.nfs",
  };
  const fsName = (d: DestinationView) => (FS[d.fsKind] ? t(FS[d.fsKind]) : (d.fsName ?? d.fsKind));
</script>

<AppShell>
  {#snippet header()}
    <ScreenHeader title={t("copy.title")} />
  {/snippet}

  {@render banner?.()}

  <Section title={t("copy.from")} data-drop="from">
    <FormRow label={t("copy.source")}>
      {#if scanning > 0}<p class="muted" role="status">{t("copy.scanning")}</p>{/if}
      {#if view.pickProblem}<Notice tone="danger">{say(view.pickProblem)}</Notice>{/if}
      {#if source}
        <div>
          <p class="path mono">{say(source.label)}</p>
          <p class="muted">
            {#each sourceSummary as part, i (i)}{#if i > 0}{t("format.dot")}{/if}{#if part.hint}<Hint text={part.hint}
                  >{part.text}</Hint
                >{:else}{part.text}{/if}{/each}
          </p>
        </div>
        {#if source.problemCount > 0}
          <details class="unreadable">
            <summary>{t("copy.unreadable", { count: source.problemCount })}</summary>
            <ul>
              {#each source.problems as p, i (i)}<li class="mono">{say(p)}</li>{/each}
            </ul>
          </details>
        {/if}
      {:else}
        <p class="muted">{t("copy.dropSource")}</p>
      {/if}
      {#if sourceError}<Notice tone="danger">{sourceError}</Notice>{/if}
      {#snippet aside()}<Button onclick={chooseSource}>{t("copy.choose")}</Button>{/snippet}
    </FormRow>
    <FormRow label={t("copy.preset")}>
      <PresetBar
        {view}
        {presets}
        busy={scanning > 0}
        onSelect={selectCopyPreset}
        onApplied={presetsApplied}
        onManage={onManagePresets}
      />
    </FormRow>

    <!-- A retry copies exactly the files that failed: nothing to choose there. -->
    {#if source?.folder && !source.isRetry}
      {@const folder = source.folder}
      <FormRow label={t("copy.options")}>
        <!-- On: DEST/DCIM/…; off: only what's inside, straight into DEST (FR-4). -->
        <Checkbox
          label={t("copy.includeFolder", { name: baseName(folder) })}
          checked={!source.contentsOnly}
          disabled={scanning > 0}
          onChange={setIncludeFolder}
        />
      </FormRow>
      {#if source.extensions.length > 0}
        <FormRow label={t("copy.fileTypes")}>
          <ExtensionChips extensions={source.extensions} selected={source.selectedExtensions} onChange={setFilter} />
          {#snippet aside()}
            <Button variant="link" onclick={() => setFilter(null)}>{t("copy.all")}</Button>
            <Button variant="link" onclick={() => setFilter([])}>{t("copy.none")}</Button>
          {/snippet}
        </FormRow>
      {/if}
      <!-- On top of Settings' list, for this copy (and its preset): #164. -->
      <details class="also" bind:open={alsoOpen}>
        <summary>
          {view.jobIgnore.length > 0
            ? t("copy.alsoIgnoreCount", { count: view.jobIgnore.length })
            : t("copy.alsoIgnore")}
        </summary>
        {#if alsoOpen}
          <IgnoreList
            label={t("copy.alsoIgnore")}
            patterns={view.jobIgnore}
            global={settings.ignore}
            rows={4}
            onChange={setJobIgnore}
          />
        {/if}
      </details>
    {/if}
  </Section>

  <Section title={t("copy.to")} data-drop="to">
    <FormRow label={t("copy.destination")}>
      {#if checking > 0}<p class="muted" role="status">{t("copy.checking")}</p>{/if}
      {#if destination}
        <div>
          <p class="path mono">{destination.path}</p>
          {#if !destination.blocker}
            <p class="muted">{t("copy.free", { size: formatBytes(destination.availableBytes), kind: fsName(destination) })}</p>
          {/if}
        </div>
      {:else}
        <p class="muted">{t("copy.dropDestination")}</p>
      {/if}
      {#if destError}<Notice tone="danger">{destError}</Notice>{/if}
      {#if destination?.blocker && !source}<Notice tone="danger">{say(destination.blocker)}</Notice>{/if}
      {#snippet aside()}
        {#if recent.length > 0}
          <select aria-label={t("copy.recent")} value="" onchange={(e) => chooseRecent(e.currentTarget)}>
            <option value="" disabled>{t("copy.recentPlaceholder")}</option>
            {#each recent as r (r)}<option value={r}>{r}</option>{/each}
          </select>
        {/if}
        <Button onclick={chooseDestination}>{t("copy.choose")}</Button>
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
          label={t("copy.mode")}
          options={[
            { value: "copy", label: t("copy.modeCopy") },
            { value: "verify", label: t("copy.modeVerify") },
          ]}
          value={verify ? "verify" : "copy"}
          onChange={setMode}
        />
        <Hint
          above
          label={t("copy.aboutVerifying")}
          text={t("copy.aboutVerifyingText")}
        />
      {/snippet}
      {#snippet end()}
        <!-- The mode is chosen next to it; the figures are in the status. -->
        <Button
          disabled={!canStart || !!source?.isRetry}
          help={t("copy.addToQueueHelp")}
          onclick={addToQueue}>{t("copy.addToQueue")}</Button
        >
        <Button variant="primary" disabled={!canStart} help={startHelp} onclick={onStart}>{t("copy.start.label")}</Button>
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .also {
    margin-top: var(--space-2);
  }

  .also summary {
    cursor: pointer;
    color: var(--text-muted);
    font-size: var(--text-sm);
    margin-bottom: var(--space-2);
  }

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
