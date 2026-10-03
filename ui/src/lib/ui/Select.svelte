<script lang="ts">
  // A labelled menu; the label sits beside it.
  let {
    label,
    hideLabel = false,
    value,
    options,
    disabled = false,
    onChange,
  }: {
    label: string;
    /** Kept for screen readers only, when a FormRow already shows the label. */
    hideLabel?: boolean;
    value: string;
    /** `lang`: an option in another language (a language's own name): read as that, never translated. */
    options: { value: string; label: string; lang?: string }[];
    disabled?: boolean;
    /** Gets the menu too, e.g. to put its value back after an action item. */
    onChange: (value: string, menu: HTMLSelectElement) => void;
  } = $props();
  const id = $props.id();
</script>

<span class="select">
  <label for={id} class:visually-hidden={hideLabel}>{label}</label>
  <select {id} {value} {disabled} onchange={(e) => onChange(e.currentTarget.value, e.currentTarget)}>
    {#each options as option (option.value)}<option
        value={option.value}
        lang={option.lang}
        translate={option.lang ? "no" : undefined}>{option.label}</option
      >{/each}
  </select>
</span>

<style>
  .select {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }

  select {
    min-height: var(--control-height);
    min-width: 160px;
  }
</style>
