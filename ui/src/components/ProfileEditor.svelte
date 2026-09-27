<script lang="ts">
  // One source profile's fields (FR-38): its name, where the clips are on the card, whether
  // that folder itself is copied, and which file types. Problems show next to their field.
  import { useApi } from "../lib/api";
  import type { Profile, ProfileInput } from "../lib/bindings";
  import { relativeToDrive } from "../lib/drive";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Chip from "../lib/ui/Chip.svelte";
  import FormRow from "../lib/ui/FormRow.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import RadioGroup from "../lib/ui/RadioGroup.svelte";
  import TextField from "../lib/ui/TextField.svelte";

  let {
    profile,
    formId,
    onSave,
    changed = $bindable(false),
    canSave = $bindable(false),
  }: {
    /** `null` for a new profile. */
    profile: Profile | null;
    /** The action bar's Save submits this form. */
    formId: string;
    /** Throws the app's message when the profile can't be saved. */
    onSave: (input: ProfileInput) => Promise<void>;
    /** There are unsaved changes (so leaving asks, and Revert is on). */
    changed?: boolean;
    canSave?: boolean;
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
  const isChanged = $derived(
    profile === null
      ? name.trim() !== ""
      : name !== start.name ||
          folder !== start.folder ||
          includeFolder !== start.includeFolder ||
          all !== start.all ||
          (!all && types.join("\n") !== start.types.join("\n")),
  );
  const noTypes = $derived(!all && types.length === 0);
  $effect(() => {
    changed = isChanged;
    canSave = isChanged && !noTypes && !saving;
  });

  /** Back to the saved profile. */
  export function revert() {
    name = start.name;
    folder = start.folder;
    includeFolder = start.includeFolder;
    all = start.all;
    types = [...start.types];
    newType = "";
    nameProblem = folderProblem = otherProblem = null;
  }

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
      folderProblem = "Choose a directory on a card or drive.";
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
      if (message.startsWith("The directory")) folderProblem = message;
      else if (/name|profile called/.test(message)) nameProblem = message;
      else otherProblem = message;
    } finally {
      saving = false;
    }
  }
</script>

<form id={formId} class="editor" onsubmit={save}>
  <FormRow label="Name">
    <TextField label="Name" hideLabel bind:value={name} error={nameProblem} placeholder="e.g. Sony FX3" />
  </FormRow>

  <FormRow label="Directory">
    <TextField
      label="Directory on the card"
      hideLabel
      bind:value={folder}
      error={folderProblem}
      mono
      placeholder="empty = the directory or card you pick"
    />
    {#snippet aside()}<Button onclick={chooseFolder}>Choose…</Button>{/snippet}
  </FormRow>

  <FormRow label="Options">
    <Checkbox
      label={folderName ? `Include the “${folderName}” directory` : "Include the picked directory itself"}
      checked={includeFolder}
      onChange={(on) => (includeFolder = on)}
    />
  </FormRow>

  <FormRow label="File types">
    <RadioGroup
      legend="File types"
      hideLegend
      options={[
        { value: true, label: "All types" },
        { value: false, label: "Only these" },
      ]}
      value={all}
      onChange={(v) => (all = v)}
    />
    {#if !all}
      <div class="chips">
        {#each types as key (key)}
          <Chip label={label(key)} onRemove={() => (types = types.filter((k) => k !== key))} />
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
  </FormRow>

  {#if otherProblem}<Notice tone="danger">{otherProblem}</Notice>{/if}
</form>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
  }

  .add {
    width: 120px;
    min-height: 26px;
  }

  .muted {
    color: var(--text-muted);
    font-size: var(--text-sm);
    margin: 0;
  }
</style>
