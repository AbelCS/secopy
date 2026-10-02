<script lang="ts">
  // One mirror preset's fields (FR-44): its name, origin and destination, what happens to
  // files deleted in the origin, and the deep check. Problems show next to their field.
  import IgnoreList from "../lib/ui/IgnoreList.svelte";
  import { t } from "../lib/i18n";
  import { fieldOf } from "../lib/message";
  import { messageOf } from "../lib/format";
  import { useApi } from "../lib/api";
  import type { DeletedMode, MirrorPreset, MirrorPresetInput } from "../lib/bindings";
  import Button from "../lib/ui/Button.svelte";
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
    ignore: preset?.ignore ?? [],
  };
  let name = $state(start.name);
  let origin = $state(start.origin);
  let destination = $state(start.destination);
  let mode: DeletedMode = $state(start.mode);
  let days = $state(start.days);
  let deepCheck = $state(start.deepCheck);
  let ignore = $state([...start.ignore]);
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
          ignore.join("\n") !== start.ignore.join("\n") ||
          (mode === "archive" && days !== start.days) ||
          deepCheck !== start.deepCheck,
  );
  $effect(() => {
    changed = isChanged;
    canSave = isChanged && !saving;
  });

  /** Days made shorter on a saved archive: the next run removes more of what's archived (#101). */
  const shorter = $derived.by(() => {
    const n = Number.parseInt(days, 10);
    return preset?.deleted.mode === "archive" && n > 0 && n < preset.deleted.days ? n : null;
  });

  /** Back to the saved preset. */
  export function revert() {
    name = start.name;
    origin = start.origin;
    destination = start.destination;
    mode = start.mode;
    days = start.days;
    deepCheck = start.deepCheck;
    ignore = [...start.ignore];
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
        ignore,
      });
    } catch (e) {
      const message = messageOf(e);
      const field = fieldOf(e);
      if (field === "origin") originProblem = message;
      else if (field === "destination") destinationProblem = message;
      else if (field === "name") nameProblem = message;
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
        {#if shorter !== null}<p class="note">{t("mirror.editor.daysShorter", { count: shorter })}</p>{/if}
      </div>
    {:else}
      <p class="muted">{t("mirror.editor.deleteNote")}</p>
    {/if}
  </FormRow>

  <FormRow label={t("mirror.editor.comparison")}>
    <RadioGroup
      legend={t("mirror.editor.comparison")}
      hideLegend
      options={[
        { value: false, label: t("mirror.editor.standard"), help: t("mirror.editor.standardHelp") },
        {
          value: true,
          label: t("mirror.editor.paranoid"),
          help: t("mirror.editor.paranoidHelp"),
          warning: t("mirror.editor.paranoidWarning"),
        },
      ]}
      value={deepCheck}
      onChange={(v) => (deepCheck = v)}
    />
    <p class="help">{t("mirror.editor.verifyNote")}</p>
  </FormRow>

  <FormRow label={t("copy.alsoIgnore")} hint={t("mirror.editor.alsoIgnoreHint")}>
    <IgnoreList label={t("copy.alsoIgnore")} patterns={ignore} rows={4} onChange={(list) => (ignore = list)} />
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

  .help {
    margin: var(--space-2) 0 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .muted {
    color: var(--text-muted);
    font-size: var(--text-sm);
    margin: var(--space-2) 0 0;
  }
</style>
