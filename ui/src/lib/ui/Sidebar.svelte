<script lang="ts">
  // The app's sections (RFD §5.7): Copy, Queue (and Mirror, plan 7), on the left.
  let {
    items,
    selected,
    onSelect,
  }: {
    items: { id: string; label: string; count?: number }[];
    selected: string;
    onSelect: (id: string) => void;
  } = $props();
</script>

<nav class="sidebar" aria-label="Sections">
  {#each items as item (item.id)}
    <button
      type="button"
      class="item"
      class:on={item.id === selected}
      aria-current={item.id === selected ? "page" : undefined}
      aria-label={item.count ? `${item.label}, ${item.count} ${item.count === 1 ? "job" : "jobs"}` : item.label}
      onclick={() => onSelect(item.id)}
    >
      <span>{item.label}</span>
      {#if item.count}<span class="count" aria-hidden="true">{item.count}</span>{/if}
    </button>
  {/each}
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-4) var(--space-2);
    background: var(--surface);
    border-right: 1px solid var(--border);
    height: 100vh;
    box-sizing: border-box;
  }

  .item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space-2) var(--space-3);
    font: inherit;
    color: var(--text);
    background: none;
    border: 0;
    border-radius: var(--radius-control);
    text-align: left;
    cursor: pointer;
  }

  .item:hover {
    background: var(--surface-raised);
  }

  .item.on {
    background: var(--accent-soft);
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
