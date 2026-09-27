<script lang="ts">
  // Settings (RFD §5.5): what every job does. Like macOS settings, a change applies and is
  // saved at once; there is nothing to apply or cancel. Source profiles have their own screen.
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

<nav class="bar">
  <button type="button" class="back" onclick={onDone}><span aria-hidden="true">‹</span> Back</button>
</nav>

<section class="card" aria-labelledby="settings-title">
  <div class="head">
    <h2 id="settings-title">Settings</h2>
    <span class="saved" role="status">{saved ? "Saved" : ""}</span>
  </div>

  <div class="option">
    <label class="check">
      <input
        type="checkbox"
        checked={settings.writeChecksumFile}
        onchange={(e) => toggle("writeChecksumFile", e.currentTarget.checked)}
      />
      Write the checksum file to the destination
    </label>
    <p class="help">
      A <span class="mono">secopy_….xxh64</span> file lists every copied file with its checksum, so the
      copy can be checked again later, for example with <span class="mono">xxhsum -c</span>.
    </p>
  </div>

  <div class="option">
    <label class="check">
      <input
        type="checkbox"
        checked={settings.showHiddenCount}
        onchange={(e) => toggle("showHiddenCount", e.currentTarget.checked)}
      />
      Show the count of skipped hidden items
    </label>
    <p class="help">
      Hidden items (names starting with “.”, like <span class="mono">.DS_Store</span>) are never copied;
      this only shows how many were skipped.
    </p>
  </div>

  <div class="option">
    <label class="check">
      <input
        type="checkbox"
        checked={settings.reportNextToChecksum}
        disabled={!settings.writeChecksumFile}
        onchange={(e) => toggle("reportNextToChecksum", e.currentTarget.checked)}
      />
      Also save the job report next to the checksum file
    </label>
    <p class="help">Every job's report is also kept in the app; this adds a copy next to the checksum file.</p>
  </div>

  {#if settingsError}<p class="danger" role="alert">{settingsError}</p>{/if}
</section>

<style>
  .bar {
    margin: -6px 0 8px;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
    margin-bottom: var(--gap);
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 6px;
  }

  h2 {
    margin: 0;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .saved {
    color: var(--success);
    font-size: 12px;
  }

  .option {
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }

  .option:last-of-type {
    border-bottom: none;
  }

  .check {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .help {
    margin: 4px 0 0 24px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .danger {
    color: var(--danger);
  }
</style>
