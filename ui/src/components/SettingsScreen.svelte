<script lang="ts">
  // Settings (RFD §5.5): what every job does. Like macOS settings, a change applies and is
  // saved at once; there is nothing to apply or cancel. Source profiles have their own screen.
  import { useApi } from "../lib/api";
  import type { Settings } from "../lib/bindings";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";

  let {
    settings,
    onSettings,
    onDone,
  }: {
    settings: Settings;
    onSettings: (settings: Settings) => void;
    onDone: () => void;
  } = $props();

  const api = useApi();
  const SAVED_MS = 2000;
  let settingsError: string | null = $state(null);
  let saved = $state(false);
  let savedTimer: ReturnType<typeof setTimeout> | undefined;

  async function toggle(key: keyof Settings, on: boolean) {
    const next = { ...settings, [key]: on };
    onSettings(next); // applies for this run even if saving fails
    saved = false;
    try {
      await api.setSettings(next);
      settingsError = null;
      saved = true;
      clearTimeout(savedTimer);
      savedTimer = setTimeout(() => (saved = false), SAVED_MS);
    } catch (e) {
      settingsError = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<AppShell>
  {#snippet header()}<ScreenHeader title="Settings" onBack={onDone} />{/snippet}

  <Section title="Every copy">
    <div class="options">
      <Checkbox
        label="Write the checksum file to the destination"
        checked={settings.writeChecksumFile}
        onChange={(on) => toggle("writeChecksumFile", on)}
      >
        {#snippet help()}
          A <span class="mono">secopy_….xxh64</span> file lists every copied file with its checksum, so the copy
          can be checked again later, for example with <span class="mono">xxhsum -c</span>.
        {/snippet}
      </Checkbox>
      <Checkbox
        label="Show the count of skipped system files"
        checked={settings.showSystemCount}
        onChange={(on) => toggle("showSystemCount", on)}
      >
        {#snippet help()}
          Files computers leave on a card (<span class="mono">.DS_Store</span>, <span class="mono">._*</span>,
          <span class="mono">Thumbs.db</span>…) are never copied; this only shows how many were skipped. Hidden
          files the camera wrote are copied.
        {/snippet}
      </Checkbox>
      <Checkbox
        label="Also save the job report next to the checksum file"
        checked={settings.reportNextToChecksum}
        disabled={!settings.writeChecksumFile}
        onChange={(on) => toggle("reportNextToChecksum", on)}
      >
        {#snippet help()}Every job's report is also kept in the app; this adds a copy next to the checksum file.{/snippet}
      </Checkbox>
      <Checkbox
        label="Notify when a copy finishes"
        checked={settings.notifyWhenDone}
        onChange={(on) => toggle("notifyWhenDone", on)}
      >
        {#snippet help()}Only when Secopy's window isn't in front. macOS asks for permission the first time.{/snippet}
      </Checkbox>
    </div>
    {#if settingsError}<Notice tone="danger">{settingsError}</Notice>{/if}
  </Section>

  {#snippet actions()}
    <ActionBar>
      {#snippet status()}
        {#if saved}<span class="saved">Saved</span>{:else}Changes are saved as you make them.{/if}
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .options {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding-top: var(--space-1);
  }

  .saved {
    color: var(--success);
  }
</style>
