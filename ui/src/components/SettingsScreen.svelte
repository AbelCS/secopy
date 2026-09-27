<script lang="ts">
  // Settings (RFD §5.5): what every job does. Everything applies at once; the next job uses
  // it. Source profiles have their own screen.
  import { useApi } from "../lib/api";
  import type { Settings } from "../lib/bindings";

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
  let settingsError: string | null = $state(null);

  async function toggle(key: keyof Settings, on: boolean) {
    const next = { ...settings, [key]: on };
    onSettings(next); // applies for this run even if saving fails
    try {
      await api.setSettings(next);
      settingsError = null;
    } catch (e) {
      settingsError = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<section class="card" aria-labelledby="settings-title">
  <h2 id="settings-title">Settings</h2>
  <label class="check">
    <input
      type="checkbox"
      checked={settings.writeChecksumFile}
      onchange={(e) => toggle("writeChecksumFile", e.currentTarget.checked)}
    />
    Write the checksum file to the destination
  </label>
  <label class="check">
    <input
      type="checkbox"
      checked={settings.showHiddenCount}
      onchange={(e) => toggle("showHiddenCount", e.currentTarget.checked)}
    />
    Show the count of skipped hidden items
  </label>
  <label class="check">
    <input
      type="checkbox"
      checked={settings.reportNextToChecksum}
      disabled={!settings.writeChecksumFile}
      onchange={(e) => toggle("reportNextToChecksum", e.currentTarget.checked)}
    />
    Also save the job report next to the checksum file
  </label>
  {#if settingsError}<p class="danger" role="alert">{settingsError}</p>{/if}
</section>

<footer class="done">
  <button type="button" class="primary" onclick={onDone}>Done</button>
</footer>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: var(--gap);
  }

  h2 {
    margin: 0 0 10px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .check {
    display: block;
    margin: 6px 0;
  }

  .danger {
    color: var(--danger);
  }

  .done {
    display: flex;
    justify-content: flex-end;
  }
</style>
