<script lang="ts">
  // What a mirror would do (FR-47): the counts, every file by kind, and what looks wrong
  // (FR-50). Run mirror runs exactly this.
  import { useApi } from "../lib/api";
  import type { MirrorPreviewView, PreviewKind, PreviewRow, QueueView } from "../lib/bindings";
  import { formatBytes, formatCount } from "../lib/format";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import Hint from "../lib/ui/Hint.svelte";
  import SegmentedControl from "../lib/ui/SegmentedControl.svelte";

  let {
    preview,
    onRun,
    onQueue,
    onCancel,
  }: {
    preview: MirrorPreviewView;
    onRun: () => void;
    onQueue: (queue: QueueView) => void;
    onCancel: () => void;
  } = $props();

  const api = useApi();
  const PAGE = 500;
  type Shown = "all" | PreviewKind;
  let shown = $state<Shown>("all");
  let rows: PreviewRow[] = $state([]);
  let error: string | null = $state(null);
  let busy = $state(false);

  const inSync = $derived(preview.newFiles + preview.changedFiles + preview.removedFiles === 0);
  const total = $derived(
    {
      all: preview.newFiles + preview.changedFiles + preview.removedFiles,
      new: preview.newFiles,
      changed: preview.changedFiles,
      removed: preview.removedFiles,
    }[shown],
  );

  $effect(() => {
    const kind = shown === "all" ? null : shown;
    void api
      .mirrorPreviewPage(kind, 0, PAGE)
      .then((r) => {
        // Only the list asked for last is shown.
        if ((shown === "all" ? null : shown) === kind) rows = r;
      })
      .catch((e) => (error = e instanceof Error ? e.message : String(e)));
  });

  async function run() {
    if (preview.guard) {
      const sure = await api.confirm(preview.guard, "Run the mirror anyway?", "Run", "Cancel");
      if (!sure) return;
    }
    onRun();
  }

  async function queue() {
    busy = true;
    try {
      onQueue(await api.addMirrorToQueue(preview.presetId));
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<AppShell>
  {#snippet header()}<ScreenHeader title={preview.name} />{/snippet}

  <p class="route">
    <span class="mono path" title={preview.origin}><bdi>{preview.origin}</bdi></span>
    <span class="muted" aria-hidden="true">→</span>
    <span class="mono path" title={preview.destination}><bdi>{preview.destination}</bdi></span>
  </p>
  {#if preview.guard}<Notice tone="warning">{preview.guard}</Notice>{/if}
  {#if error}<Notice tone="danger">{error}</Notice>{/if}

  <Section title="Changes">
    {#if inSync}
      <p>Already in sync.</p>
    {:else}
      <ul class="counts">
        <li><span class="sign" aria-hidden="true">+</span><span>{formatCount(preview.newFiles)} new</span>
          <span class="muted">{formatBytes(preview.newBytes)}</span></li>
        <li><span class="sign" aria-hidden="true">↻</span><span>{formatCount(preview.changedFiles)} changed</span>
          <span class="muted">{formatBytes(preview.changedBytes)}</span></li>
        <li><span class="sign" aria-hidden="true">−</span><span>
          {formatCount(preview.removedFiles)} deleted in the origin →
          {#if preview.archiveDays === null}deleted{:else}<Hint
              text="Moved into the hidden .secopy-archive directory in the destination, and removed for good after {formatCount(
                preview.archiveDays,
              )} days."
              >archived, kept {formatCount(preview.archiveDays)} days</Hint
            >{/if}
        </span></li>
      </ul>
    {/if}
    <p class="muted unchanged">{formatCount(preview.unchanged)} unchanged</p>
  </Section>

  {#if !inSync}
    <Section title="Files">
      {#snippet aside()}
        <SegmentedControl
          label="Show"
          options={[
            { value: "all" as Shown, label: "All" },
            { value: "new" as Shown, label: "New" },
            { value: "changed" as Shown, label: "Changed" },
            { value: "removed" as Shown, label: "Deleted" },
          ]}
          value={shown}
          onChange={(v) => (shown = v)}
        />
      {/snippet}
      <table>
        <tbody>
          {#each rows as r (`${r.kind}-${r.path}`)}
            <tr>
              <td class="mono name" title={r.path}><bdi>{r.path}</bdi></td>
              <td class="size">{formatBytes(r.size)}</td>
              <td class="muted">{r.reason}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if total > rows.length && rows.length === PAGE}
        <p class="muted">and {formatCount(total - rows.length)} more</p>
      {/if}
    </Section>
  {/if}

  {#snippet actions()}
    <ActionBar>
      {#snippet start()}<Button onclick={onCancel}>Cancel</Button>{/snippet}
      {#snippet end()}
        <Button disabled={busy} onclick={queue}>Add to queue</Button>
        <Button variant="primary" disabled={inSync} onclick={run}>Run mirror</Button>
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .route {
    display: flex;
    gap: var(--space-2);
    align-items: baseline;
    margin: 0 0 var(--space-3);
    min-width: 0;
  }

  .path {
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }

  .muted {
    color: var(--text-muted);
  }

  .counts {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .counts li {
    display: flex;
    gap: var(--space-2);
    align-items: baseline;
  }

  .sign {
    width: 1em;
    color: var(--text-muted);
    text-align: center;
  }

  .unchanged {
    margin: var(--space-2) 0 0;
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
    max-width: 420px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  td.size {
    width: 90px;
    text-align: right;
    color: var(--text-muted);
  }
</style>
