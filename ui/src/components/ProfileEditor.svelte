<script lang="ts">
  // One source profile's fields (FR-38): its name, where the clips are on the card, whether
  // that folder itself is copied, and which file types. Problems show next to their field.
  import { useApi } from "../lib/api";
  import type { Profile, ProfileInput } from "../lib/bindings";
  import { relativeToDrive } from "../lib/drive";

  let {
    profile,
    onSave,
    onDelete,
  }: {
    /** `null` for a new profile. */
    profile: Profile | null;
    /** Throws the app's message when the profile can't be saved. */
    onSave: (input: ProfileInput) => Promise<void>;
    onDelete: (() => void) | null;
  } = $props();

  const api = useApi();
  const NO_EXTENSION = "(no extension)";

  // The editor is recreated for each profile, so these start from it once.
  // svelte-ignore state_referenced_locally
  const start = {
    name: profile?.name ?? "",
    folder: profile?.folder ?? "",
    includeFolder: profile?.includeFolder ?? true,
    all: profile ? profile.extensions === null : true,
    types: profile?.extensions ?? [],
  };
  let name = $state(start.name);
  let folder = $state(start.folder);
  let includeFolder = $state(start.includeFolder);
  let all = $state(start.all);
  let types: (string | null)[] = $state([...start.types]);
  let newType = $state("");
  let saving = $state(false);
  let nameProblem: string | null = $state(null);
  let folderProblem: string | null = $state(null);
  let otherProblem: string | null = $state(null);

  const folderName = $derived(folder.split("/").filter(Boolean).pop() ?? "");
  const changed = $derived(
    profile === null
      ? name.trim() !== ""
      : name !== start.name ||
          folder !== start.folder ||
          includeFolder !== start.includeFolder ||
          all !== start.all ||
          (!all && types.join("\n") !== start.types.join("\n")),
  );
  const noTypes = $derived(!all && types.length === 0);
  const canSave = $derived(changed && !noTypes && !saving);

  const label = (key: string | null) => (key === null ? NO_EXTENSION : `.${key}`);

  function addType() {
    const key = newType.trim().replace(/^\.+/, "").toLowerCase();
    newType = "";
    if (key && !types.includes(key)) types = [...types, key];
  }

  async function chooseFolder() {
    const path = await api.pickCardFolder();
    if (path === null) return;
    const relative = relativeToDrive(path);
    if (relative === null) {
      folderProblem = "Choose a folder on a card or drive.";
    } else {
      folder = relative;
      folderProblem = null;
    }
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    nameProblem = folderProblem = otherProblem = null;
    try {
      await onSave({ name, folder, includeFolder, extensions: all ? null : types });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      if (message.startsWith("The folder")) folderProblem = message;
      else if (/name|profile called/.test(message)) nameProblem = message;
      else otherProblem = message;
    } finally {
      saving = false;
    }
  }
</script>

<form class="editor" onsubmit={save}>
  <label class="field">
    <span>Name</span>
    <input bind:value={name} aria-describedby="name-problem" placeholder="e.g. Sony FX3" />
  </label>
  {#if nameProblem}<p id="name-problem" class="danger" role="alert">{nameProblem}</p>{/if}

  <div class="field">
    <label for="profile-folder">Folder on the card</label>
    <div class="row">
      <input
        id="profile-folder"
        class="mono"
        bind:value={folder}
        aria-describedby="folder-problem"
        placeholder="empty = the folder or card you pick"
      />
      <button type="button" onclick={chooseFolder}>Choose…</button>
    </div>
  </div>
  {#if folderProblem}<p id="folder-problem" class="danger" role="alert">{folderProblem}</p>{/if}

  <label class="check">
    <input type="checkbox" bind:checked={includeFolder} />
    {#if folderName}Include the “{folderName}” folder{:else}Include the picked folder itself{/if}
  </label>

  <fieldset class="field">
    <legend>File types</legend>
    <div class="row">
      <label class="check"><input type="radio" bind:group={all} value={true} /> All types</label>
      <label class="check"><input type="radio" bind:group={all} value={false} /> Only these</label>
    </div>
    {#if !all}
      <div class="chips">
        {#each types as key (key)}
          <span class="chip">
            {label(key)}
            <button
              type="button"
              class="remove"
              aria-label={`Remove ${label(key)}`}
              onclick={() => (types = types.filter((k) => k !== key))}>×</button
            >
          </span>
        {/each}
        <input
          class="add"
          aria-label="Add a file type"
          placeholder="+ add type"
          bind:value={newType}
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addType();
            }
          }}
          onblur={addType}
        />
      </div>
      {#if noTypes}<p class="muted">Add at least one file type.</p>{/if}
    {/if}
  </fieldset>

  {#if otherProblem}<p class="danger" role="alert">{otherProblem}</p>{/if}
  <div class="actions">
    {#if onDelete}
      <button type="button" class="danger-button" onclick={onDelete}>Delete…</button>
    {/if}
    <span class="spacer"></span>
    <button type="submit" class="primary" disabled={!canSave}>Save</button>
  </div>
</form>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    border: none;
    padding: 0;
    margin: 0;
  }

  .field > span,
  .field > label,
  legend {
    font-size: 12px;
    color: var(--text-muted);
    padding: 0;
  }

  .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .row input {
    flex: 1;
  }

  .check {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    margin-top: 6px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px 2px 10px;
    border: 1px solid var(--accent);
    border-radius: 999px;
  }

  .remove {
    border: none;
    background: none;
    padding: 0 6px;
    color: var(--text-muted);
  }

  .add {
    width: 110px;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
  }

  .spacer {
    flex: 1;
  }

  .danger,
  .danger-button {
    color: var(--danger);
  }

  .danger {
    margin: -6px 0 0;
  }

  .muted {
    color: var(--text-muted);
    margin: 4px 0 0;
  }
</style>
