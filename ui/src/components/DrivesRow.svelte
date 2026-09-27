<script lang="ts">
  // FROM's drives (plan 3b-1): the mounted volumes, asked for every 2 s. Clicking one
  // picks it, like a drop.
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
  let drives: DriveView[] = $state([]);

  const isOn = (d: DriveView) => source === d.path || !!source?.startsWith(`${d.path}/`);

  async function refresh() {
    try {
      drives = await api.listDrives();
    } catch {
      // Keep the last list; the next poll tries again.
    }
  }

  onMount(() => {
    void refresh();
    const timer = setInterval(refresh, POLL_MS);
    return () => clearInterval(timer);
  });
</script>

<div class="drives">
  {#each drives as d (d.path)}
    <Chip label={d.name} meta={formatBytes(d.totalBytes)} selected={isOn(d)} onToggle={() => onPick(d.path)} />
  {:else}
    <span class="muted">No cards or drives connected.</span>
  {/each}
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
