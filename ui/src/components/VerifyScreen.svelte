<script lang="ts">
  // Verify (plan 8, FR-34): choose the directory of a copy, or a whole drive; every file its
  // checksum files list is read again and compared. Nothing is written to it.
  import { onMount, type Snippet } from "svelte";
  import { useApi } from "../lib/api";
  import type { CheckView, QueueView } from "../lib/bindings";
  import { formatBytes, messageOf, plural } from "../lib/format";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import FormRow from "../lib/ui/FormRow.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";

  let {
    onStart,
    onQueue,
    banner,
  }: {
    /** Verify's Start: check `path`. */
    onStart: (path: string) => void;
    onQueue: (queue: QueueView) => void;
    /** App-wide messages, shown first. */
    banner?: Snippet;
  } = $props();

  const api = useApi();
  let view = $state<CheckView | null>(null);
  let error: string | null = $state(null);
  let checking = $state(false);
  const ready = $derived(!!view && view.files > 0 && !checking);

  async function choose(path: string | null) {
    if (path === null) return;
    checking = true;
    try {
      view = await api.checkDirectory(path);
      error = null;
    } catch (e) {
      view = null;
      error = messageOf(e);
    } finally {
      checking = false;
    }
  }

  async function queue() {
    if (!view) return;
    try {
      onQueue(await api.addCheckToQueue(view.directory));
      error = null;
    } catch (e) {
      error = messageOf(e);
    }
  }

  onMount(() => {
    // A directory dropped on the Directory row is chosen, like Choose….
    const unlisten = api.onDrop((paths, target) => {
      if (target?.closest("[data-drop]")?.getAttribute("data-drop") === "verify" && paths.length > 0)
        void choose(paths[0]);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

<AppShell>
  {#snippet header()}<ScreenHeader title="Verify" />{/snippet}

  {@render banner?.()}
  <Section title="Directory">
    <div data-drop="verify">
      <FormRow label="Directory">
        {#if view}
          <p class="mono path">{view.directory}</p>
          {#if view.files > 0}
            <p class="muted">
              {plural(view.checksumFiles, "checksum file")} · {plural(view.files, "file")} listed · {formatBytes(view.bytes)}
              · {plural(view.notChecked, "file")} not listed
            </p>
          {/if}
        {:else}
          <p class="muted">
            Choose the directory of a copy, or a whole drive: Secopy reads every file its checksum files list and
            compares it. Nothing is written to it.
          </p>
        {/if}
        {#snippet aside()}
          <Button disabled={checking} onclick={async () => choose(await api.pickDirectory("Directory to verify"))}>
            Choose…
          </Button>
        {/snippet}
      </FormRow>
    </div>
    {#if view && view.files === 0}
      <Notice tone="warning">
        No checksum files here: there's nothing to verify. Secopy writes one with every copy and every mirror.
      </Notice>
    {/if}
    {#if view && view.problems.length > 0}
      <Notice tone="danger">
        {#each view.problems as p (p)}<p class="problem">{p}</p>{/each}
      </Notice>
    {/if}
    {#if error}<Notice tone="danger">{error}</Notice>{/if}
  </Section>

  {#snippet actions()}
    <ActionBar>
      {#snippet end()}
        <Button
          disabled={!ready}
          help="Adds this directory to the Queue; it's verified when the queue gets to it."
          onclick={queue}>Add to queue</Button
        >
        <Button
          variant="primary"
          disabled={!ready}
          help={view
            ? `Reads the ${plural(view.files, "listed file")} (${formatBytes(view.bytes)}) and compares each with its checksum; nothing is written.`
            : ""}
          onclick={() => view && onStart(view.directory)}>Start</Button
        >
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .path {
    margin: 0;
    word-break: break-all;
  }

  .muted {
    margin: var(--space-1) 0 0;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }

  .problem {
    margin: 0;
  }
</style>
