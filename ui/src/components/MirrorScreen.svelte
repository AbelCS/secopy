<script lang="ts">
  // Mirror presets (RFD §5.8, FR-44): the list on the left, the selected one on the right.
  // A saved preset is previewed (then run) or added to the queue; an edited one is saved first.
  import { locale, t } from "../lib/i18n";
  import { ARCHIVE_DIR } from "../lib/engine";
  import { AppError, say } from "../lib/message";
  import { onMount, type Snippet } from "svelte";
  import { useApi } from "../lib/api";
  import type { ArchiveView, MirrorPreset, MirrorPresetInput, MirrorPreviewView, QueueView } from "../lib/bindings";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import { formatBytes, messageOf } from "../lib/format";
  import Button from "../lib/ui/Button.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";
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

  /** "Discard changes?" for the mirror being edited. */
  function askDiscard(): Promise<boolean> {
    const message =
      selectedId === NEW ? t("mirror.discardNew") : t("ui.discard.named", { name: selected?.name ?? "" });
    return api.confirm(message, t("ui.discard.title"), t("ui.discard.discard"), t("ui.discard.keep"));
  }

  /** Whether it's fine to leave: asks when the mirror being edited has unsaved changes. */
  export async function mayLeave(): Promise<boolean> {
    if (!changed) return true;
    if (asking) return false;
    asking = true;
    try {
      return await askDiscard();
    } finally {
      asking = false;
    }
  }

  async function select(id: string) {
    if (id === selectedId || asking) return;
    if (changed) {
      asking = true;
      let discard: boolean;
      try {
        discard = await askDiscard();
      } finally {
        asking = false;
      }
      if (!discard) return;
    }
    changed = false;
    // What was said about the mirror left (an error too) isn't about this one (#199).
    said = null;
    error = null;
    selectedId = id;
  }

  /** The selected mirror's archive (#99); `null` while it's being looked at. */
  let archive: ArchiveView | null = $state(null);
  const savedId = $derived(selectedId !== NEW && selected ? selected.id : null);

  /** The destination saved for the selected mirror: its archive is the one shown (#113). */
  const savedDestination = $derived(savedId ? selected!.destination : null);

  /** Counts the looks: only the latest one's answer is shown (#113). */
  let looks = 0;

  async function lookAtArchive(id: string) {
    const look = ++looks;
    const seen = await api.mirrorArchive(id).catch(() => null);
    if (look === looks && savedId === id && (!seen || seen.destination === savedDestination)) archive = seen;
  }

  $effect(() => {
    archive = null;
    if (savedId && savedDestination) void lookAtArchive(savedId);
  });

  /** Looked at again whenever the destination may have come back (#195): Choose… (the same
   *  directory too), Save, the window back in front (a disk plugged in, a directory made). */
  function lookAgain() {
    if (savedId && savedDestination) void lookAtArchive(savedId);
  }

  onMount(() => {
    window.addEventListener("focus", lookAgain);
    return () => window.removeEventListener("focus", lookAgain);
  });

  /** "Delete it at the next run" is pending for the destination shown (#101). */
  const pendingDeletion = $derived(!!selected && selected.clearArchive === savedDestination);

  const DATE = { day: "numeric", month: "short", year: "numeric" } as const;
  const archiveText = $derived.by(() => {
    if (!archive) return "";
    if (!archive.connected) return t("mirror.archive.notFound");
    if (archive.files === 0) return t("mirror.archive.empty");
    return [
      t("mirror.archive.files", { count: archive.files }),
      formatBytes(archive.bytes),
      archive.oldest
        ? t("mirror.archive.oldest", { date: new Intl.DateTimeFormat(locale(), DATE).format(new Date(archive.oldest)) })
        : "",
    ]
      .filter(Boolean)
      .join(t("format.dot"));
  });

  async function deleteArchive(p: MirrorPreset, held: ArchiveView) {
    const sure = await api.confirm(
      t("mirror.archive.confirm", { count: held.files, size: formatBytes(held.bytes) }),
      t("mirror.archive.confirmTitle"),
      t("mirror.delete.ok"),
      t("mirror.delete.keep"),
    );
    if (!sure) return;
    try {
      const deleted = await api.deleteMirrorArchive(p.id, held.destination);
      said = t("mirror.archive.deleted", { count: deleted.removed });
      error = deleted.notDeleted ? say(deleted.notDeleted) : null;
    } catch (e) {
      said = null;
      error = messageOf(e);
    }
    await lookAtArchive(p.id);
  }

  /** Switching to Delete: what to do with what's already archived (#101). */
  type ArchiveChoice = "now" | "nextRun" | "keep" | "cancel";
  let asked: { archive: ArchiveView; days: number; answer: (c: ArchiveChoice) => void } | null = $state(null);

  function askAboutArchive(archive: ArchiveView, days: number): Promise<ArchiveChoice> {
    return new Promise((answer) => (asked = { archive, days, answer }));
  }

  function answer(choice: ArchiveChoice) {
    asked?.answer(choice);
    asked = null;
  }

  async function save(input: MirrorPresetInput) {
    if (selectedId === NEW) {
      const list = await api.createMirrorPreset(input);
      onPresets(list);
      const name = input.name.trim().toLowerCase();
      selectedId = list.find((p) => p.name.toLowerCase() === name)?.id ?? null;
    } else if (selected) {
      const preset = selected;
      let choice: ArchiveChoice = "keep";
      // Asked only for the archive of the destination kept: a new destination leaves the old
      // one's archive alone, as it's no longer this mirror's.
      const sameDestination = input.destination.trimStart().replace(/\/+$/, "") === preset.destination;
      if (preset.deleted.mode === "archive" && input.deleted.mode === "delete" && sameDestination) {
        const archive = await api.mirrorArchive(preset.id);
        if (archive.files > 0 || !archive.connected) choice = await askAboutArchive(archive, input.deleted.days);
      }
      if (choice === "cancel") return;
      // Saved first: an edit that can't be saved deletes nothing (#113). The destination is
      // the same, so its archive is still the one asked about.
      onPresets(await api.editMirrorPreset(preset.id, input));
      // Saved: the editor is made again from the saved preset, so what goes wrong from here is
      // said on this screen, with the save done (#192).
      try {
        const deleted = choice === "now" ? await api.deleteMirrorArchive(preset.id, preset.destination) : null;
        if (choice === "nextRun") onPresets(await api.clearMirrorArchiveNextRun(preset.id));
        if (deleted) {
          said = t("mirror.archive.deleted", { count: deleted.removed });
          error = deleted.notDeleted ? say(deleted.notDeleted) : null;
        }
        lookAgain();
      } catch (e) {
        const key = choice === "now" ? "mirror.archive.savedNotDeleted" : "mirror.archive.savedNotPending";
        error = t(key, { why: messageOf(e) });
      }
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
      // Cancel stops the preview: nothing went wrong.
      error = e instanceof AppError && e.m.key === "errors.mirror.previewCancelled" ? null : messageOf(e);
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
        discard = await askDiscard();
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
      t("mirror.delete.message", { name: p.name }),
      t("mirror.delete.title"),
      t("mirror.delete.ok"),
      t("mirror.delete.keep"),
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
    <ScreenHeader title={t("mirror.title")} />
  {/snippet}

  {@render banner?.()}
  <div class="panes">
    <Section title={t("mirror.all")}>
      <nav class="list" aria-label={t("mirror.list")}>
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
        <Button variant="link" onclick={() => select(NEW)}>{t("mirror.addNew")}</Button>
      </nav>
    </Section>

    <div class="stack">
    <Section title={selectedId === NEW ? t("mirror.newTitle") : (selected?.name ?? t("mirror.about"))}>
      {#if selectedId === NEW || selected}
        {#key editorKey}
          <MirrorEditor
            bind:this={editor}
            bind:changed
            bind:canSave
            formId={FORM}
            preset={selectedId === NEW ? null : selected}
            onSave={save}
            onDestinationChosen={lookAgain}
          />
        {/key}
      {:else}
        <EmptyState>
          <p>{t("mirror.empty.what")}</p>
          <p>{t("mirror.empty.how")}</p>
        </EmptyState>
      {/if}
      {#if said}<Notice tone="success">{said}</Notice>{/if}
      {#if error}<Notice tone="danger">{error}</Notice>{/if}
    </Section>
    {#if savedId && selected}
      {@const p = selected}
      <Section title={t("mirror.archive.title")}>
        <div class="archive">
          <span class="muted archive-text">
            <span>{archiveText}</span>
            {#if pendingDeletion}<span>{t("mirror.archive.pending")}</span>{/if}
            {#if archive?.busy && archive.files > 0}<span>{t("mirror.archive.busy")}</span>{/if}
          </span>
          <span class="archive-actions">
            <Button
              disabled={!archive?.connected || archive.files === 0}
              onclick={() => api.reveal(`${p.destination}/${ARCHIVE_DIR}`)}>{t("mirror.archive.show")}</Button
            >
            <Button
              variant="danger"
              disabled={!archive?.connected || archive.files === 0 || archive.busy}
              onclick={() => archive && deleteArchive(p, archive)}>{t("mirror.archive.delete")}</Button
            >
          </span>
        </div>
      </Section>
    {/if}
    </div>
  </div>

  {#snippet actions()}
    <ActionBar
      status={changed
        ? t("mirror.unsaved")
        : compared
          ? t("mirror.comparing", { done: compared.done, files: t("mirror.files", { count: compared.total }) })
          : previewing
            ? t("mirror.previewing")
            : ""}
    >
      {#snippet start()}
        {#if selected && selectedId !== NEW}
          <Button onclick={exportSelected}>{t("ui.export")}</Button>
          <Button variant="danger" onclick={() => remove(selected)}>{t("ui.delete")}</Button>
        {/if}
      {/snippet}
      {#snippet end()}
        {#if editing}
          <Button disabled={!changed} onclick={() => editor?.revert()}>{t("ui.revert")}</Button>
          <Button variant="primary" type="submit" form={FORM} disabled={!canSave}>{t("ui.save")}</Button>
        {:else if selected}
          {#if previewing}
            <Button onclick={() => api.cancelMirrorPreview()}>{t("ui.cancel")}</Button>
          {:else}
            <!-- A queued mirror is only its preset: it's previewed again at its turn. -->
            <Button
              disabled={busy}
              help={t("mirror.addToQueueHelp")}
              onclick={() => act(async () => onQueue(await api.addMirrorToQueue(selected.id)))}
            >
              {t("mirror.addToQueue")}
            </Button>
          {/if}
          <Button variant="primary" disabled={busy || previewing} onclick={() => preview(selected)}>{t("mirror.preview.open")}</Button>
        {/if}
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

{#if asked}
  {@const q = asked}
  <Dialog title={t("mirror.switch.title")} onClose={() => answer("cancel")}>
    <p>
      {!q.archive.connected
        ? t("mirror.switch.unavailable")
        : q.archive.busy
          ? t("mirror.switch.busy")
          : t("mirror.switch.files", { count: q.archive.files, size: formatBytes(q.archive.bytes) })}
    </p>
    {#snippet actions()}
      <Button data-autofocus onclick={() => answer("cancel")}>{t("ui.cancel")}</Button>
      {#if q.archive.connected && !q.archive.busy}
        <Button onclick={() => answer("keep")}>{t("mirror.switch.keep", { count: q.days })}</Button>
        <Button variant="danger" onclick={() => answer("now")}>{t("mirror.switch.deleteNow")}</Button>
      {:else}
        <Button onclick={() => answer("keep")}>{t("mirror.switch.keepArchived", { count: q.days })}</Button>
        <Button variant="danger" onclick={() => answer("nextRun")}>{t("mirror.switch.deleteNextRun")}</Button>
      {/if}
    {/snippet}
  </Dialog>
{/if}

<style>
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }

  .archive {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .archive-text {
    display: flex;
    flex-direction: column;
  }

  .archive-actions {
    display: flex;
    gap: var(--space-2);
  }

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
