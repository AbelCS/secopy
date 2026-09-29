<script lang="ts">
  // One mirror preset's fields (FR-44): its name, origin and destination, what happens to
  // files deleted in the origin, and the deep check. Problems show next to their field.
  import { t } from "../lib/i18n";
  import { messageOf } from "../lib/format";
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
    const path = await api.pickDirectory(t(which === "origin" ? "mirror.editor.origin" : "mirror.editor.destination"));
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
      const message = messageOf(e);
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
  <FormRow label={t("mirror.editor.name")}>
    <TextField
      label={t("mirror.editor.name")}
      hideLabel
      bind:value={name}
      error={nameProblem}
      placeholder={t("mirror.editor.namePlaceholder")}
    />
  </FormRow>

  <FormRow label={t("mirror.editor.origin")}>
    <TextField
      label={t("mirror.editor.origin")}
      hideLabel
      bind:value={origin}
      error={originProblem}
      mono
      placeholder={t("mirror.editor.originPlaceholder")}
    />
    {#snippet aside()}<Button onclick={() => choose("origin")}>{t("ui.choose")}</Button>{/snippet}
  </FormRow>

  <FormRow label={t("mirror.editor.destination")}>
    <TextField
      label={t("mirror.editor.destination")}
      hideLabel
      bind:value={destination}
      error={destinationProblem}
      mono
      placeholder={t("mirror.editor.destinationPlaceholder")}
    />
    {#snippet aside()}<Button onclick={() => choose("destination")}>{t("ui.choose")}</Button>{/snippet}
  </FormRow>

  <FormRow label={t("mirror.editor.deleted")}>
    <RadioGroup
      legend={t("mirror.editor.deletedLegend")}
      hideLegend
      options={[
        { value: "archive" as DeletedMode, label: t("mirror.editor.archive") },
        { value: "delete" as DeletedMode, label: t("mirror.editor.delete") },
      ]}
      value={mode}
      onChange={(v) => (mode = v)}
    />
    {#if mode === "archive"}
      <div class="days">
        <TextField
          label={t("mirror.editor.days")}
          type="number"
          min="1"
          bind:value={days}
          help={t("mirror.editor.daysHelp")}
        />
      </div>
    {:else}
      <p class="muted">{t("mirror.editor.deleteNote")}</p>
    {/if}
  </FormRow>

  <FormRow label={t("mirror.editor.checking")}>
    <p class="note">{t("mirror.editor.verifyNote")}</p>
    <Checkbox
      label={t("mirror.editor.deepCheck")}
      checked={deepCheck}
      onChange={(on) => (deepCheck = on)}
    >
      {#snippet help()}
        {t("mirror.editor.deepCheckHelp")}
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
