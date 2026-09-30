<script lang="ts">
  // The file-type filter (FR-7..FR-9): one chip per extension, largest first. All / None are
  // the row's actions (Setup).
  import { t } from "../lib/i18n";
  import type { ExtensionView } from "../lib/bindings";
  import { formatBytes, formatCount } from "../lib/format";
  import Chip from "../lib/ui/Chip.svelte";

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

<div class="chips">
  {#each extensions as ext (ext.key)}
    <Chip
      label={ext.key === null ? t("copy.noExtension") : `.${ext.key}`}
      meta="{formatCount(ext.files)}{t('format.dot')}{formatBytes(ext.bytes)}"
      selected={isOn(ext.key)}
      onToggle={() => toggle(ext.key)}
    />
  {/each}
</div>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
</style>
