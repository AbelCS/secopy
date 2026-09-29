<script lang="ts">
  // What a mirror would do (FR-47): the counts, every file by kind, and what looks wrong
  // (FR-50). Start runs exactly this.
  import { t } from "../lib/i18n";
  import { useApi } from "../lib/api";
  import type { MirrorPreviewView, PreviewKind, PreviewRow, QueueView } from "../lib/bindings";
  import { formatBytes, messageOf } from "../lib/format";
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

  const inSync = $derived(
    preview.newFiles + preview.changedFiles + preview.removedFiles + preview.failing === 0,
  );
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
      .catch((e) => (error = messageOf(e)));
  });

  /** Start's help: what it copies, then what it does with files gone from the origin (FR-48, FR-49). */
  const startHelp = $derived.by(() => {
    const { newFiles, changedFiles, removedFiles } = preview;
    const copied =
      newFiles > 0 && changedFiles > 0
        ? t("mirror.preview.copied.both", { new: newFiles, changed: changedFiles, count: newFiles + changedFiles })
        : newFiles > 0
          ? t("mirror.preview.copied.new", { count: newFiles })
          : t("mirror.preview.copied.changed", { count: changedFiles });
    const removed = t("mirror.files", { count: removedFiles });
    const archive = preview.archiveDays !== null;
    if (newFiles + changedFiles === 0) {
      if (removedFiles === 0) return "";
      return t(archive ? "mirror.preview.startHelp.archive" : "mirror.preview.startHelp.delete", { removed });
    }
    if (removedFiles === 0) return t("mirror.preview.startHelp.copy", { copied });
    return t(archive ? "mirror.preview.startHelp.copyArchive" : "mirror.preview.startHelp.copyDelete", { copied, removed });
  });

  async function run() {
    if (preview.guard) {
      const sure = await api.confirm(preview.guard, t("mirror.preview.guard.title"), t("mirror.preview.guard.run"), t("mirror.preview.guard.cancel"));
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
      error = messageOf(e);
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

  <Section title={t("mirror.preview.changes")}>
    {#if inSync}
      <p>{t("mirror.preview.inSync")}</p>
    {:else}
      <ul class="counts">
        <li><span class="sign" aria-hidden="true">+</span><span>{t("mirror.preview.new", { count: preview.newFiles })}</span>
          <span class="muted">{formatBytes(preview.newBytes)}</span></li>
        <li><span class="sign" aria-hidden="true">↻</span><span>{t("mirror.preview.changed", { count: preview.changedFiles })}</span>
          <span class="muted">{formatBytes(preview.changedBytes)}</span></li>
        <li><span class="sign" aria-hidden="true">−</span><span>
          {t("mirror.preview.removed", { count: preview.removedFiles })}
          {#if preview.archiveDays === null}{t("mirror.preview.deleted")}{:else}<Hint
              text={t("mirror.preview.archivedHelp", { days: preview.archiveDays })}
              >{t("mirror.preview.archived", { days: preview.archiveDays })}</Hint
            >{/if}
        </span></li>
        {#if preview.failing > 0}
          <li class="failing"><span class="sign" aria-hidden="true">✗</span><span>
            {t("mirror.preview.failing", { count: preview.failing })}
          </span></li>
        {/if}
      </ul>
    {/if}
    <p class="muted unchanged">{t("mirror.preview.unchanged", { count: preview.unchanged })}</p>
  </Section>

  {#if !inSync}
    <Section title={t("mirror.preview.files")}>
      {#snippet aside()}
        <SegmentedControl
          label={t("mirror.preview.show")}
          options={[
            { value: "all" as Shown, label: t("mirror.preview.shown.all") },
            { value: "new" as Shown, label: t("mirror.preview.shown.new") },
            { value: "changed" as Shown, label: t("mirror.preview.shown.changed") },
            { value: "removed" as Shown, label: t("mirror.preview.shown.removed") },
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
        <p class="muted">{t("mirror.preview.more", { count: total - rows.length })}</p>
      {/if}
    </Section>
  {/if}

  {#snippet actions()}
    <ActionBar>
      {#snippet start()}<Button onclick={onCancel}>{t("ui.cancel")}</Button>{/snippet}
      {#snippet end()}
        <!-- A queued mirror is only its preset: it's previewed again at its turn. -->
        <Button disabled={busy} help={t("mirror.addToQueueHelp")} onclick={queue}>{t("mirror.addToQueue")}</Button>
        <Button variant="primary" disabled={inSync} help={startHelp} onclick={run}>{t("mirror.preview.start")}</Button>
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

  .failing {
    color: var(--danger);
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
