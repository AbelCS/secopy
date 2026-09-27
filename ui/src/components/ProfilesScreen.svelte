<script lang="ts">
  // Source profiles (FR-38): the list on the left, the selected one's editor on the right.
  import { useApi } from "../lib/api";
  import type { Profile, ProfileInput, SessionView } from "../lib/bindings";
  import ProfileEditor from "./ProfileEditor.svelte";

  let {
    profiles,
    onProfiles,
    onView,
    onDone,
  }: {
    profiles: Profile[];
    onProfiles: (profiles: Profile[]) => void;
    /** Editing or deleting the selected profile changes what FROM shows. */
    onView: (view: SessionView) => void;
    onDone: () => void;
  } = $props();

  const api = useApi();
  const NEW = "new";
  // svelte-ignore state_referenced_locally
  let selectedId: string | null = $state(profiles[0]?.id ?? null);
  let error: string | null = $state(null);

  const selected = $derived(profiles.find((p) => p.id === selectedId) ?? null);
  /** Recreates the editor for another profile, or after this one was saved. */
  const editorKey = $derived(selectedId === NEW ? NEW : JSON.stringify(selected));

  async function save(input: ProfileInput) {
    if (selectedId === NEW) {
      const list = await api.createProfile(input);
      onProfiles(list);
      const name = input.name.trim().toLowerCase();
      selectedId = list.find((p) => p.name.toLowerCase() === name)?.id ?? null;
    } else if (selected) {
      const result = await api.editProfile(selected.id, input);
      onProfiles(result.profiles);
      onView(result.session);
    }
  }

  async function remove(p: Profile) {
    const sure = await api.confirm(
      `The profile “${p.name}” is deleted. Cards and copies are not touched.`,
      "Delete profile?",
      "Delete",
      "Keep",
    );
    if (!sure) return;
    try {
      const result = await api.deleteProfile(p.id);
      onProfiles(result.profiles);
      onView(result.session);
      selectedId = result.profiles[0]?.id ?? null;
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<section class="card" aria-labelledby="profiles-title">
  <div class="head">
    <h2 id="profiles-title">Profiles</h2>
    <button type="button" class="primary" onclick={onDone}>Done</button>
  </div>
  <div class="panes">
    <nav class="list" aria-label="Profiles">
      {#each profiles as p (p.id)}
        <button
          type="button"
          class="item"
          class:on={p.id === selectedId}
          aria-label={p.name}
          aria-current={p.id === selectedId}
          onclick={() => (selectedId = p.id)}
        >
          <span class="name">{p.name}</span>
          <span class="muted mono">{p.folder || "(what you pick)"}</span>
        </button>
      {/each}
      <button type="button" class="new" class:on={selectedId === NEW} onclick={() => (selectedId = NEW)}>
        + New profile
      </button>
    </nav>
    <div class="detail">
      {#if selectedId === NEW || selected}
        {#key editorKey}
          <ProfileEditor
            profile={selectedId === NEW ? null : selected}
            onSave={save}
            onDelete={selected ? () => remove(selected) : null}
          />
        {/key}
      {:else}
        <p class="muted">
          A profile remembers where the clips are on a card of a given camera (for example
          <span class="mono">PRIVATE/M4ROOT/CLIP</span>), whether that folder itself is copied, and
          which file types. Pick it in the main window and a card is set up in one click.
        </p>
        <p class="muted">Create one here with “+ New profile”, or with “Save as new…” in the main window.</p>
      {/if}
      {#if error}<p class="danger" role="alert">{error}</p>{/if}
    </div>
  </div>
</section>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 14px 16px;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  h2 {
    margin: 0;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .panes {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 16px;
    min-height: 320px;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    border-right: 1px solid var(--border);
    padding-right: 12px;
  }

  .item,
  .new {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    text-align: left;
    border: 1px solid transparent;
    background: none;
    padding: 6px 8px;
  }

  .item.on,
  .new.on {
    border-color: var(--accent);
    background: var(--bg);
  }

  .item .muted {
    font-size: 11px;
  }

  .new {
    color: var(--accent);
    margin-top: 4px;
  }

  .muted {
    color: var(--text-muted);
  }

  .danger {
    color: var(--danger);
  }
</style>
