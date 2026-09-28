<script lang="ts">
  // A text field with a visible label, optional help, and its error right under it.
  import type { HTMLInputAttributes } from "svelte/elements";

  let {
    label,
    hideLabel = false,
    value = $bindable(""),
    help,
    error = null,
    mono = false,
    ...rest
  }: HTMLInputAttributes & {
    label: string;
    /** Kept for screen readers only, when a FormRow already shows the label. */
    hideLabel?: boolean;
    value?: string;
    help?: string;
    error?: string | null;
    mono?: boolean;
  } = $props();
  const id = $props.id();
  const described = $derived(error ? `${id}-error` : help ? `${id}-help` : undefined);
</script>

<div class="field">
  <label for={id} class:visually-hidden={hideLabel}>{label}</label>
  <div class="row">
    <input
      {id}
      class:mono
      bind:value
      aria-describedby={described}
      aria-invalid={error ? "true" : undefined}
      {...rest}
    />
  </div>
  {#if error}
    <p id="{id}-error" class="error" role="alert">{error}</p>
  {:else if help}
    <p id="{id}-help" class="help">{help}</p>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  label {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .row {
    display: flex;
    gap: var(--space-2);
  }

  input {
    flex: 1;
    min-width: 0;
    min-height: var(--control-height);
  }

  input[aria-invalid="true"] {
    border-color: var(--danger);
  }

  .help,
  .error {
    margin: 0;
    font-size: var(--text-sm);
  }

  .help {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }
</style>
