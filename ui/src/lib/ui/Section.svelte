<script lang="ts">
  // A titled part of a screen (FROM, TO, Files…). The only card style in the app.
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  let {
    title,
    aside,
    children,
    ...rest
  }: HTMLAttributes<HTMLElement> & {
    title: string;
    /** Small controls on the title's line, on the right. */
    aside?: Snippet;
    children: Snippet;
  } = $props();
  const id = $props.id();
</script>

<section class="section" aria-labelledby={id} {...rest}>
  <div class="head">
    <h2 {id}>{title}</h2>
    {#if aside}<div class="aside">{@render aside()}</div>{/if}
  </div>
  {@render children()}
</section>

<style>
  .section {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-3) var(--space-4) var(--space-4);
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-2);
  }

  h2 {
    margin: 0;
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
</style>
