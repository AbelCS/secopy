<script lang="ts">
  // What pre-flight found (FR-16..FR-18, RFD §5.2): what blocks Start, what will fail, what's
  // already there, and the choice for files that differ.
  import type { ConflictPolicy, DestinationView, PlanView } from "../lib/bindings";
  import { formatCount, plural } from "../lib/format";

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

<div class="preflight" aria-live="polite">
  {#if blocker}
    <p class="danger" role="alert">{blocker}</p>
  {:else}
    {#if destination.existingItems}
      <p class="warning">
        <span class="mono">{destination.copyRoot}</span> already contains
        {plural(destination.existingItems, "item")}. Identical files will be skipped.
      </p>
    {/if}
    {#if destination.problemCount > 0}
      <details class="danger-box">
        <summary>{plural(destination.problemCount, "file")} will fail</summary>
        <ul>
          {#each destination.problems as p (p.path)}
            <li><span class="mono">{p.path}</span>: {p.reason}</li>
          {/each}
          {#if destination.problemCount > destination.problems.length}
            <li class="muted">
              and {formatCount(destination.problemCount - destination.problems.length)} more
            </li>
          {/if}
        </ul>
      </details>
    {/if}
    {#if destination.identical > 0}
      <p>
        {plural(destination.identical, "identical file")} will be skipped (not checked).
      </p>
    {/if}
    {#if destination.differs > 0}
      <fieldset>
        <legend>
          {plural(destination.differs, "file")}
          {destination.differs === 1 ? "differs" : "differ"} from what's there
        </legend>
        {#each choices as c (c.value)}
          <label>
            <input
              type="radio"
              name="conflicts"
              value={c.value}
              checked={conflicts === c.value}
              onchange={() => onConflicts(c.value)}
            />
            {c.label}
          </label>
        {/each}
      </fieldset>
    {/if}
    {#if destination.stalePartials > 0}
      <p class="muted">
        {plural(destination.stalePartials, "unfinished file")} from an interrupted copy will be replaced.
      </p>
    {/if}
  {/if}
</div>

<style>
  .preflight p {
    margin: 6px 0;
  }

  .danger {
    color: var(--danger);
  }

  .warning {
    color: var(--warning);
  }

  .muted {
    color: var(--text-muted);
  }

  .danger-box summary {
    color: var(--danger);
    cursor: pointer;
  }

  fieldset {
    border: none;
    padding: 0;
    margin: 6px 0;
    display: flex;
    gap: 16px;
    align-items: center;
  }

  legend {
    float: left;
    margin-right: 8px;
  }
</style>
