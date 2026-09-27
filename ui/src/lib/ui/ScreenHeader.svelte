<script lang="ts">
  // Top of every screen: the title, and screen-wide controls on the right. Buttons that act
  // on the screen, Back included, are in the action bar.
  import { onMount, type Snippet } from "svelte";

  let { title, trailing }: { title: string; trailing?: Snippet } = $props();

  // A new screen takes focus at its title, so a screen reader says where you are (NFR-10).
  let heading: HTMLHeadingElement;
  onMount(() => heading.focus());
</script>

<div class="screen-header">
  <div class="lead">
    <h1 bind:this={heading} tabindex="-1">{title}</h1>
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

  /* A heading isn't a control: no focus ring. */
  h1:focus {
    outline: none;
  }

  .trailing {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-muted);
  }
</style>
