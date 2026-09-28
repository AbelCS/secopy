<script lang="ts">
  // File › Export… (#77): what goes in the .secopy file. Kinds with nothing in them are off.
  import type { ExportWhat } from "../lib/bindings";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";

  let {
    copyPresets,
    mirrorPresets,
    onExport,
    onClose,
  }: {
    copyPresets: number;
    mirrorPresets: number;
    onExport: (what: ExportWhat) => void;
    onClose: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let what: ExportWhat = $state({ settings: true, copyPresets: copyPresets > 0, mirrorPresets: mirrorPresets > 0 });
  const any = $derived(what.settings || what.copyPresets || what.mirrorPresets);
  const count = (n: number) => (n > 0 ? `(${n})` : "(none)");
</script>

<Dialog title="Export" {onClose}>
  <p>Choose what goes in the file. The queue and this Mac's recent destinations stay here.</p>
  <div class="kinds">
    <Checkbox label="Settings" checked={what.settings} onChange={(on) => (what.settings = on)} />
    <Checkbox
      label="Copy presets {count(copyPresets)}"
      checked={what.copyPresets}
      disabled={copyPresets === 0}
      onChange={(on) => (what.copyPresets = on)}
    />
    <Checkbox
      label="Mirror presets {count(mirrorPresets)}"
      checked={what.mirrorPresets}
      disabled={mirrorPresets === 0}
      onChange={(on) => (what.mirrorPresets = on)}
    />
  </div>
  {#snippet actions()}
    <Button onclick={onClose} data-autofocus>Cancel</Button>
    <Button variant="primary" disabled={!any} onclick={() => onExport({ ...what })}>Export…</Button>
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0;
  }

  .kinds {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }
</style>
