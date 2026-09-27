<script lang="ts">
  // Top of every screen: ‹ Back (only when there is somewhere to go back to), the title, and
  // screen-wide controls on the right.
  import type { Snippet } from "svelte";
  import Button from "./Button.svelte";

  let {
    title,
    onBack,
    trailing,
  }: {
    title: string;
    onBack?: () => void;
    trailing?: Snippet;
  } = $props();
</script>

<div class="screen-header">
  <div class="lead">
    {#if onBack}
      <Button variant="link" icon="chevron-left" onclick={onBack}>Back</Button>
    {/if}
    <h1>{title}</h1>
  </div>
  {#if trailing}<div class="trailing">{@render trailing()}</div>{/if}
</div>

<style>
  .screen-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-3);
    min-height: 32px;
  }

  .lead {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .trailing {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-muted);
  }
</style>
