<script lang="ts">
  // A source profile's fields (FR-38), for Settings → New profile and Edit.
  import type { Profile, ProfileInput } from "../lib/bindings";

  let {
    profile,
    onSave,
    onCancel,
  }: {
    /** `null` for a new profile. */
    profile: Profile | null;
    onSave: (input: ProfileInput) => void;
    onCancel: () => void;
  } = $props();

  const NO_EXTENSION = "(no extension)";
  // svelte-ignore state_referenced_locally
  let name = $state(profile?.name ?? "");
  // svelte-ignore state_referenced_locally
  let folder = $state(profile?.folder ?? "");
  // svelte-ignore state_referenced_locally
  let includeFolder = $state(profile?.includeFolder ?? true);
  // svelte-ignore state_referenced_locally
  let types = $state(profile?.extensions?.map((k) => k ?? NO_EXTENSION).join(", ") ?? "");

  /** "mp4, xml" → ["mp4", "xml"]; empty = every file type. The app normalizes the rest. */
  function extensions(): (string | null)[] | null {
    const list = types
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);
    return list.length === 0 ? null : list.map((t) => (t === NO_EXTENSION ? null : t));
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    onSave({ name, folder, includeFolder, extensions: extensions() });
  }
</script>

<form class="profile-form" onsubmit={submit}>
  <label>Name <input bind:value={name} /></label>
  <label>Folder on the card <input bind:value={folder} placeholder="e.g. PRIVATE/M4ROOT/CLIP (empty = what you pick)" /></label>
  <label class="check"><input type="checkbox" bind:checked={includeFolder} /> Include the folder itself</label>
  <label>File types (comma-separated; empty = all) <input bind:value={types} placeholder="e.g. mp4, xml" /></label>
  <div class="actions">
    <button type="submit" class="primary">Save</button>
    <button type="button" onclick={onCancel}>Cancel</button>
  </div>
</form>

<style>
  .profile-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 8px 0;
  }

  .actions {
    display: flex;
    gap: 8px;
  }
</style>
