<script lang="ts">
  // What pre-flight found (FR-16..FR-18, RFD §5.2): what blocks Start, what will fail, what's
  // already there, and the choice for files that differ.
  import { t } from "../lib/i18n";
  import { say } from "../lib/message";
  import type { ConflictPolicy, DestinationView, PlanView } from "../lib/bindings";
  import FormRow from "../lib/ui/FormRow.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import RadioGroup from "../lib/ui/RadioGroup.svelte";

  let {
    destination,
    plan,
    conflicts,
    onConflicts,
  }: {
    destination: DestinationView;
    plan: PlanView | null;
    conflicts: ConflictPolicy;
    onConflicts: (policy: ConflictPolicy) => void;
  } = $props();

  const blocker = $derived(destination.blocker ?? plan?.blocker ?? null);
  const choices: { value: ConflictPolicy; label: string }[] = [
    { value: "keepBoth", label: t("copy.preflight.keepBoth") },
    { value: "overwrite", label: t("copy.preflight.overwrite") },
    { value: "skip", label: t("copy.preflight.skip") },
  ];
</script>

<FormRow label={t("copy.preflight.filesGoTo")}>
  <p class="mono">{destination.copyRoot}</p>
  {#if destination.existingItems}
    <Notice tone="warning">
      {t("copy.preflight.notEmpty", { count: destination.existingItems })}
    </Notice>
  {/if}
</FormRow>
{#if blocker}
  <Notice tone="danger">{say(blocker)}</Notice>
{:else}
  {#if destination.problemCount > 0}
    <FormRow label={t("copy.preflight.problems")}>
      <details class="failing">
        <summary>{t("copy.preflight.willFail", { count: destination.problemCount })}</summary>
        <ul>
          {#each destination.problems as p (p.path)}
            <li><span class="mono">{p.path}</span>: {say(p.reason)}</li>
          {/each}
          {#if destination.problemCount > destination.problems.length}
            <li class="muted">{t("copy.preflight.more", { count: destination.problemCount - destination.problems.length })}</li>
          {/if}
        </ul>
      </details>
    </FormRow>
  {/if}
  {#if destination.identical > 0}
    <FormRow
      label={t("copy.preflight.identical")}
      hint={t("copy.preflight.identicalHint")}
    >
      <p>{t("copy.preflight.identicalSkipped", { count: destination.identical })}</p>
    </FormRow>
  {/if}
  {#if destination.differs > 0}
    <FormRow
      label={t("copy.preflight.existing")}
      hint={t("copy.preflight.existingHint")}
    >
      <RadioGroup
        legend={t("copy.preflight.differ", { count: destination.differs })}
        options={choices}
        value={conflicts}
        onChange={onConflicts}
      />
    </FormRow>
  {/if}
  {#if destination.stalePartials > 0}
    <FormRow label={t("copy.preflight.leftovers")}>
      <p class="muted">
        {t("copy.preflight.stale", { count: destination.stalePartials })}
      </p>
    </FormRow>
  {/if}
{/if}

<style>
  p {
    margin: 0;
  }

  .muted {
    color: var(--text-muted);
  }

  .failing summary {
    color: var(--danger);
    cursor: pointer;
  }
</style>
