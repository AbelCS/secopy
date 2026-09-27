<script lang="ts">
  // FROM's source profile (FR-38): pick one, see when this run differs from it, and save
  // the difference with Update profile or Save as new….
  import { useApi } from "../lib/api";
  import type { Profile, ProfilesView, SessionView } from "../lib/bindings";
  import Button from "../lib/ui/Button.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import Select from "../lib/ui/Select.svelte";
  import TextField from "../lib/ui/TextField.svelte";

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

  const options = $derived([
    { value: "", label: "None" },
    ...profiles.map((p) => ({ value: p.id, label: p.name })),
    { value: MANAGE, label: "Manage profiles…" },
  ]);

  function choose(value: string, menu: HTMLSelectElement) {
    if (value === MANAGE) {
      menu.value = view.profileId ?? "";
      onManage();
      return;
    }
    onSelect(value === "" ? null : value);
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
  <Select label="Profile" hideLabel value={view.profileId ?? ""} {options} disabled={forFiles} onChange={choose} />
  {#if selected && view.profileChanged}
    <span class="muted">Changed for this run</span>
    <Button disabled={busy} onclick={() => act(() => api.updateProfile())}>Update profile</Button>
  {/if}
  {#if canSaveAs}
    <Button disabled={busy} onclick={openSaveAs}>Save as new…</Button>
  {/if}
</div>
{#if savingAs}
  <form class="save-as" onsubmit={saveAs}>
    <TextField label="Name" bind:value={name} placeholder="e.g. Sony FX3" />
    <TextField label="Directory on the card" bind:value={folder} mono placeholder="e.g. PRIVATE/M4ROOT/CLIP" />
    <div class="buttons">
      <Button onclick={() => (savingAs = false)}>Cancel</Button>
      <Button variant="primary" type="submit" disabled={busy}>Save</Button>
    </div>
  </form>
{/if}
{#if error}<Notice tone="danger">{error}</Notice>{/if}

<style>
  .profile {
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
