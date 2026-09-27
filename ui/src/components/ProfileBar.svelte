<script lang="ts">
  // FROM's source profile (FR-38): pick one, see when this run differs from it, and save
  // the difference with Update profile or Save as new….
  import { useApi } from "../lib/api";
  import type { Profile, ProfilesView, SessionView } from "../lib/bindings";

  let {
    view,
    profiles,
    busy,
    onSelect,
    onApplied,
    onManage,
  }: {
    view: SessionView;
    profiles: Profile[];
    /** A scan is running: its result decides what Update / Save as new would save. */
    busy: boolean;
    onSelect: (id: string | null) => void;
    /** After Update profile / Save as new…: the profiles and the new view. */
    onApplied: (result: ProfilesView) => void;
    onManage: () => void;
  } = $props();

  const api = useApi();
  const MANAGE = "manage";
  let error: string | null = $state(null);
  let savingAs = $state(false);
  let name = $state("");
  let folder = $state("");

  const selected = $derived(profiles.find((p) => p.id === view.profileId) ?? null);
  /** Profiles apply to folders only (spec B4). */
  const forFiles = $derived(!!view.source && !view.source.isFolder);
  const canSaveAs = $derived(
    !!view.source?.isFolder && !view.source.isRetry && (!selected || view.profileChanged),
  );

  async function act(call: () => Promise<ProfilesView>) {
    try {
      onApplied(await call());
      error = null;
      savingAs = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function choose(menu: HTMLSelectElement) {
    if (menu.value === MANAGE) {
      menu.value = view.profileId ?? "";
      onManage();
      return;
    }
    onSelect(menu.value === "" ? null : menu.value);
  }

  function openSaveAs() {
    name = "";
    folder = view.suggestedFolder;
    error = null;
    savingAs = true;
  }

  function saveAs(event: SubmitEvent) {
    event.preventDefault();
    void act(() => api.saveProfileAs(name, folder));
  }
</script>

<div class="profile">
  <label>
    Profile
    <select value={view.profileId ?? ""} disabled={forFiles} onchange={(e) => choose(e.currentTarget)}>
      <option value="">None</option>
      {#each profiles as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
      <option value={MANAGE}>Manage profiles…</option>
    </select>
  </label>
  {#if selected && view.profileChanged}
    <span class="muted">{selected.name} · changed for this run</span>
    <button type="button" disabled={busy} onclick={() => act(() => api.updateProfile())}>Update profile</button>
  {/if}
  {#if canSaveAs}
    <button type="button" disabled={busy} onclick={openSaveAs}>Save as new…</button>
  {/if}
</div>
{#if savingAs}
  <form class="save-as" onsubmit={saveAs}>
    <label>Name <input bind:value={name} /></label>
    <label>Directory on the card <input bind:value={folder} placeholder="e.g. PRIVATE/M4ROOT/CLIP" /></label>
    <button type="submit" class="primary" disabled={busy}>Save</button>
    <button type="button" onclick={() => (savingAs = false)}>Cancel</button>
  </form>
{/if}
{#if error}<p class="danger" role="alert">{error}</p>{/if}

<style>
  .profile,
  .save-as {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
  }

  .muted {
    color: var(--text-muted);
  }

  .danger {
    color: var(--danger);
  }
</style>
