<script lang="ts">
  // Settings (RFD §5.5): what every job does. Changes apply when saved; Cancel (or Esc) drops
  // them. Copy presets have their own screen.
  import { messageOf } from "../lib/format";
  import { useApi } from "../lib/api";
  import type { Snippet } from "svelte";
  import type { Settings } from "../lib/bindings";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";

  let {
    settings,
    banner,
    onSettings,
    onExport,
    onImport,
    onDone,
  }: {
    settings: Settings;
    /** App-wide messages, shown first. */
    banner?: Snippet;
    onSettings: (settings: Settings) => void;
    /** Export… and Import… (#77): a .secopy file with the settings and presets. */
    onExport?: () => void;
    onImport?: () => void;
    onDone: () => void;
  } = $props();

  const api = useApi();
  // The screen is recreated each time it opens, so the draft starts from the saved settings.
  // svelte-ignore state_referenced_locally
  let draft: Settings = $state({ ...settings });
  let saving = $state(false);
  let settingsError: string | null = $state(null);
  const changed = $derived((Object.keys(draft) as (keyof Settings)[]).some((k) => draft[k] !== settings[k]));

  /** Whether it's fine to leave: asks when there are unsaved changes (Import opens over it). */
  export async function mayLeave(): Promise<boolean> {
    if (!changed) return true;
    return await api.confirm("Your changes to the settings aren't saved.", "Discard changes?", "Discard", "Keep editing");
  }

  async function save() {
    saving = true;
    try {
      onSettings(await api.setSettings(draft));
      onDone();
    } catch (e) {
      settingsError = messageOf(e);
    } finally {
      saving = false;
    }
  }
</script>

<!-- A held Esc repeats: only the first press counts. -->
<svelte:window onkeydown={(e) => e.key === "Escape" && !e.repeat && onDone()} />

<AppShell>
  {#snippet header()}<ScreenHeader title="Settings" />{/snippet}

  {#if banner}{@render banner()}{/if}

  <Section title="Every copy">
    <div class="options">
      <Checkbox
        label="Write the checksum file to the destination"
        checked={draft.writeChecksumFile}
        onChange={(on) => (draft.writeChecksumFile = on)}
      >
        {#snippet help()}
          A <span class="mono">secopy_….xxh64</span> file lists every copied file with its checksum, so the copy
          can be checked again later, for example with <span class="mono">xxhsum -c</span>.
        {/snippet}
      </Checkbox>
      <Checkbox
        label="Show the count of skipped system files"
        checked={draft.showSystemCount}
        onChange={(on) => (draft.showSystemCount = on)}
      >
        {#snippet help()}
          Files computers leave behind (<span class="mono">.DS_Store</span>, <span class="mono">._*</span>,
          <span class="mono">Thumbs.db</span>…) are never copied; this only shows how many were skipped. Every
          other file is copied, hidden or not.
        {/snippet}
      </Checkbox>
      <Checkbox
        label="Also save the job report next to the checksum file"
        checked={draft.reportNextToChecksum}
        disabled={!draft.writeChecksumFile}
        onChange={(on) => (draft.reportNextToChecksum = on)}
      >
        {#snippet help()}Every job's report is also kept in the app; this adds a copy next to the checksum file.{/snippet}
      </Checkbox>
      <Checkbox
        label="Notify when a copy finishes"
        checked={draft.notifyWhenDone}
        onChange={(on) => (draft.notifyWhenDone = on)}
      >
        {#snippet help()}Only when Secopy's window isn't in front. macOS asks for permission the first time.{/snippet}
      </Checkbox>
    </div>
    {#if settingsError}<Notice tone="danger">{settingsError}</Notice>{/if}
  </Section>

  {#if onExport || onImport}
    <Section title="Settings and presets">
      <p class="muted">Move your settings and presets to a new Mac, or share presets, in a .secopy file.</p>
      <div class="transfer">
        {#if onExport}
          <Button
            help={changed ? "Exports your saved settings; save first to include these changes." : ""}
            onclick={onExport}>Export…</Button
          >
        {/if}
        {#if onImport}<Button onclick={onImport}>Import…</Button>{/if}
      </div>
    </Section>
  {/if}

  {#snippet actions()}
    <ActionBar>
      {#snippet start()}<Button onclick={onDone}>Cancel</Button>{/snippet}
      {#snippet end()}
        <Button variant="primary" disabled={!changed || saving} onclick={save}>Save</Button>
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .transfer {
    display: flex;
    gap: var(--space-2);
  }

  .muted {
    margin: 0 0 var(--space-3);
    color: var(--text-muted);
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding-top: var(--space-1);
  }
</style>
