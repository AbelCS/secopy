<script lang="ts">
  // The app's sections as tabs at the top (Copy, Mirror, Queue), and on the right what
  // applies to the whole app (Settings). The chosen tab is filled; hidden while jobs run.
  import type { Snippet } from "svelte";

  let {
    items,
    selected,
    onSelect,
    trailing,
  }: {
    items: { id: string; label: string; count?: number }[];
    selected: string;
    onSelect: (id: string) => void;
    /** On the right: Settings. */
    trailing?: Snippet;
  } = $props();
</script>

<div class="tabbar">
  <nav class="tabs" aria-label="Sections">
    {#each items as item (item.id)}
      <button
        type="button"
        class="tab"
        class:on={item.id === selected}
        aria-current={item.id === selected ? "page" : undefined}
        aria-label={item.count ? `${item.label}, ${item.count} ${item.count === 1 ? "job" : "jobs"}` : item.label}
        onclick={() => onSelect(item.id)}
      >
        {item.label}
        {#if item.count}<span class="count" aria-hidden="true">{item.count}</span>{/if}
      </button>
    {/each}
  </nav>
  {#if trailing}<div class="trailing">{@render trailing()}</div>{/if}
</div>

<style>
  .tabbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-5);
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .tabs {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    overflow: hidden;
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-height);
    padding: 0 var(--space-4);
    font: inherit;
    color: var(--text-muted);
    background: none;
    border: 0;
    cursor: pointer;
    transition: background var(--duration) var(--ease);
  }

  .tab + .tab {
    border-left: 1px solid var(--border);
  }

  .tab:hover {
    color: var(--text);
    background: var(--surface-raised);
  }

  .tab.on {
    color: var(--text);
    background: var(--accent-soft);
  }

  .tab:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .count {
    min-width: 20px;
    padding: 0 var(--space-1);
    border-radius: var(--radius-pill);
    background: var(--surface-raised);
    color: var(--text-muted);
    font-size: var(--text-xs);
    text-align: center;
  }
</style>
