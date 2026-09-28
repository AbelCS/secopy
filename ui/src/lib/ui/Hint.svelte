<script lang="ts">
  // A short explanation of a term, on hover and on keyboard focus: the term gets a dotted
  // underline, or with no term an ⓘ mark stands in (`label` names it). Only where a word
  // isn't clear on its own. VoiceOver reads the explanation after the term (a description);
  // it stays out of the names of the things around it.
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    text,
    children,
    label = "More about this",
    above = false,
  }: {
    text: string;
    /** The term explained; none for an ⓘ mark. */
    children?: Snippet;
    /** The mark's name for screen readers, "About Copy & Verify". */
    label?: string;
    /** Opens upwards, for terms near the bottom of the window. */
    above?: boolean;
  } = $props();
  const id = $props.id();
</script>

{#if children}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <span class="hint term" tabindex="0" aria-describedby={id}>
    {@render children()}<span class="tip" class:above role="tooltip" aria-hidden="true" {id}>{text}</span>
  </span>
{:else}
  <span class="hint">
    <button type="button" class="mark" aria-label={label} aria-describedby={id}><Icon name="info" size={14} /></button>
    <span class="tip" class:above role="tooltip" aria-hidden="true" {id}>{text}</span>
  </span>
{/if}

<style>
  .hint {
    position: relative;
    display: inline-flex;
    align-items: center;
  }

  .term {
    text-decoration: underline dotted;
    text-decoration-color: var(--text-muted);
    text-underline-offset: 3px;
    cursor: help;
  }

  .mark {
    display: inline-flex;
    padding: 2px;
    color: var(--text-muted);
    background: none;
    border: 0;
    border-radius: var(--radius-control);
    cursor: help;
  }

  .mark:hover {
    color: var(--text);
  }

  .tip {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 20;
    width: max-content;
    max-width: 300px;
    padding: 6px 8px;
    font-size: var(--text-sm);
    font-weight: 400;
    line-height: 1.4;
    letter-spacing: normal;
    text-transform: none;
    white-space: normal;
    text-align: left;
    color: var(--text);
    background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    box-shadow: 0 6px 20px rgb(0 0 0 / 40%);
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition: opacity 0.12s;
  }

  .tip.above {
    top: auto;
    bottom: calc(100% + 6px);
  }

  .hint:hover .tip,
  .hint:focus-within .tip {
    opacity: 1;
    visibility: visible;
  }
</style>
