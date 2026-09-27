<script lang="ts">
  // FROM's drives (plan 3b-1): the mounted volumes, asked for every 2 s. Clicking one
  // picks it, like a drop.
  import { onMount } from "svelte";
  import { useApi } from "../lib/api";
  import type { DriveView } from "../lib/bindings";
  import { formatBytes } from "../lib/format";

  let {
    source,
    onPick,
  }: {
    /** The current source folder, to show which drive it is on. */
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

{#if drives.length > 0}
  <div class="drives" role="group" aria-label="Drives">
    {#each drives as d (d.path)}
      <button type="button" class="drive" class:on={isOn(d)} aria-pressed={isOn(d)} onclick={() => onPick(d.path)}>
        {d.name} <span class="muted">{formatBytes(d.totalBytes)}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .drives {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0 0 8px;
  }

  .drive.on {
    border-color: var(--accent);
  }

  .muted {
    color: var(--text-muted);
  }
</style>
