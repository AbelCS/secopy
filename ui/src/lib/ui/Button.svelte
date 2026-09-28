<script lang="ts">
  // The one button. primary: the screen's main action (one per screen, bottom right).
  // secondary: everything else. danger: destructive, kept apart on the left. link: navigation.
  //
  // `help`: one sentence for a button whose short label hides the detail ("Start" → what it
  // copies, where, the shortcut). It opens above the button (most sit in the action bar at the
  // bottom) on hover, after a moment like a macOS help tag, and at once on keyboard focus;
  // VoiceOver reads it once, as the button's description. A disabled button shows none: its
  // figures aren't real yet. Without `help` the button is rendered on its own, as always.
  // Callers pass "" while the words aren't known yet, so the button isn't rebuilt when they come.
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    variant = "secondary",
    icon,
    type = "button",
    help,
    children,
    ...rest
  }: HTMLButtonAttributes & {
    variant?: "primary" | "secondary" | "danger" | "link";
    icon?: IconName;
    /**
     * What the button does, in one sentence with real figures; not shown while disabled. A
     * button whose words depend on data passes "" until they're known, never `undefined`.
     */
    help?: string;
    children: Snippet;
  } = $props();

  const id = $props.id();
  const tip = $derived(help && !rest.disabled ? help : undefined);
  let wrap: HTMLElement | undefined = $state();
  let tipEl: HTMLElement | undefined = $state();
  /** The tip lines up with the button's right edge: at its left edge it would run off the window. */
  let end = $state(false);

  /** Picks the side the tip lines up with, so it stays inside the window. */
  function place() {
    if (!wrap || !tipEl) return;
    const box = wrap.getBoundingClientRect();
    const width = tipEl.offsetWidth;
    const margin = 8;
    end = box.left + width > window.innerWidth - margin && box.right - width >= margin;
  }

  // Again when the words change (their width does too), and on every hover and focus below.
  $effect(() => {
    void tip;
    place();
  });
</script>

{#snippet label()}
  {#if icon}<Icon name={icon} />{/if}
  {@render children()}
{/snippet}

<!-- A `help` with no words yet ("") keeps the wrapper: the button isn't rebuilt when they come. -->
{#if help !== undefined}
  <span class="button-help" bind:this={wrap}>
    <button
      {type}
      class="button {variant}"
      aria-describedby={tip ? id : undefined}
      onpointerenter={place}
      onfocus={place}
      {...rest}>{@render label()}</button
    >
    {#if tip}<span class="tip" class:end role="tooltip" aria-hidden="true" {id} bind:this={tipEl}>{tip}</span>{/if}
  </span>
{:else}
  <button {type} class="button {variant}" {...rest}>{@render label()}</button>
{/if}

<style>
  .button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-height);
    padding: 0 var(--space-3);
    font: inherit;
    color: var(--text);
    background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--duration) var(--ease),
      border-color var(--duration) var(--ease);
  }

  .button:hover:not(:disabled) {
    border-color: var(--text-faint);
  }

  .primary {
    color: var(--on-accent);
    background: var(--accent-strong);
    border-color: var(--accent-strong);
  }

  .primary:hover:not(:disabled) {
    border-color: var(--accent-strong);
    background: var(--accent-strong-hover);
  }

  .danger {
    color: var(--danger);
  }

  .link {
    color: var(--accent);
    background: none;
    border-color: transparent;
    padding: 0;
    min-height: 0;
  }

  .link:hover:not(:disabled) {
    border-color: transparent;
    text-decoration: underline;
  }

  .button:disabled {
    color: var(--text-faint);
    background: var(--surface-raised);
    border-color: var(--border);
    cursor: default;
  }

  .link:disabled {
    background: none;
    border-color: transparent;
  }

  /* The tip: Hint's look, above the button. */
  .button-help {
    position: relative;
    display: inline-flex;
  }

  .tip {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    z-index: 20;
    width: max-content;
    max-width: 300px;
    padding: 6px 8px;
    font-size: var(--text-sm);
    font-weight: 400;
    line-height: 1.4;
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
    transition:
      opacity 0.12s,
      visibility 0.12s;
  }

  .tip.end {
    left: auto;
    right: 0;
  }

  /* Hover: after a moment, like a macOS help tag. */
  .button-help:hover .tip {
    opacity: 1;
    visibility: visible;
    transition-delay: 0.5s;
  }

  /* Keyboard focus: at once. */
  .button:focus-visible + .tip {
    opacity: 1;
    visibility: visible;
    transition-delay: 0s;
  }
</style>
