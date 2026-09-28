<script lang="ts">
  // FROM's copy preset (FR-38): pick one, see when this run differs from it, and save
  // the difference with Update preset or Save as new….
  import { messageOf } from "../lib/format";
  import { tick } from "svelte";
  import { useApi } from "../lib/api";
  import type { CopyPreset, CopyPresetsView, SessionView } from "../lib/bindings";
  import Button from "../lib/ui/Button.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import Select from "../lib/ui/Select.svelte";
  import TextField from "../lib/ui/TextField.svelte";

  let {
    view,
    presets,
    busy,
    onSelect,
    onApplied,
    onManage,
  }: {
    view: SessionView;
    presets: CopyPreset[];
    /** A scan is running: its result decides what Update / Save as new would save. */
    busy: boolean;
    onSelect: (id: string | null) => void;
    /** After Update preset / Save as new…: the presets and the new view. */
    onApplied: (result: CopyPresetsView) => void;
    onManage: () => void;
  } = $props();

  const api = useApi();
  const MANAGE = "manage";
  let error: string | null = $state(null);
  let savingAs = $state(false);
  let name = $state("");

  const selected = $derived(presets.find((p) => p.id === view.presetId) ?? null);
  /** Presets apply to folders only (spec B4). */
  const forFiles = $derived(!!view.source && !view.source.isFolder);
  const canSaveAs = $derived(
    !!view.source?.isFolder && !view.source.isRetry && (!selected || view.presetChanged),
  );

  /** The preset and the source; a new scan of the same source (filters) doesn't change it. */
  const context = $derived(`${view.presetId ?? ""}\n${view.source?.label ?? ""}`);
  // Another preset or source: an old error or an open Save as new… no longer applies.
  $effect(() => {
    void context;
    error = null;
    savingAs = false;
  });

  async function act(call: () => Promise<CopyPresetsView>) {
    try {
      onApplied(await call());
      error = null;
      savingAs = false;
    } catch (e) {
      error = messageOf(e);
    }
  }

  const options = $derived([
    { value: "", label: "None" },
    ...presets.map((p) => ({ value: p.id, label: p.name })),
    { value: MANAGE, label: "Manage presets…" },
  ]);

  function choose(value: string, menu: HTMLSelectElement) {
    if (value === MANAGE) {
      menu.value = view.presetId ?? "";
      onManage();
      return;
    }
    onSelect(value === "" ? null : value);
  }

  let bar: HTMLElement;

  /** Closes Save as new… and gives focus back to the button that opened it. */
  async function closeSaveAs() {
    savingAs = false;
    await tick();
    bar.querySelector<HTMLElement>("[data-save-as]")?.focus();
  }

  function openSaveAs() {
    name = "";
    error = null;
    savingAs = true;
  }

  function saveAs(event: SubmitEvent) {
    event.preventDefault();
    void act(() => api.saveCopyPresetAs(name));
  }
</script>

<div class="preset" bind:this={bar}>
  {#if presets.length > 0}
    <Select label="Preset" hideLabel value={view.presetId ?? ""} {options} disabled={forFiles} onChange={choose} />
  {:else if !canSaveAs}
    <!-- Nothing to choose yet: a menu with only None would be noise. -->
    <Button onclick={onManage}>Create a preset…</Button>
  {/if}
  {#if selected && view.presetChanged}
    <span class="muted">Changed for this run</span>
    <Button disabled={busy} onclick={() => act(() => api.updateCopyPreset())}>Update preset</Button>
  {/if}
  {#if canSaveAs}
    <Button disabled={busy} onclick={openSaveAs} data-save-as>Save as new…</Button>
  {/if}
</div>
{#if savingAs}
  <!-- Esc from any of its fields closes the form, like Cancel. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form
    class="save-as"
    onsubmit={saveAs}
    onkeydown={(e) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        void closeSaveAs();
      }
    }}
  >
    <TextField label="Name" bind:value={name} placeholder="e.g. Sony FX3" />
    <div class="buttons">
      <Button onclick={closeSaveAs}>Cancel</Button>
      <Button variant="primary" type="submit" disabled={busy}>Save</Button>
    </div>
  </form>
{/if}
{#if error}<Notice tone="danger">{error}</Notice>{/if}

<style>
  .preset {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  .save-as {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    align-items: end;
    gap: var(--space-3);
    padding: var(--space-3);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .buttons {
    display: flex;
    gap: var(--space-2);
  }

  .muted {
    color: var(--text-muted);
  }
</style>
