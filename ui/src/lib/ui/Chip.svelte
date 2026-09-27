<script lang="ts">
  // A small pill: a toggle (a file type to copy, with ✓ when on) or a removable value.
  import Icon from "./Icon.svelte";

  let {
    label,
    meta,
    selected = false,
    onToggle,
    onRemove,
  }: {
    label: string;
    /** Secondary text, e.g. "106 · 180.0 GB". */
    meta?: string;
    selected?: boolean;
    onToggle?: () => void;
    /** Makes it a removable value instead of a toggle. */
    onRemove?: () => void;
  } = $props();
</script>

{#if onRemove}
  <span class="chip removable">
    {label}
    <button type="button" class="remove" aria-label="Remove {label}" onclick={onRemove}><Icon name="x" size={12} /></button>
  </span>
{:else}
  <button type="button" class="chip" class:selected aria-pressed={selected} onclick={onToggle}>
    {#if selected}<Icon name="check" size={12} />{/if}
    {label}
    {#if meta}<span class="meta">{meta}</span>{/if}
  </button>
{/if}

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    min-height: 26px;
    padding: 0 var(--space-3);
    font: inherit;
    color: var(--text-muted);
    background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    cursor: pointer;
    transition: background var(--duration) var(--ease);
  }

  .chip.selected {
    color: var(--text);
    background: var(--accent-soft);
    border-color: var(--accent);
  }

  .meta {
    color: var(--text-muted);
    margin-left: var(--space-1);
  }

  .removable {
    color: var(--text);
    border-color: var(--accent);
    background: var(--accent-soft);
    padding-right: var(--space-1);
    cursor: default;
  }

  .remove {
    display: inline-flex;
    padding: 2px;
    color: var(--text-muted);
    background: none;
    border: none;
    border-radius: var(--radius-pill);
    cursor: pointer;
  }
</style>
