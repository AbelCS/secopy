<script lang="ts">
  // Settings (RFD §5.5): what every job does. Changes apply when saved; Cancel (or Esc) drops
  // them. Copy presets have their own screen.
  import { t, tParts } from "../lib/i18n";
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
  // File names and a command: shown as they are, in mono, inside the translated help.
  const CODE = { file: "secopy_….xxh64", command: "xxhsum -c", a: ".DS_Store", b: "._*", c: "Thumbs.db" };
  // The screen is recreated each time it opens, so the draft starts from the saved settings.
  // svelte-ignore state_referenced_locally
  let draft: Settings = $state({ ...settings });
  let saving = $state(false);
  let settingsError: string | null = $state(null);
  /** A setting's value, compared by what it holds: the ignore list is an array (#158). */
  const same = (a: unknown, b: unknown) =>
    Array.isArray(a) && Array.isArray(b) ? a.length === b.length && a.every((x, i) => x === b[i]) : a === b;
  const changed = $derived((Object.keys(draft) as (keyof Settings)[]).some((k) => !same(draft[k], settings[k])));

  /** Whether it's fine to leave: asks when there are unsaved changes (Import opens over it). */
  export async function mayLeave(): Promise<boolean> {
    if (!changed) return true;
    return await api.confirm(t("settings.discard"), t("ui.discard.title"), t("ui.discard.discard"), t("ui.discard.keep"));
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

{#snippet withCode(parts: { text: string; value: boolean }[])}
  {#each parts as p, i (i)}{#if p.value}<span class="mono">{p.text}</span>{:else}{p.text}{/if}{/each}
{/snippet}

<!-- A held Esc repeats: only the first press counts. -->
<svelte:window onkeydown={(e) => e.key === "Escape" && !e.repeat && onDone()} />

<AppShell>
  {#snippet header()}<ScreenHeader title={t("settings.title")} />{/snippet}

  {#if banner}{@render banner()}{/if}

  <Section title={t("settings.everyCopy")}>
    <div class="options">
      <Checkbox
        label={t("settings.checksumFile.label")}
        checked={draft.writeChecksumFile}
        onChange={(on) => (draft.writeChecksumFile = on)}
      >
        {#snippet help()}
          {@render withCode(tParts("settings.checksumFile.help", CODE))}
        {/snippet}
      </Checkbox>
      <Checkbox
        label={t("settings.mhl.label")}
        checked={draft.writeMhl}
        onChange={(on) => (draft.writeMhl = on)}
      >
        {#snippet help()}{t("settings.mhl.help")}{/snippet}
      </Checkbox>
      <Checkbox
        label={t("settings.systemCount.label")}
        checked={draft.showSystemCount}
        onChange={(on) => (draft.showSystemCount = on)}
      >
        {#snippet help()}
          {@render withCode(tParts("settings.systemCount.help", CODE))}
        {/snippet}
      </Checkbox>
      <Checkbox
        label={t("settings.report.label")}
        checked={draft.reportNextToChecksum}
        disabled={!draft.writeChecksumFile}
        onChange={(on) => (draft.reportNextToChecksum = on)}
      >
        {#snippet help()}{t("settings.report.help")}{/snippet}
      </Checkbox>
      <Checkbox
        label={t("settings.notify.label")}
        checked={draft.notifyWhenDone}
        onChange={(on) => (draft.notifyWhenDone = on)}
      >
        {#snippet help()}{t("settings.notify.help")}{/snippet}
      </Checkbox>
      <Checkbox
        label={t("settings.menuBar.label")}
        checked={draft.keepInMenuBar}
        onChange={(on) => (draft.keepInMenuBar = on)}
      >
        {#snippet help()}
          {t("settings.menuBar.help")}
        {/snippet}
      </Checkbox>
    </div>
    {#if settingsError}<Notice tone="danger">{settingsError}</Notice>{/if}
  </Section>

  {#if onExport || onImport}
    <Section title={t("settings.transfer.title")}>
      <p class="muted">{t("settings.transfer.about")}</p>
      <div class="transfer">
        {#if onExport}
          <Button
            help={changed ? t("settings.transfer.exportHelp") : ""}
            onclick={onExport}>{t("ui.export")}</Button
          >
        {/if}
        {#if onImport}<Button onclick={onImport}>{t("ui.importFile")}</Button>{/if}
      </div>
    </Section>
  {/if}

  {#snippet actions()}
    <ActionBar>
      {#snippet start()}<Button onclick={onDone}>{t("ui.cancel")}</Button>{/snippet}
      {#snippet end()}
        <Button variant="primary" disabled={!changed || saving} onclick={save}>{t("ui.save")}</Button>
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
