<script lang="ts">
  // Mirror presets (RFD §5.8, FR-44): the list on the left, the selected one on the right.
  // A saved preset is previewed (then run) or added to the queue; an edited one is saved first.
  import type { Snippet } from "svelte";
  import { useApi } from "../lib/api";
  import type { MirrorPreset, MirrorPresetInput, MirrorPreviewView, QueueView } from "../lib/bindings";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import { formatCount, messageOf, plural } from "../lib/format";
  import Button from "../lib/ui/Button.svelte";
  import EmptyState from "../lib/ui/EmptyState.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import MirrorEditor from "./MirrorEditor.svelte";

  let {
    presets,
    onPresets,
    onPreview,
    onQueue,
    banner,
  }: {
    presets: MirrorPreset[];
    onPresets: (presets: MirrorPreset[]) => void;
    /** Preview… worked out what the preset would do. */
    onPreview: (view: MirrorPreviewView) => void;
    onQueue: (queue: QueueView) => void;
    /** App-wide messages, shown first. */
    banner?: Snippet;
  } = $props();

  const api = useApi();
  const NEW = "new";
  const FORM = "mirror-editor";
  // svelte-ignore state_referenced_locally
  let selectedId: string | null = $state(presets[0]?.id ?? null);
  let error: string | null = $state(null);
  /** A short confirmation, "Exported “Footage”." */
  let said: string | null = $state(null);
  let busy = $state(false);
  /** Preview… is working; with the deep check, how far it is. */
  let previewing = $state(false);
  let compared: { done: number; total: number } | null = $state(null);
  /** The editor has unsaved changes. */
  let changed = $state(false);
  let canSave = $state(false);
  let editor: ReturnType<typeof MirrorEditor> | undefined = $state();

  const selected = $derived(presets.find((p) => p.id === selectedId) ?? null);
  /** Recreates the editor for another preset, or after this one was saved. */
  const editorKey = $derived(selectedId === NEW ? NEW : JSON.stringify(selected));
  const editing = $derived(selectedId === NEW || changed);

  /** "Discard changes?" is open: another click doesn't ask a second time. */
  let asking = false;

  async function select(id: string) {
    if (id === selectedId || asking) return;
    if (changed) {
      const which = selectedId === NEW ? "the new mirror" : `“${selected?.name ?? ""}”`;
      asking = true;
      let discard: boolean;
      try {
        discard = await api.confirm(`Your changes to ${which} aren't saved.`, "Discard changes?", "Discard", "Keep editing");
      } finally {
        asking = false;
      }
      if (!discard) return;
    }
    changed = false;
    said = null;
    selectedId = id;
  }

  async function save(input: MirrorPresetInput) {
    if (selectedId === NEW) {
      const list = await api.createMirrorPreset(input);
      onPresets(list);
      const name = input.name.trim().toLowerCase();
      selectedId = list.find((p) => p.name.toLowerCase() === name)?.id ?? null;
    } else if (selected) {
      onPresets(await api.editMirrorPreset(selected.id, input));
    }
  }

  async function act(action: () => Promise<void>) {
    busy = true;
    try {
      await action();
      error = null;
    } catch (e) {
      error = messageOf(e);
    } finally {
      busy = false;
    }
  }

  async function preview(p: MirrorPreset) {
    previewing = true;
    compared = null;
    try {
      onPreview(await api.previewMirror(p.id, (c) => (compared = c)));
      error = null;
    } catch (e) {
      const message = messageOf(e);
      error = message === "Cancelled." ? null : message;
    } finally {
      previewing = false;
      compared = null;
    }
  }

  /** Export… (#77): the saved mirror, after asking about unsaved changes. */
  async function exportSelected() {
    const preset = selected;
    if (!preset || asking) return;
    if (changed) {
      asking = true;
      let discard: boolean;
      try {
        discard = await api.confirm(`Your changes to “${preset.name}” aren't saved.`, "Discard changes?", "Discard", "Keep editing");
      } finally {
        asking = false;
      }
      if (!discard) return;
      editor?.revert();
      changed = false;
    }
    const path = await api.pickExportPath(`${preset.name.replace(/[/:]/g, "-")}.secopy`);
    if (!path) return;
    try {
      said = await api.exportMirrorPreset(preset.id, path);
      error = null;
    } catch (e) {
      said = null;
      error = messageOf(e);
    }
  }

  async function remove(p: MirrorPreset) {
    const sure = await api.confirm(
      `The mirror “${p.name}” is deleted. Its origin, destination and archive are not touched.`,
      "Delete mirror?",
      "Delete",
      "Keep",
    );
    if (!sure) return;
    await act(async () => {
      const list = await api.deleteMirrorPreset(p.id);
      onPresets(list);
      selectedId = list[0]?.id ?? null;
    });
  }
