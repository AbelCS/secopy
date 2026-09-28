<script lang="ts">
  // The top bar: the kinds of job as tabs on the left (Copy, Mirror, Verify), and on the
  // right what isn't a kind of job: the Queue (where jobs wait and run, with their count)
  // and Settings. The current tab is underlined; the Queue is highlighted while it's open.
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    items,
    selected,
    onSelect,
    queue,
    trailing,
  }: {
    items: { id: string; label: string }[];
    selected: string;
    onSelect: (id: string) => void;
    /** The Queue button: its job count; `selected` is "queue" while it's open. */
    queue?: { count: number };
    /** After the Queue: Settings. */
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
        onclick={() => onSelect(item.id)}
      >
        {item.label}
      </button>
    {/each}
  </nav>
  <div class="trailing">
    {#if queue}
      <button
        type="button"
        class="queue"
        class:on={selected === "queue"}
        aria-current={selected === "queue" ? "page" : undefined}
        aria-label={queue.count ? `Queue, ${queue.count} ${queue.count === 1 ? "job" : "jobs"}` : "Queue"}
        onclick={() => onSelect("queue")}
      >
        <Icon name="list" />
        Queue
        {#if queue.count}<span class="count" aria-hidden="true">{queue.count}</span>{/if}
      </button>
    {/if}
    {#if trailing}{@render trailing()}{/if}
  </div>
</div>

<style>
  .tabbar {
    display: flex;
    align-items: stretch;
    justify-content: space-between;
    gap: var(--space-3);
    padding: 0 var(--space-5);
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .tabs {
    display: flex;
    gap: var(--space-5);
  }

  .tab {
    position: relative;
    padding: var(--space-3) var(--space-1);
    font: inherit;
    font-weight: 500;
    color: var(--text-muted);
    background: none;
    border: 0;
    cursor: pointer;
    transition: color var(--duration) var(--ease);
  }

  .tab:hover {
    color: var(--text);
  }

  /* The current tab: brighter, with an accent line on the bar's edge. */
  .tab.on {
    color: var(--text);
  }

  .tab.on::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    border-radius: 2px 2px 0 0;
    background: var(--accent);
  }

  .tab:focus-visible,
  .queue:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    border-radius: var(--radius-control);
  }

  .trailing {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .queue {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-height);
    padding: 0 var(--space-3);
    font: inherit;
    color: var(--text);
    background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .queue.on {
    background: var(--accent-soft);
    border-color: var(--accent);
  }

  .count {
    min-width: 20px;
    padding: 0 var(--space-1);
    border-radius: var(--radius-pill);
    background: var(--accent-strong);
    color: var(--on-accent);
    font-size: var(--text-xs);
    font-weight: 600;
    text-align: center;
  }
</style>
