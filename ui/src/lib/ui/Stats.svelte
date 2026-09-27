<script lang="ts">
  // Figures on one line: "3 files · 7.0 GB written · took 0:06".
  let { items }: { items: string[] } = $props();
</script>

<ul class="stats">
  {#each items as item, i (i)}<li>{item}</li>{/each}
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
