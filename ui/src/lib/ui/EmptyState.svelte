<script lang="ts">
  // What an empty part of a screen is for, and how to fill it.
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();
</script>

<div class="empty">{@render children()}</div>

<style>
  .empty {
    color: var(--text-muted);
    line-height: 1.5;
    max-width: 56ch;
  }
</style>
