<script lang="ts" module>
  /** A figure, or one that explains itself on hover. */
  export type Stat = string | { text: string; hint: string };
</script>

<script lang="ts">
  // Figures on one line: "3 files · 7.0 GB written · took 0:06". A figure that needs it can
  // explain itself (a `Hint`).
  import Hint from "./Hint.svelte";

  let { items }: { items: Stat[] } = $props();
</script>

<ul class="stats">
  {#each items as item, i (i)}
    <li>{#if typeof item === "string"}{item}{:else}<Hint text={item.hint}>{item.text}</Hint>{/if}</li>
  {/each}
</ul>

<style>
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-2);
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--text-muted);
  }

  li + li::before {
    content: "·";
    margin-right: var(--space-2);
  }
</style>
