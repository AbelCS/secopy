<script lang="ts">
  // FROM's drives (plan 3b-1): the mounted volumes, asked for again 2 s after each answer
  // (never two questions at once). Clicking one picks it, like a drop.
  import { onMount } from "svelte";
  import { useApi } from "../lib/api";
  import type { DriveView } from "../lib/bindings";
  import { formatBytes } from "../lib/format";
  import Chip from "../lib/ui/Chip.svelte";

  let {
    source,
    onPick,
  }: {
    /** The current source directory, to show which drive it is on. */
    source: string | null;
    onPick: (path: string) => void;
  } = $props();

  const api = useApi();
  const POLL_MS = 2000;
  /** `null` until the first answer. */
  let drives: DriveView[] | null = $state(null);

  const isOn = (d: DriveView) => source === d.path || !!source?.startsWith(`${d.path}/`);

  async function refresh() {
    try {
      drives = await api.listDrives();
    } catch {
      // Keep the last list; the next poll tries again.
    }
  }

  onMount(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;
    async function poll() {
      await refresh();
      if (!stopped) timer = setTimeout(poll, POLL_MS);
    }
    void poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });
</script>

<div class="drives">
  {#if drives === null}
    <span class="muted">Looking for drives…</span>
  {:else}
    {#each drives as d (d.path)}
      <Chip label={d.name} meta={formatBytes(d.totalBytes)} selected={isOn(d)} onToggle={() => onPick(d.path)} />
    {:else}
      <span class="muted">No cards or drives connected.</span>
    {/each}
  {/if}
</div>

<style>
  .drives {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .muted {
    color: var(--text-muted);
    padding-top: 4px;
  }
</style>
