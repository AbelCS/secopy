<script lang="ts">
  // A list of name patterns to ignore (#158, #161, #164): the field and Add, one pattern per row
  // in a box that scrolls, each with its remove button, and how many. Settings' list has
  // Restore defaults; a job's list knows Settings' to refuse repeats.
  import { t } from "../i18n";
  import { MAX_LEN, MAX_PATTERNS, patternProblem } from "../patterns";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import TextField from "./TextField.svelte";

  let {
    label,
    patterns,
    onChange,
    global = [],
    rows = 8,
    onRestore,
  }: {
    /** Names the list for screen readers. */
    label: string;
    patterns: string[];
    onChange: (list: string[]) => void;
    /** Settings' list, when this is a job's: a pattern already there isn't added. */
    global?: string[];
    /** Rows shown before the list scrolls. */
    rows?: number;
    /** Restore defaults (Settings' list only). */
    onRestore?: () => void;
  } = $props();

  let pattern = $state("");
  let error: string | null = $state(null);

  function add() {
    const problem = patternProblem(pattern, patterns, global);
    if (problem === "slash") error = t("errors.pattern.slash");
    else if (problem === "tooLong") error = t("errors.pattern.tooLong", { max: MAX_LEN });
    else if (problem === "tooMany") error = t("errors.pattern.tooMany", { max: MAX_PATTERNS });
    else if (problem === "badChar") error = t("errors.pattern.badChar");
    else if (problem === "repeat") error = t("settings.ignore.repeat");
    else if (problem === "inGlobal") error = t("settings.ignore.inGlobal");
    else {
      const p = pattern.replace(/^ +| +$/g, "");
      if (p) onChange([...patterns, p]);
      pattern = "";
      error = null;
    }
  }
</script>

<!-- Not a form: it sits inside the preset editors' forms. Return adds, as Add does. -->
<div class="add">
  <TextField
    label={t("settings.ignore.field")}
    hideLabel
    mono
    placeholder={t("settings.ignore.placeholder")}
    bind:value={pattern}
    {error}
    onkeydown={(e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        add();
      }
    }}
  />
  <Button onclick={add}>{t("settings.ignore.add")}</Button>
</div>
{#if patterns.length > 0}
  <ul class="patterns" aria-label={label} translate="no" style="--rows: {rows}">
    {#each patterns as p (p)}
      <li>
        <span class="mono">{p}</span>
        <button
          type="button"
          class="remove"
          aria-label={t("ui.remove", { name: p })}
          onclick={() => onChange(patterns.filter((q) => q !== p))}><Icon name="x" size={12} /></button
        >
      </li>
    {/each}
  </ul>
{:else if onRestore}
  <p class="empty">{t("settings.ignore.empty")}</p>
{/if}
{#if patterns.length > 0 || onRestore}
  <div class="foot">
    <span class="count">{t("settings.ignore.count", { count: patterns.length })}</span>
    {#if onRestore}<Button onclick={onRestore}>{t("settings.ignore.restore")}</Button>{/if}
  </div>
{/if}

<style>
  .add {
    display: flex;
    gap: var(--space-2);
    align-items: flex-start;
  }

  .add > :global(:first-child) {
    flex: 1;
  }

  /* `--rows` rows, then it scrolls. */
  .patterns {
    list-style: none;
    margin: var(--space-2) 0 0;
    padding: var(--space-1) 0;
    max-height: calc(var(--rows) * 1.9rem + 2 * var(--space-1));
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    background: var(--surface);
  }

  .patterns li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    justify-content: space-between;
    min-height: 1.9rem;
    padding: 0 var(--space-2) 0 var(--space-3);
  }

  .patterns li:hover,
  .patterns li:focus-within {
    background: var(--surface-raised);
  }

  /* A long pattern wraps: its remove button stays in view (up to 255 characters). */
  .patterns li span {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .remove {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 1.4rem;
    height: 1.4rem;
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
  }

  .patterns li:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }

  .remove:hover {
    color: var(--text);
  }

  .empty {
    margin: var(--space-2) 0 0;
    color: var(--text-muted);
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: var(--space-2);
  }

  .count {
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
</style>
