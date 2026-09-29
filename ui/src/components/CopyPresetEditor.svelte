<script lang="ts">
  // One copy preset's fields (FR-38): its name, the source it loads, whether that directory itself
  // is copied, and which file types. Problems show next to their field.
  import { t } from "../lib/i18n";
  import { baseName, messageOf } from "../lib/format";
  import { useApi } from "../lib/api";
  import type { CopyPreset, CopyPresetInput } from "../lib/bindings";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Chip from "../lib/ui/Chip.svelte";
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
    /** `null` for a new preset. */
    preset: CopyPreset | null;
    /** The action bar's Save submits this form. */
    formId: string;
    /** Throws the app's message when the preset can't be saved. */
    onSave: (input: CopyPresetInput) => Promise<void>;
    /** There are unsaved changes (so leaving asks, and Revert is on). */
    changed?: boolean;
    canSave?: boolean;
  } = $props();

  const api = useApi();

  // The editor is recreated for each preset, so these start from it once.
  // svelte-ignore state_referenced_locally
  const start = {
    name: preset?.name ?? "",
    source: preset?.source ?? "",
    includeFolder: preset?.includeFolder ?? true,
    all: preset ? preset.extensions === null : true,
    types: preset?.extensions ?? [],
  };
  let name = $state(start.name);
  let source = $state(start.source);
  let includeFolder = $state(start.includeFolder);
  let all = $state(start.all);
  let types: (string | null)[] = $state([...start.types]);
  let newType = $state("");
  let saving = $state(false);
  let nameProblem: string | null = $state(null);
  let sourceProblem: string | null = $state(null);
  let otherProblem: string | null = $state(null);

  const folderName = $derived(baseName(source));
  const isChanged = $derived(
    preset === null
      ? name.trim() !== ""
      : name !== start.name ||
          source !== start.source ||
          includeFolder !== start.includeFolder ||
          all !== start.all ||
          (!all && types.join("\n") !== start.types.join("\n")),
  );
  const noTypes = $derived(!all && types.length === 0);
  $effect(() => {
    changed = isChanged;
    canSave = isChanged && !noTypes && !saving;
  });

  /** Back to the saved preset. */
  export function revert() {
    name = start.name;
    source = start.source;
    includeFolder = start.includeFolder;
    all = start.all;
    types = [...start.types];
    newType = "";
    nameProblem = sourceProblem = otherProblem = null;
  }

  const label = (key: string | null) => (key === null ? t("presets.editor.noExtension") : `.${key}`);

  /**
   * Adds what was typed: several types split by commas or spaces ("mp4, mov"), each without
   * its dots ("*.MP4" is mp4). `*` or `*.*` alone means All types.
   */
  function addType() {
    const typed = newType.trim();
    newType = "";
    if (typed === "*" || typed === "*.*") {
      all = true;
      return;
    }
    const keys = typed.split(/[\s,]+/).map((t) => t.replace(/^\*?\.+/, "").toLowerCase());
    for (const key of keys) if (key && key !== "*" && !types.includes(key)) types = [...types, key];
  }

  async function chooseSource() {
    const path = await api.pickDirectory(t("presets.editor.pickSource"));
    if (path === null) return;
    source = path;
    sourceProblem = null;
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    nameProblem = sourceProblem = otherProblem = null;
    try {
      await onSave({ name, source, includeFolder, extensions: all ? null : types });
    } catch (e) {
      const message = messageOf(e);
      if (message.startsWith("The source")) sourceProblem = message;
      else if (/name|preset called/.test(message)) nameProblem = message;
      else otherProblem = message;
    } finally {
      saving = false;
    }
  }
</script>

<form id={formId} class="editor" onsubmit={save}>
  <FormRow label={t("presets.editor.name")}>
    <TextField
      label={t("presets.editor.name")}
      hideLabel
      bind:value={name}
      error={nameProblem}
      placeholder={t("presets.namePlaceholder")}
    />
  </FormRow>

  <FormRow label={t("presets.editor.source")}>
    <TextField
      label={t("presets.editor.source")}
      hideLabel
      bind:value={source}
      error={sourceProblem}
      mono
      placeholder={t("presets.editor.sourcePlaceholder")}
    />
    {#snippet aside()}<Button onclick={chooseSource}>{t("ui.choose")}</Button>{/snippet}
  </FormRow>

  <FormRow label={t("presets.editor.options")}>
    <Checkbox
      label={folderName
        ? t("presets.editor.includeNamed", { name: folderName })
        : t("presets.editor.includePicked")}
      checked={includeFolder}
      onChange={(on) => (includeFolder = on)}
    />
  </FormRow>

  <FormRow label={t("presets.editor.fileTypes")}>
    <RadioGroup
      legend={t("presets.editor.fileTypes")}
      hideLegend
      options={[
        { value: true, label: t("presets.editor.allTypes") },
        { value: false, label: t("presets.editor.onlyThese") },
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
          aria-label={t("presets.editor.addType")}
          placeholder={t("presets.editor.addTypePlaceholder")}
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
      {#if noTypes}<p class="muted">{t("presets.editor.atLeastOne")}</p>{/if}
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
