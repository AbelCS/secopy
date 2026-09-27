<script lang="ts">
  // One labelled row inside a Section: the label in a fixed column on the left, the content,
  // and the row's own actions (Choose…, All · None) on the right. Rows keep a section
  // readable: every kind of thing in its own line, the labels in one column.
  import type { Snippet } from "svelte";

  let {
    label,
    aside,
    children,
  }: {
    label: string;
    aside?: Snippet;
    children: Snippet;
  } = $props();
  const id = $props.id();
</script>

<div class="row form-row" class:with-aside={!!aside} role="group" aria-labelledby={id}>
  <div class="label" {id}>{label}</div>
  <div class="content">{@render children()}</div>
  {#if aside}<div class="aside">{@render aside()}</div>{/if}
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr);
    align-items: start;
    gap: var(--space-3);
    padding: var(--space-2) 0;
  }

  /* A hairline between rows in the same section. */
  :global(.form-row) + .row {
    border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  }

  .with-aside {
    grid-template-columns: 96px minmax(0, 1fr) auto;
  }

  .label {
    padding-top: 5px;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
    padding-top: 2px;
  }

  .aside {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
</style>
