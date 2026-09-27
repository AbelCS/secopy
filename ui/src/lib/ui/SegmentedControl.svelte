<script lang="ts" generics="T extends string">
  // One control for a small exclusive choice (Copy / Copy & Verify); the chosen one is filled.
  let {
    label,
    options,
    value,
    onChange,
  }: {
    /** Names the group for screen readers. */
    label: string;
    options: { value: T; label: string }[];
    value: T;
    onChange: (value: T) => void;
  } = $props();
  const name = $props.id();
</script>

<div class="segmented" role="radiogroup" aria-label={label}>
  {#each options as option (option.value)}
    <label class:on={option.value === value}>
      <input type="radio" {name} checked={option.value === value} onchange={() => onChange(option.value)} />
      {option.label}
    </label>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    overflow: hidden;
  }

  label {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-height: var(--control-height);
    padding: 0 var(--space-3);
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--duration) var(--ease);
  }

  label + label {
    border-left: 1px solid var(--border);
  }

  label.on {
    background: var(--accent);
    color: var(--on-accent);
  }

  label:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
</style>
