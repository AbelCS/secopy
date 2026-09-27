<script lang="ts">
  // The bottom bar: other and destructive actions on the left, a short status in the middle,
  // the screen's main action on the right.
  import type { Snippet } from "svelte";

  let { start, status, end }: { start?: Snippet; status?: string | Snippet; end?: Snippet } = $props();
</script>

<div class="bar" role="group" aria-label="Actions">
  <div class="start">{#if start}{@render start()}{/if}</div>
  <div class="status" aria-live="polite">
    {#if typeof status === "string"}{status}{:else if status}{@render status()}{/if}
  </div>
  <div class="end">{#if end}{@render end()}{/if}</div>
</div>

<style>
  .bar {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: var(--space-3);
  }

  .start,
  .end {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .status {
    min-width: 0;
    text-align: right;
    font-size: var(--text-sm);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
