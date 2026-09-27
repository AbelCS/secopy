<script lang="ts">
  // A checkbox with its label, and an optional line of help under it.
  import type { Snippet } from "svelte";

  let {
    label,
    checked,
    disabled = false,
    help,
    onChange,
  }: {
    label: string;
    checked: boolean;
    disabled?: boolean;
    help?: Snippet;
    onChange: (checked: boolean) => void;
  } = $props();
  const id = $props.id();
</script>

<div class="checkbox">
  <label>
    <input
      type="checkbox"
      {checked}
      {disabled}
      aria-describedby={help ? `${id}-help` : undefined}
      onchange={(e) => onChange(e.currentTarget.checked)}
    />
    {label}
  </label>
  {#if help}<p id="{id}-help" class="help">{@render help()}</p>{/if}
</div>

<style>
  label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  input {
    margin: 0;
    accent-color: var(--accent);
  }

  .help {
    margin: var(--space-1) 0 0 calc(13px + var(--space-2));
    font-size: var(--text-sm);
    color: var(--text-muted);
  }
</style>
