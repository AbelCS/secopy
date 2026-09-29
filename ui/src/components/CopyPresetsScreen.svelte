<script lang="ts">
  // Copy presets (FR-38): the list on the left, the selected one's editor on the right.
  import { t } from "../lib/i18n";
  import { messageOf } from "../lib/format";
  import { useApi } from "../lib/api";
  import type { CopyPreset, CopyPresetInput, SessionView } from "../lib/bindings";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import EmptyState from "../lib/ui/EmptyState.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import CopyPresetEditor from "./CopyPresetEditor.svelte";

  let {
    presets,
    onPresets,
    onView,
    onDone,
  }: {
    presets: CopyPreset[];
    onPresets: (presets: CopyPreset[]) => void;
    /** Editing or deleting the selected preset changes what FROM shows. */
    onView: (view: SessionView) => void;
    onDone: () => void;
  } = $props();

  const api = useApi();
  const NEW = "new";
  // svelte-ignore state_referenced_locally
  let selectedId: string | null = $state(presets[0]?.id ?? null);
  let error: string | null = $state(null);
  /** A short confirmation, "Exported “Sony FX3”." */
  let said: string | null = $state(null);
  /** The editor has unsaved changes. */
  let changed = $state(false);
  let canSave = $state(false);
  let editor: ReturnType<typeof CopyPresetEditor> | undefined = $state();
  const FORM = "copy-preset-editor";

  const selected = $derived(presets.find((p) => p.id === selectedId) ?? null);
  /** Recreates the editor for another preset, or after this one was saved. */
  const editorKey = $derived(selectedId === NEW ? NEW : JSON.stringify(selected));

  /** "Discard changes?" is open: another Esc or click doesn't ask a second time. */
  let asking = false;

  /** Whether it's fine to leave the preset being edited; asks when it has changes. */
  export async function mayLeave(): Promise<boolean> {
    if (!changed) return true;
    if (asking) return false;
    const message =
      selectedId === NEW ? t("presets.discardNew") : t("ui.discard.named", { name: selected?.name ?? "" });
    asking = true;
    try {
      return await api.confirm(message, t("ui.discard.title"), t("ui.discard.discard"), t("ui.discard.keep"));
    } finally {
      asking = false;
    }
  }

  async function select(id: string) {
    if (id === selectedId || !(await mayLeave())) return;
    changed = false;
    said = null;
    selectedId = id;
  }

  /** Export… (#77): the saved preset, after asking about unsaved changes. */
  async function exportSelected() {
    const preset = selected;
    if (!preset || !(await mayLeave())) return;
    if (changed) {
      editor?.revert();
      changed = false;
    }
    const path = await api.pickExportPath(`${preset.name.replace(/[/:]/g, "-")}.secopy`);
    if (!path) return;
    try {
      said = await api.exportCopyPreset(preset.id, path);
      error = null;
    } catch (e) {
      said = null;
      error = messageOf(e);
    }
  }

  async function back() {
    if (await mayLeave()) onDone();
  }

  async function save(input: CopyPresetInput) {
    if (selectedId === NEW) {
      const list = await api.createCopyPreset(input);
      onPresets(list);
      const name = input.name.trim().toLowerCase();
      selectedId = list.find((p) => p.name.toLowerCase() === name)?.id ?? null;
    } else if (selected) {
      const result = await api.editCopyPreset(selected.id, input);
      onPresets(result.presets);
      onView(result.session);
    }
  }

  async function remove(p: CopyPreset) {
    const sure = await api.confirm(
      t("presets.delete.message", { name: p.name }),
      t("presets.delete.title"),
      t("presets.delete.ok"),
      t("presets.delete.keep"),
    );
    if (!sure) return;
    try {
      const result = await api.deleteCopyPreset(p.id);
      onPresets(result.presets);
      onView(result.session);
      selectedId = result.presets[0]?.id ?? null;
      error = null;
    } catch (e) {
      error = messageOf(e);
    }
  }
</script>

<!-- A held Esc repeats: only the first press counts. -->
<svelte:window onkeydown={(e) => e.key === "Escape" && !e.repeat && void back()} />

<AppShell>
  {#snippet header()}<ScreenHeader title={t("presets.title")} />{/snippet}

  <div class="panes">
    <Section title={t("presets.all")}>
      <nav class="list" aria-label={t("presets.title")}>
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
            <!-- The left-to-right mark keeps the slashes in place inside the right-aligned cut. -->
            <span class="muted mono path" title={p.source}>{p.source ? `\u200E${p.source}` : t("presets.noSource")}</span>
          </button>
        {/each}
        <Button variant="link" onclick={() => select(NEW)}>{t("presets.addNew")}</Button>
      </nav>
    </Section>

    <Section title={selectedId === NEW ? t("presets.newTitle") : (selected?.name ?? t("presets.about"))}>
      {#if selectedId === NEW || selected}
        {#key editorKey}
          <CopyPresetEditor
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
          <p>{t("presets.empty.what")}</p>
          <p>{t("presets.empty.how")}</p>
        </EmptyState>
      {/if}
      {#if said}<Notice tone="success">{said}</Notice>{/if}
      {#if error}<Notice tone="danger">{error}</Notice>{/if}
    </Section>
  </div>

  {#snippet actions()}
    <ActionBar status={changed ? t("presets.unsaved") : ""}>
      {#snippet start()}
        <Button icon="chevron-left" onclick={back}>{t("ui.back")}</Button>
        {#if selected && selectedId !== NEW}
          <Button onclick={exportSelected}>{t("ui.export")}</Button>
          <Button variant="danger" onclick={() => remove(selected)}>{t("ui.delete")}</Button>
        {/if}
      {/snippet}
      {#snippet end()}
        {#if selectedId === NEW || selected}
          <Button disabled={!changed} onclick={() => editor?.revert()}>{t("ui.revert")}</Button>
          <Button variant="primary" type="submit" form={FORM} disabled={!canSave}>{t("ui.save")}</Button>
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

  .list :global(.link) {
    align-self: flex-start;
    margin-top: var(--space-2);
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

  .item.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .muted {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }

  /* A long source keeps its end (the directory that matters) visible. */
  .path {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
</style>
