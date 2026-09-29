<script lang="ts">
  // The menu bar panel (#80): a small window under the icon while Secopy's window is hidden
  // during a job. What runs, from where to where, a bar, files, speed and time left; Pause,
  // Open Secopy and Quit. Rust sends what to show; clicking elsewhere closes it.
  import { onMount } from "svelte";
  import { tauriApi, type Api } from "../lib/api";
  import { formatDuration, formatSpeed } from "../lib/format";
  import { t } from "../lib/i18n";
  import { say } from "../lib/message";
  import type { PanelView } from "../lib/bindings";
  import Button from "../lib/ui/Button.svelte";
  import Icon from "../lib/ui/Icon.svelte";

  let { api = tauriApi }: { api?: Api } = $props();

  let view = $state<PanelView | null>(null);
  let loaded = $state(false);
  /** Pressed Pause or Resume: shown at once, until an update confirms it (one from before the
   *  press doesn't flip it back). */
  let pausedNow = $state<boolean | null>(null);
  const paused = $derived(pausedNow ?? view?.paused ?? false);

  function show(next: PanelView | null) {
    view = next;
    if (next === null || next.ended || next.paused === pausedNow) pausedNow = null;
  }

  onMount(() => {
    void api.menubarView().then((v) => {
      show(v);
      loaded = true;
    });
    const unlisten = api.onPanelView(show);
    return () => void unlisten.then((stop) => stop());
  });

  function pauseOrResume() {
    if (paused) void api.resumeJob();
    else void api.pauseJob();
    pausedNow = !paused;
  }

  const meta = $derived.by(() => {
    if (!view) return "";
    if (view.removing) return t("menubar.removing");
    const files = t("menubar.files", { done: view.filesDone, count: view.totalFiles });
    if (paused) return t("menubar.paused", { files });
    return [
      files,
      view.speed === null ? "" : formatSpeed(view.speed),
      view.leftMs === null ? "" : t("menubar.left", { time: formatDuration(view.leftMs) }),
    ]
      .filter(Boolean)
      .join(t("format.dot"));
  });
</script>

<main class="panel">
  {#if !view}
    {#if loaded}<p class="muted">{t("menubar.nothing")}</p>{/if}
  {:else if view.ended}
    <div class="ended" class:ok={view.ended.ok}>
      <Icon name={view.ended.ok ? "check" : "x"} size={20} />
      <span>{say(view.ended.text)}</span>
    </div>
    <div class="actions">
      <Button variant="link" onclick={() => api.quitApp()}>{t("menubar.quit")}</Button>
      <Button variant="primary" onclick={() => api.openMainWindow()}>{t("menubar.open")}</Button>
    </div>
  {:else}
    <div class="head">
      <span class="heading">{say(view.heading)}</span>
      <span class="percent">{view.percent === null ? t("menubar.title.starting") : t("menubar.percent", { value: view.percent })}</span>
    </div>
    <dl class="route">
      {#if view.from}<dt>{t("menubar.from")}</dt><dd class="path mono" title={say(view.from)}><bdi>{say(view.from)}</bdi></dd>{/if}
      {#if view.to}<dt>{t("menubar.to")}</dt><dd class="path mono" title={view.to}><bdi>{view.to}</bdi></dd>{/if}
    </dl>
    <div
      class="bar"
      role="progressbar"
      aria-label={say(view.heading)}
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={view.fraction === null ? undefined : Math.round(view.fraction * 100)}
    >
      <div class="fill" class:paused style="width: {(view.fraction ?? 0) * 100}%"></div>
    </div>
    <p class="meta">{meta}</p>
    <div class="actions">
      <Button variant="link" onclick={() => api.quitApp()}>{t("menubar.quitAsk")}</Button>
      <span class="right">
        <Button disabled={view.removing} onclick={pauseOrResume}>{t(paused ? "menubar.resume" : "menubar.pause")}</Button>
        <Button variant="primary" onclick={() => api.openMainWindow()}>{t("menubar.open")}</Button>
      </span>
    </div>
  {/if}
</main>

<style>
  /* The window is see-through: only the panel's rounded card shows, like a macOS popover. */
  :global(:root),
  :global(html),
  :global(body),
  :global(#app) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
  }

  .panel {
    box-sizing: border-box;
    height: 100%;
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow: hidden;
    color: var(--text);
    font-size: var(--text-md);
    user-select: none;
    -webkit-user-select: none;
    cursor: default;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--space-2);
  }

  .heading {
    font-weight: 600;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .percent {
    font-size: var(--text-lg);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .route {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--space-2);
    row-gap: 2px;
    margin: var(--space-1) 0 0;
    font-size: var(--text-sm);
  }

  dt {
    color: var(--text-faint);
  }

  /* Long paths keep their end, the part that tells them apart. */
  .path {
    margin: 0;
    color: var(--text-muted);
    direction: rtl;
    text-align: left;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .mono {
    font-family: var(--font-mono);
  }

  .bar {
    height: 6px;
    margin: var(--space-2) 0 var(--space-1);
    border-radius: var(--radius-pill);
    background: var(--surface-raised);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    transition: width var(--duration) var(--ease);
  }

  .fill.paused {
    background: var(--text-muted);
  }

  .meta {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .ended {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: 600;
    color: var(--danger);
    flex: 1;
  }

  .ended.ok {
    color: var(--success);
  }

  .actions {
    margin-top: auto;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .right {
    display: flex;
    gap: var(--space-2);
  }

  .muted {
    color: var(--text-muted);
    margin: auto;
  }
</style>
