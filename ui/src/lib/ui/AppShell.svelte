<script lang="ts">
  // Every screen's frame: the header on top, content that scrolls, the action bar at the
  // bottom (docs/design/design-system.md).
  import type { Snippet } from "svelte";

  let { header, actions, children }: { header: Snippet; actions?: Snippet; children: Snippet } = $props();
</script>

<div class="shell">
  <header class="header">{@render header()}</header>
  <main class="content">
    <div class="inner">{@render children()}</div>
  </main>
  {#if actions}<footer class="actions">{@render actions()}</footer>{/if}
</div>

<style>
  .shell {
    display: grid;
    grid-template-rows: auto 1fr auto;
    height: 100vh;
  }

  .header {
    padding: var(--space-4) var(--space-5) var(--space-3);
  }

  .content {
    overflow-y: auto;
    padding: 0 var(--space-5) var(--space-4);
  }

  .inner {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .actions {
    border-top: 1px solid var(--border);
    background: var(--surface);
    padding: var(--space-3) var(--space-5);
  }
</style>
