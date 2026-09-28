<script lang="ts">
  // One mirror preset's fields (FR-44): its name, origin and destination, what happens to
  // files deleted in the origin, and the deep check. Problems show next to their field.
  import { useApi } from "../lib/api";
  import type { DeletedMode, MirrorPreset, MirrorPresetInput } from "../lib/bindings";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import FormRow from "../lib/ui/FormRow.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import RadioGroup from "../lib/ui/RadioGroup.svelte";
  import TextField from "../lib/ui/TextField.svelte";

  let {
    preset,
    formId,
    onSave,
    changed = $bindable(false),
    canSave = $bindable(false),
  }: {
    /** `null` for a new mirror. */
    preset: MirrorPreset | null;
    /** The action bar's Save submits this form. */
    formId: string;
    /** Throws the app's message when the preset can't be saved. */
    onSave: (input: MirrorPresetInput) => Promise<void>;
    /** There are unsaved changes. */
    changed?: boolean;
    canSave?: boolean;
  } = $props();

  const api = useApi();

  // The editor is recreated for each preset, so these start from it once.
  // svelte-ignore state_referenced_locally
  const start = {
    name: preset?.name ?? "",
    origin: preset?.origin ?? "",
    destination: preset?.destination ?? "",
    mode: preset?.deleted.mode ?? ("archive" as DeletedMode),
    days: String(preset?.deleted.days ?? 30),
    deepCheck: preset?.deepCheck ?? false,
  };
  let name = $state(start.name);
  let origin = $state(start.origin);
  let destination = $state(start.destination);
  let mode: DeletedMode = $state(start.mode);
  let days = $state(start.days);
  let deepCheck = $state(start.deepCheck);
  let saving = $state(false);
  let nameProblem: string | null = $state(null);
  let originProblem: string | null = $state(null);
  let destinationProblem: string | null = $state(null);
  let otherProblem: string | null = $state(null);

  const isChanged = $derived(
    preset === null
      ? name.trim() !== ""
      : name !== start.name ||
          origin !== start.origin ||
          destination !== start.destination ||
          mode !== start.mode ||
          (mode === "archive" && days !== start.days) ||
          deepCheck !== start.deepCheck,
  );
  $effect(() => {
    changed = isChanged;
    canSave = isChanged && !saving;
  });

  /** Back to the saved preset. */
  export function revert() {
    name = start.name;
    origin = start.origin;
    destination = start.destination;
    mode = start.mode;
    days = start.days;
    deepCheck = start.deepCheck;
    nameProblem = originProblem = destinationProblem = otherProblem = null;
  }

  async function choose(which: "origin" | "destination") {
    const path = await api.pickDirectory();
    if (path === null) return;
    if (which === "origin") origin = path;
    else destination = path;
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    nameProblem = originProblem = destinationProblem = otherProblem = null;
    try {
      await onSave({
        name,
        origin,
        destination,
        deleted: { mode, days: Number.parseInt(days, 10) || 0 },
        deepCheck,
      });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      if (message.startsWith("The origin")) originProblem = message;
      else if (message.startsWith("The destination")) destinationProblem = message;
      else if (/name|mirror called/.test(message)) nameProblem = message;
      else otherProblem = message;
    } finally {
      saving = false;
    }
  }
</script>

<form id={formId} class="editor" onsubmit={save}>
  <FormRow label="Name">
    <TextField label="Name" hideLabel bind:value={name} error={nameProblem} placeholder="e.g. Footage → NAS" />
  </FormRow>

  <FormRow label="Origin">
    <TextField
      label="Origin"
      hideLabel
      bind:value={origin}
      error={originProblem}
      mono
      placeholder="e.g. /Volumes/SSD/Footage"
    />
    {#snippet aside()}<Button onclick={() => choose("origin")}>Choose…</Button>{/snippet}
  </FormRow>

  <FormRow label="Destination">
    <TextField
      label="Destination"
      hideLabel
      bind:value={destination}
      error={destinationProblem}
      mono
      placeholder="e.g. /Volumes/Media/Footage"
    />
    {#snippet aside()}<Button onclick={() => choose("destination")}>Choose…</Button>{/snippet}
  </FormRow>

  <FormRow label="Deleted files">
    <RadioGroup
      legend="Files deleted in the origin"
      hideLegend
      options={[
        { value: "archive" as DeletedMode, label: "Archive them" },
        { value: "delete" as DeletedMode, label: "Delete them" },
      ]}
      value={mode}
      onChange={(v) => (mode = v)}
    />
    {#if mode === "archive"}
      <div class="days">
        <TextField
          label="Days to keep"
          type="number"
          min="1"
          bind:value={days}
          help="In a hidden .secopy-archive directory on the destination, then removed."
        />
      </div>
    {:else}
      <p class="muted">Files deleted in the origin are deleted in the destination too. This can't be undone.</p>
    {/if}
  </FormRow>

  <FormRow label="Checking">
    <p class="note">
      New and changed files are always verified after copying: read back from the destination and compared by
      checksum.
    </p>
    <Checkbox
      label="Also compare unchanged files byte for byte"
      checked={deepCheck}
      onChange={(on) => (deepCheck = on)}
    >
      {#snippet help()}
        Files with the same size and date are normally left alone. This reads both copies in full to catch a
        damaged or silently changed file in the backup. Slow on big libraries.
      {/snippet}
    </Checkbox>
  </FormRow>

  {#if otherProblem}<Notice tone="danger">{otherProblem}</Notice>{/if}
</form>

<style>
  .days {
    max-width: 420px;
    margin-top: var(--space-2);
  }

  .days :global(input) {
    max-width: 96px;
  }

  .note {
    margin: 0 0 var(--space-2);
  }

  .muted {
    color: var(--text-muted);
    font-size: var(--text-sm);
    margin: var(--space-2) 0 0;
  }
</style>
