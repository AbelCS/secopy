<script lang="ts" generics="T">
  // A small set of exclusive options under a legend (File types: All types / Only these).
  let {
    legend,
    options,
    value,
    onChange,
  }: {
    legend: string;
    options: { value: T; label: string }[];
    value: T;
    onChange: (value: T) => void;
  } = $props();
  const name = $props.id();
</script>

<fieldset class="radio-group">
  <legend>{legend}</legend>
  <div class="options">
    {#each options as option (option.label)}
      <label>
        <input type="radio" {name} checked={option.value === value} onchange={() => onChange(option.value)} />
        {option.label}
      </label>
    {/each}
  </div>
</fieldset>

<style>
  .radio-group {
    border: none;
    padding: 0;
    margin: 0;
  }

  legend {
    padding: 0;
    margin-bottom: var(--space-1);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .options {
    display: flex;
    gap: var(--space-4);
  }

  label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  input {
    margin: 0;
    accent-color: var(--accent);
  }
</style>
