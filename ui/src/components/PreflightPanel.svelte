<script lang="ts">
  // What pre-flight found (FR-16..FR-18, RFD §5.2): what blocks Start, what will fail, what's
  // already there, and the choice for files that differ.
  import type { ConflictPolicy, DestinationView, PlanView } from "../lib/bindings";
  import { formatCount, plural } from "../lib/format";
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
    { value: "keepBoth", label: "Keep both" },
    { value: "overwrite", label: "Overwrite" },
    { value: "skip", label: "Skip" },
  ];
</script>

{#if blocker}
  <Notice tone="danger">{blocker}</Notice>
{:else}
  <FormRow label="Files go to">
    <p class="mono">{destination.copyRoot}</p>
    {#if destination.existingItems}
      <Notice tone="warning">
        Already contains {plural(destination.existingItems, "item")}. Identical files will be skipped.
      </Notice>
    {/if}
  </FormRow>
  {#if destination.problemCount > 0}
    <FormRow label="Problems">
      <details class="failing">
        <summary>{plural(destination.problemCount, "file")} will fail</summary>
        <ul>
          {#each destination.problems as p (p.path)}
            <li><span class="mono">{p.path}</span>: {p.reason}</li>
          {/each}
          {#if destination.problemCount > destination.problems.length}
            <li class="muted">and {formatCount(destination.problemCount - destination.problems.length)} more</li>
          {/if}
        </ul>
      </details>
    </FormRow>
  {/if}
  {#if destination.identical > 0}
    <FormRow
      label="Identical"
      hint="Files with the same name, size and date (within 2 seconds) as one already in the destination. They aren't copied or read."
    >
      <p>{plural(destination.identical, "identical file")} will be skipped (not checked).</p>
    </FormRow>
  {/if}
  {#if destination.differs > 0}
    <FormRow
      label="Existing files"
      hint="Files with the same name as one already in the destination, but a different size or date. Keep both: the new one is copied with a number added to its name. Overwrite: the one there is replaced. Skip: the one there is left as it is."
    >
      <RadioGroup
        legend="{plural(destination.differs, 'file')} {destination.differs === 1 ? 'differs' : 'differ'} from what's there"
        options={choices}
        value={conflicts}
        onChange={onConflicts}
      />
    </FormRow>
  {/if}
  {#if destination.stalePartials > 0}
    <FormRow label="Leftovers">
      <p class="muted">
        {plural(destination.stalePartials, "unfinished file")} from an interrupted copy will be replaced.
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