</script>

<AppShell>
  {#snippet header()}
    <ScreenHeader title="Mirror" />
  {/snippet}

  {@render banner?.()}
  <div class="panes">
    <Section title="All mirrors">
      <nav class="list" aria-label="Mirrors">
        {#each presets as p (p.id)}
          <button
            type="button"
            class="item"
            class:on={p.id === selectedId}
            aria-label={p.name}
            aria-current={p.id === selectedId}
            onclick={() => select(p.id)}
          >
            <span>{p.name}</span>
            <span class="muted mono path" title={p.destination}><bdi>{p.destination}</bdi></span>
          </button>
        {/each}
        <Button variant="link" onclick={() => select(NEW)}>+ New mirror</Button>
      </nav>
    </Section>

    <Section title={selectedId === NEW ? "New mirror" : (selected?.name ?? "About mirrors")}>
      {#if selectedId === NEW || selected}
        {#key editorKey}
          <MirrorEditor
            bind:this={editor}
            bind:changed
            bind:canSave
            formId={FORM}
            preset={selectedId === NEW ? null : selected}
            onSave={save}
          />
        {/key}
      {:else}
        <EmptyState>
          <p>
            A mirror keeps a copy of a directory identical to it: new and changed files are copied and verified,
            and files deleted in the origin are archived (or deleted) in the destination. Nothing is ever written
            to the origin, and every run shows a preview first.
          </p>
          <p>Create one with “+ New mirror”.</p>
        </EmptyState>
      {/if}
      {#if said}<Notice tone="success">{said}</Notice>{/if}
      {#if error}<Notice tone="danger">{error}</Notice>{/if}
    </Section>
  </div>

  {#snippet actions()}
    <ActionBar
      status={changed
        ? "Unsaved changes"
        : compared
          ? `Comparing contents: ${formatCount(compared.done)} of ${plural(compared.total, "file")}`
          : previewing
            ? "Working out the preview…"
            : ""}
    >
      {#snippet start()}
        {#if selected && selectedId !== NEW}
          <Button onclick={exportSelected}>Export…</Button>
          <Button variant="danger" onclick={() => remove(selected)}>Delete…</Button>
        {/if}
      {/snippet}
      {#snippet end()}
        {#if editing}
          <Button disabled={!changed} onclick={() => editor?.revert()}>Revert</Button>
          <Button variant="primary" type="submit" form={FORM} disabled={!canSave}>Save</Button>
        {:else if selected}
          {#if previewing}
            <Button onclick={() => api.cancelMirrorPreview()}>Cancel</Button>
          {:else}
            <!-- A queued mirror is only its preset: it's previewed again at its turn. -->
            <Button
              disabled={busy}
              help="Adds this mirror to the Queue; what to copy and remove is worked out again when it runs."
              onclick={() => act(async () => onQueue(await api.addMirrorToQueue(selected.id)))}
            >
              Add to queue
            </Button>
          {/if}
          <Button variant="primary" disabled={busy || previewing} onclick={() => preview(selected)}>Preview…</Button>
        {/if}
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .panes {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: var(--space-3);
    align-items: start;
  }

  .list {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-1);
  }

  .item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    text-align: left;
    font: inherit;
    color: var(--text);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    padding: var(--space-2);
    cursor: pointer;
  }

  .item:hover {
    background: var(--surface-raised);
  }

  .item.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .muted {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }

  .path {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
</style>
