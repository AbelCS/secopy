<script module lang="ts">
  /** Open dialogs, the last one on top: only it answers Esc. */
  const open: symbol[] = [];
</script>

<script lang="ts">
  // A question over the screen, for choices a system dialog can't hold (a checkbox). The
  // element marked data-autofocus gets the focus (the safe answer); Esc is `onClose`.
  import { onMount, type Snippet } from "svelte";

  let {
    title,
    children,
    actions,
    onClose,
  }: {
    title: string;
    children: Snippet;
    /** The answers, safe one first. */
    actions: Snippet;
    onClose: () => void;
  } = $props();
  const id = $props.id();
  let panel: HTMLElement;

  const me = Symbol();

  onMount(() => {
    open.push(me);
    const before = document.activeElement as HTMLElement | null;
    (panel.querySelector<HTMLElement>("[data-autofocus]") ?? panel).focus();
    return () => {
      open.splice(open.indexOf(me), 1);
      before?.focus?.();
    };
  });

  // Caught on the way down, so Esc answers the dialog only: the screen behind doesn't also
  // close or ask again, nor does a dialog under this one.
  function onKey(e: KeyboardEvent) {
    if (open.at(-1) !== me) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "Tab") {
      // Focus goes round the dialog's own controls, never to the screen behind (#117).
      const inside = [
        ...panel.querySelectorAll<HTMLElement>("button, input, select, textarea, [tabindex]:not([tabindex='-1'])"),
      ].filter((el) => !el.hasAttribute("disabled"));
      if (inside.length === 0) return;
      const at = inside.indexOf(document.activeElement as HTMLElement);
      const next = e.shiftKey ? (at <= 0 ? inside.length - 1 : at - 1) : at === inside.length - 1 ? 0 : at + 1;
      e.preventDefault();
      inside[next].focus();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="backdrop">
  <div bind:this={panel} class="dialog" role="dialog" aria-modal="true" aria-labelledby="{id}-title" tabindex="-1">
    <h2 id="{id}-title">{title}</h2>
    <div class="body">{@render children()}</div>
    <div class="actions">{@render actions()}</div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 60%, transparent);
  }

  .dialog {
    width: min(440px, calc(100vw - 2 * var(--space-4)));
    padding: var(--space-4);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 12px 40px rgb(0 0 0 / 45%);
    outline: none;
  }

  h2 {
    margin: 0 0 var(--space-2);
    font-size: var(--text-lg);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .body :global(p) {
    margin: 0;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }
</style>
