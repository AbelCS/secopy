<script lang="ts" generics="T">
  // A small set of exclusive options under a legend (File types: All types / Only these).
  // Options with a `help` line (and a `warning`, in the warning style) are stacked.
  import Icon from "./Icon.svelte";

  let {
    legend,
    hideLegend = false,
    options,
    value,
    onChange,
  }: {
    legend: string;
    /** Kept for screen readers only, when a FormRow already shows it. */
    hideLegend?: boolean;
    options: { value: T; label: string; help?: string; warning?: string }[];
    value: T;
    onChange: (value: T) => void;
  } = $props();
  const name = $props.id();
  const stacked = $derived(options.some((o) => o.help || o.warning));
</script>

<fieldset class="radio-group">
  <legend class:visually-hidden={hideLegend}>{legend}</legend>
  <div class="options" class:stacked>
    {#each options as option, i (option.label)}
      {@const detail = option.help || option.warning ? `${name}-${i}` : undefined}
      <div class="option">
        <label>
          <input
            type="radio"
            {name}
            checked={option.value === value}
            aria-describedby={detail}
            onchange={() => onChange(option.value)}
          />
          {option.label}
        </label>
        {#if detail}
          <div class="detail" id={detail}>
            {#if option.help}<span class="help">{option.help}</span>{/if}
            {#if option.warning}<span class="warning"><Icon name="alert" size={12} />{option.warning}</span>{/if}
          </div>
        {/if}
      </div>
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

  .options.stacked {
    flex-direction: column;
    gap: var(--space-2);
  }

  .detail {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-left: calc(13px + var(--space-2));
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .warning {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--warning);
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
