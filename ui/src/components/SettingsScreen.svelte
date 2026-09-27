<script lang="ts">
  // Settings (RFD §5.5): what every job does, and the source profiles (FR-38). Everything
  // applies at once; the next job uses it.
  import { useApi } from "../lib/api";
  import type { Profile, ProfileInput, ProfilesView, SessionView, Settings } from "../lib/bindings";
  import ProfileForm from "./ProfileForm.svelte";

  let {
    settings,
    profiles,
    onSettings,
    onProfiles,
    onView,
    onDone,
  }: {
    settings: Settings;
    profiles: Profile[];
    onSettings: (settings: Settings) => void;
    onProfiles: (profiles: Profile[]) => void;
    onView: (view: SessionView) => void;
    onDone: () => void;
  } = $props();

  const api = useApi();
  let settingsError: string | null = $state(null);
  let profileError: string | null = $state(null);
  /** The id of the profile being edited, "new", or nothing. */
  let editing: string | null = $state(null);
  const NEW = "new";

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

  async function act(call: () => Promise<void>) {
    try {
      await call();
      profileError = null;
      editing = null;
    } catch (e) {
      profileError = e instanceof Error ? e.message : String(e);
    }
  }

  function applied(result: ProfilesView) {
    onProfiles(result.profiles);
    onView(result.session);
  }

  function save(input: ProfileInput) {
    const id = editing;
    void act(async () => {
      if (id === NEW) onProfiles(await api.createProfile(input));
      else if (id) applied(await api.editProfile(id, input));
    });
  }

  async function remove(p: Profile) {
    const sure = await api.confirm(
      `The profile “${p.name}” is deleted. Cards and copies are not touched.`,
      "Delete profile?",
      "Delete",
      "Keep",
    );
    if (sure) await act(async () => applied(await api.deleteProfile(p.id)));
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

<section class="card" aria-labelledby="profiles-title">
  <h2 id="profiles-title">Source profiles</h2>
  {#if profiles.length === 0}
    <p class="muted">No profiles yet. Save one from the main window, or create one here.</p>
  {/if}
  <ul class="profiles">
    {#each profiles as p (p.id)}
      <li>
        <span><strong>{p.name}</strong> <span class="muted mono">{p.folder || "(what you pick)"}</span></span>
        <span class="actions">
          <button type="button" aria-label={`Edit ${p.name}`} onclick={() => (editing = p.id)}>Edit</button>
          <button type="button" aria-label={`Delete ${p.name}`} onclick={() => remove(p)}>Delete</button>
        </span>
      </li>
      {#if editing === p.id}
        <ProfileForm profile={p} onSave={save} onCancel={() => (editing = null)} />
      {/if}
    {/each}
  </ul>
  {#if editing === NEW}
    <ProfileForm profile={null} onSave={save} onCancel={() => (editing = null)} />
  {:else}
    <button type="button" onclick={() => (editing = NEW)}>New profile</button>
  {/if}
  {#if profileError}<p class="danger" role="alert">{profileError}</p>{/if}
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

  .profiles {
    list-style: none;
    padding: 0;
    margin: 0 0 8px;
  }

  .profiles li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 0;
  }

  .actions {
    display: flex;
    gap: 6px;
  }

  .muted {
    color: var(--text-muted);
  }

  .danger {
    color: var(--danger);
  }

  .done {
    display: flex;
    justify-content: flex-end;
  }
</style>
