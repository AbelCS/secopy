<script lang="ts">
  // Finished files (RFD §5.3). Virtualized: only the visible rows exist, and rows are fetched
  // from the app a page at a time, so it stays smooth with a million files.
  import { useApi } from "../lib/api";
  import type { FinishedRow } from "../lib/bindings";
  import { formatBytes, formatDuration, formatSpeed } from "../lib/format";

  let {
    total,
    failedTotal,
    updated,
  }: {
    /** Finished files so far. */
    total: number;
    /** Failed files so far. */
    failedTotal: number;
    /** Changes with every progress update. */
    updated: number;
  } = $props();

  const api = useApi();
  const ROW = 28;
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
      api.finishedPage(page * PAGE, PAGE, only).then((rows) => {
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
    ({ verified: "✓ Verified", copied: "✓ Copied", skipped: "Skipped", failed: "✗ Failed" })[r.status];
</script>

<div class="head">
  <h3>Finished</h3>
  <label>
    <input
      type="checkbox"
      checked={failedOnly}
      onchange={(e) => showFailedOnly(e.currentTarget.checked)}
    />
    Failed only
  </label>
</div>
<div
  bind:this={viewport}
  class="viewport"
  style:height="{HEIGHT}px"
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  role="list"
  aria-label="Finished files"
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
          <span class={row.status} title={row.reason ?? ""}>
            {statusText(row)}{row.reason ? ` — ${row.reason}` : ""}
          </span>
        {:else}
          <span class="muted">…</span>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  h3 {
    margin: 12px 0 6px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .viewport {
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
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
    grid-template-columns: 1fr 80px 60px 90px 150px 120px;
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

  .verified,
  .copied {
    color: var(--success);
  }

  .failed {
    color: var(--danger);
  }

  .muted,
  .skipped {
    color: var(--text-muted);
  }
</style>
