<script lang="ts">
  // The file-type filter (FR-7..FR-9): one chip per extension, largest first, with All / None.
  import type { ExtensionView } from "../lib/bindings";
  import { formatBytes, formatCount } from "../lib/format";

  let {
    extensions,
    selected,
    onChange,
  }: {
    extensions: ExtensionView[];
    /** `null` = every extension. */
    selected: (string | null)[] | null;
    /** `null` when every extension ends up selected. */
    onChange: (selected: (string | null)[] | null) => void;
  } = $props();

  const isOn = (key: string | null) => selected === null || selected.includes(key);

  function toggle(key: string | null) {
    const current = selected ?? extensions.map((e) => e.key);
    const next = current.includes(key) ? current.filter((k) => k !== key) : [...current, key];
    onChange(next.length === extensions.length ? null : next);
  }
</script>

<div class="chips" role="group" aria-label="File types">
  {#each extensions as ext (ext.key)}
    <button
      type="button"
      class="chip"
      class:on={isOn(ext.key)}
      class:selected-fill={isOn(ext.key)}
      aria-pressed={isOn(ext.key)}
      onclick={() => toggle(ext.key)}
    >
      {#if isOn(ext.key)}<span class="tick" aria-hidden="true">✓</span>{/if}{ext.label} <span class="muted">{formatCount(ext.files)} · {formatBytes(ext.bytes)}</span>
    </button>
  {/each}
  <button type="button" class="link" onclick={() => onChange(null)}>All</button>
  <button type="button" class="link" onclick={() => onChange([])}>None</button>
</div>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    padding: 3px 10px;
    border-radius: 999px;
    color: var(--text-muted);
  }

  .chip.on {
    color: var(--text);
  }

  .tick {
    margin-right: 4px;
  }

  .muted {
    color: var(--text-muted);
  }

  .link {
    background: none;
    border: none;
    color: var(--accent);
    padding: 3px 6px;
  }
</style>
