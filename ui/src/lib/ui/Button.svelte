<script lang="ts">
  // The one button. primary: the screen's main action (one per screen, bottom right).
  // secondary: everything else. danger: destructive, kept apart on the left. link: navigation.
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    variant = "secondary",
    icon,
    type = "button",
    children,
    ...rest
  }: HTMLButtonAttributes & {
    variant?: "primary" | "secondary" | "danger" | "link";
    icon?: IconName;
    children: Snippet;
  } = $props();
</script>

<button {type} class="button {variant}" {...rest}>
  {#if icon}<Icon name={icon} />{/if}
  {@render children()}
</button>

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
    background: color-mix(in srgb, var(--accent-strong) 88%, white);
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
</style>
