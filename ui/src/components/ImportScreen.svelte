<script lang="ts">
  // Import (#77): what a .secopy file holds, against what this Mac has. Nothing changes
  // until Import; Back leaves everything as it was.
  import type { Snippet } from "svelte";
  import type { ImportChoices, ImportView, PresetImport } from "../lib/bindings";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import RadioGroup from "../lib/ui/RadioGroup.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";

  let {
    view,
    banner,
    onImport,
    onBack,
  }: {
    view: ImportView;
    /** App-wide messages, shown first. */
    banner?: Snippet;
    onImport: (c: ImportChoices) => void | Promise<void>;
    onBack: () => void;
  } = $props();

  type Row = { on: boolean; replace: boolean };
  const rows = (list: PresetImport[]): Row[] => list.map((p) => ({ on: !p.problem, replace: false }));
  // svelte-ignore state_referenced_locally
  let settingsOn = $state(!!view.settings && !view.settings.problem && view.settings.changes.length > 0);
  // svelte-ignore state_referenced_locally
  let copy = $state(rows(view.copyPresets));
  // svelte-ignore state_referenced_locally
  let mirrors = $state(rows(view.mirrorPresets));

  const chosen = (list: Row[]) =>
    list.flatMap((r, index) => (r.on ? [{ index, replace: r.replace }] : []));
  const choices = $derived<ImportChoices>({
    settings: settingsOn,
    copyPresets: chosen(copy),
    mirrorPresets: chosen(mirrors),
  });
  const any = $derived(choices.settings || choices.copyPresets.length > 0 || choices.mirrorPresets.length > 0);
  /** Import is working: a second press imports nothing more. */
  let importing = $state(false);

  async function importChosen() {
    if (importing) return;
    importing = true;
    try {
      await onImport(choices);
    } finally {
      importing = false;
    }
  }
</script>

<!-- A held Esc repeats: only the first press counts. -->
<svelte:window onkeydown={(e) => e.key === "Escape" && !e.repeat && onBack()} />

{#snippet presets(title: string, list: PresetImport[], state: Row[])}
  {#if list.length > 0}
    <Section {title}>
      <ul class="items">
        {#each list as p, i (i)}
          <li>
            <Checkbox label={p.name} checked={state[i].on} disabled={!!p.problem} onChange={(on) => (state[i].on = on)} />
            {#each p.paths as path (path)}<p class="note mono">{path}</p>{/each}
            {#if p.problem}<p class="note problem">{p.problem}</p>{/if}
            {#each p.missing as path (path)}<p class="note">{path} isn't connected now.</p>{/each}
            {#if !p.clash && !p.problem && p.newName !== p.name}
              <p class="note">Imported as “{p.newName}”: the file has this name twice.</p>
            {/if}
            {#if p.clash && !p.problem}
              <div class="choice">
              <RadioGroup
                legend="You already have “{p.clash}”"
                options={[
                  { value: false, label: `Keep both, as “${p.newName}”` },
                  { value: true, label: "Replace yours" },
                ]}
                value={state[i].replace}
                onChange={(v) => (state[i].replace = v)}
              />
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    </Section>
  {/if}
{/snippet}

<AppShell>
  {#snippet header()}<ScreenHeader title="Import" />{/snippet}

  {#if banner}{@render banner()}{/if}
  <p class="file mono">{view.fileName}</p>

  {#if view.settings}
    <Section title="Settings">
      {#if view.settings.problem}
        <p class="problem">{view.settings.problem}</p>
      {:else if view.settings.changes.length === 0}
        <p class="note">Same as yours</p>
      {:else}
        <Checkbox label="Import the settings" checked={settingsOn} onChange={(on) => (settingsOn = on)} />
        <ul class="changes">{#each view.settings.changes as c (c)}<li>{c}</li>{/each}</ul>
      {/if}
    </Section>
  {/if}
  {@render presets("Copy presets", view.copyPresets, copy)}
  {@render presets("Mirror presets", view.mirrorPresets, mirrors)}

  {#snippet actions()}
    <ActionBar>
      {#snippet start()}<Button icon="chevron-left" onclick={onBack}>Back</Button>{/snippet}
      {#snippet end()}<Button variant="primary" disabled={!any || importing} onclick={importChosen}>Import</Button>{/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .file {
    color: var(--text-muted);
    margin: 0 0 var(--space-2);
  }

  .items,
  .changes {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .changes {
    gap: var(--space-1);
    margin: var(--space-2) 0 0 var(--space-5);
  }

  .note {
    margin: var(--space-1) 0 0 var(--space-5);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .problem {
    color: var(--danger);
  }

  .choice {
    margin: var(--space-2) 0 0 var(--space-5);
  }
</style>
