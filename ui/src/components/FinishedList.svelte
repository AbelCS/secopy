<script lang="ts">
  // Finished files (RFD §5.3). Virtualized: only the visible rows exist, and rows are fetched
  // from the app a page at a time, so it stays smooth with a million files.
  import { useApi } from "../lib/api";
  import type { FinishedRow } from "../lib/bindings";
  import { formatBytes, formatDuration, formatSpeed } from "../lib/format";
  import Section from "../lib/ui/Section.svelte";

  let {
    title = "Finished",
    total,
    failedTotal,
    updated,
    fetchPage,
  }: {
    title?: string;
    /** Finished files so far. */
    total: number;
    /** Failed files so far. */
    failedTotal: number;
    /** Changes with every progress update. */
    updated: number;
    /** Where the rows come from: the current job by default, or a job of the queue run. */
    fetchPage?: (offset: number, limit: number, failedOnly: boolean) => Promise<FinishedRow[]>;
  } = $props();

  const api = useApi();
  const ROW = 28;
  /** The most the list shows before it scrolls; a shorter list is only as tall as its rows. */
  const HEIGHT = 280;
  const PAGE = 100;
  const OVERSCAN = 5;

  let failedOnly = $state(false);
  let scrollTop = $state(0);
  let viewport: HTMLElement;
  /** Fetched pages by page number; a page is refetched until it is full. */
  let pages: Map<number, FinishedRow[]> = $state(new Map());
  /**
   * The update each page was last asked for in. A page can come back short for a moment
   * (a file counted as finished just before it's listed); it's asked again on the next
   * update, not in a loop.
   */
  const asked = new Map<number, number>();

  const count = $derived(failedOnly ? failedTotal : total);
  const height = $derived(Math.min(HEIGHT, Math.max(1, count) * ROW));
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const last = $derived(Math.min(count, Math.ceil((scrollTop + HEIGHT) / ROW) + OVERSCAN));
  const visible = $derived(
    Array.from({ length: Math.max(0, last - first) }, (_, i) => {
      const index = first + i;
      return { index, row: pages.get(Math.floor(index / PAGE))?.[index % PAGE] };
    }),
  );

  $effect(() => {
    // Fetch the pages the visible rows need; incomplete pages again as more files finish.
    const only = failedOnly;
    const now = updated;
    const wanted = new Set(visible.map((v) => Math.floor(v.index / PAGE)));
    for (const page of wanted) {
      const have = pages.get(page);
      const expected = Math.min(PAGE, count - page * PAGE);
      if ((have && have.length >= expected) || asked.get(page) === now) continue;
      asked.set(page, now);
      (fetchPage ?? api.finishedPage)(page * PAGE, PAGE, only).then((rows) => {
        if (only !== failedOnly) return;
        pages = new Map(pages).set(page, rows);
      });
    }
  });

  function showFailedOnly(on: boolean) {
    failedOnly = on;
    pages = new Map();
    asked.clear();
    scrollTop = 0;
    viewport.scrollTop = 0;
  }

  const statusText = (r: FinishedRow) =>
    ({ verified: "✓ Verified", copied: "✓ Copied", skipped: "Skipped", failed: "✗ Failed", cancelled: "Cancelled" })[
      r.status
    ];
</script>

<Section {title}>
  {#snippet aside()}
    <label class="only">
      <input
        type="checkbox"
        checked={failedOnly}
        onchange={(e) => showFailedOnly(e.currentTarget.checked)}
      />
      Failed only
    </label>
  {/snippet}
<!-- Column names for the eye; each row's cells are read in order. -->
<div class="row head" aria-hidden="true">
  <span>File</span><span>Size</span><span>Time</span><span>Speed</span><span>Checksum</span><span>Status</span>
</div>
<!-- Focusable so the arrow keys scroll it (a scrollable region, WCAG 2.1.1). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={viewport}
  class="viewport"
  style:height="{height}px"
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  role="list"
  aria-label="Finished files"
  tabindex="0"
>
  <div style:height="{count * ROW}px" class="spacer">
    {#each visible as { index, row } (index)}
      <div class="row" role="listitem" style:top="{index * ROW}px">
        {#if row}
          <span class="name" title={row.finalPath}>{row.finalPath}</span>
          <span>{formatBytes(row.size)}</span>
          <span>{formatDuration(row.millis)}</span>
          <span>{formatSpeed(row.millis > 0 ? (row.size * 1000) / row.millis : null)}</span>
          <span class="mono">{row.hash ?? "—"}</span>
          <!-- A short word; why is on hover, and in the summary's Failed list. -->
          <span class={row.status} title={row.reason ?? ""}>{statusText(row)}</span>
        {:else}
          <span class="muted">…</span>
        {/if}
      </div>
    {/each}
  </div>
</div>
</Section>

<style>
  .only {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .only input {
    margin: 0;
    accent-color: var(--accent);
  }

  .viewport {
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 28px;
    display: grid;
    /* The name gets whatever is left; its full path is in the tooltip. */
    grid-template-columns: minmax(0, 1fr) 70px 44px 80px 136px 96px;
    gap: 8px;
    align-items: center;
    padding: 0 10px;
    font-size: 12px;
    white-space: nowrap;
  }

  .row span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row.head {
    position: static;
    height: auto;
    padding-bottom: var(--space-1);
    font-size: var(--text-xs);
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .verified,
  .copied {
    color: var(--success);
  }

  .failed {
    color: var(--danger);
  }

  .muted,
  .skipped,
  .cancelled {
    color: var(--text-muted);
  }
</style>
