<script lang="ts">
  // Source profiles (FR-38): the list on the left, the selected one's editor on the right.
  import { messageOf } from "../lib/format";
  import { useApi } from "../lib/api";
  import type { Profile, ProfileInput, SessionView } from "../lib/bindings";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import EmptyState from "../lib/ui/EmptyState.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";
  import Section from "../lib/ui/Section.svelte";
  import ProfileEditor from "./ProfileEditor.svelte";

  let {
    profiles,
    onProfiles,
    onView,
    onDone,
  }: {
    profiles: Profile[];
    onProfiles: (profiles: Profile[]) => void;
    /** Editing or deleting the selected profile changes what FROM shows. */
    onView: (view: SessionView) => void;
    onDone: () => void;
  } = $props();

  const api = useApi();
  const NEW = "new";
  // svelte-ignore state_referenced_locally
  let selectedId: string | null = $state(profiles[0]?.id ?? null);
  let error: string | null = $state(null);
  /** The editor has unsaved changes. */
  let changed = $state(false);
  let canSave = $state(false);
  let editor: ReturnType<typeof ProfileEditor> | undefined = $state();
  const FORM = "profile-editor";

  const selected = $derived(profiles.find((p) => p.id === selectedId) ?? null);
  /** Recreates the editor for another profile, or after this one was saved. */
  const editorKey = $derived(selectedId === NEW ? NEW : JSON.stringify(selected));

  /** Whether it's fine to leave the profile being edited; asks when it has changes. */
  async function mayLeave(): Promise<boolean> {
    if (!changed) return true;
    const which = selectedId === NEW ? "the new profile" : `“${selected?.name ?? ""}”`;
    return api.confirm(`Your changes to ${which} aren't saved.`, "Discard changes?", "Discard", "Keep editing");
  }

  async function select(id: string) {
    if (id === selectedId || !(await mayLeave())) return;
    changed = false;
    selectedId = id;
  }

  async function back() {
    if (await mayLeave()) onDone();
  }

  async function save(input: ProfileInput) {
    if (selectedId === NEW) {
      const list = await api.createProfile(input);
      onProfiles(list);
      const name = input.name.trim().toLowerCase();
      selectedId = list.find((p) => p.name.toLowerCase() === name)?.id ?? null;
    } else if (selected) {
      const result = await api.editProfile(selected.id, input);
      onProfiles(result.profiles);
      onView(result.session);
    }
  }

  async function remove(p: Profile) {
    const sure = await api.confirm(
      `The profile “${p.name}” is deleted. Cards and copies are not touched.`,
      "Delete profile?",
      "Delete",
      "Keep",
    );
    if (!sure) return;
    try {
      const result = await api.deleteProfile(p.id);
      onProfiles(result.profiles);
      onView(result.session);
      selectedId = result.profiles[0]?.id ?? null;
      error = null;
    } catch (e) {
      error = messageOf(e);
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && void back()} />

<AppShell>
  {#snippet header()}<ScreenHeader title="Profiles" />{/snippet}

  <div class="panes">
    <Section title="All profiles">
      <nav class="list" aria-label="Profiles">
        {#each profiles as p (p.id)}
          <button
            type="button"
            class="item"
            class:on={p.id === selectedId}
            aria-label={p.name}
            aria-current={p.id === selectedId}
            onclick={() => select(p.id)}
          >
            <span>{p.name}</span>
            <!-- The left-to-right mark keeps the slashes in place inside the right-aligned cut. -->
            <span class="muted mono path" title={p.source}>{p.source ? `\u200E${p.source}` : "(no source yet)"}</span>
          </button>
        {/each}
        <Button variant="link" onclick={() => select(NEW)}>+ New profile</Button>
      </nav>
    </Section>

    <Section title={selectedId === NEW ? "New profile" : (selected?.name ?? "About profiles")}>
      {#if selectedId === NEW || selected}
        {#key editorKey}
          <ProfileEditor
            bind:this={editor}
            bind:changed
            bind:canSave
            formId={FORM}
            profile={selectedId === NEW ? null : selected}
            onSave={save}
          />
        {/key}
      {:else}
        <EmptyState>
          <p>
            A profile saves a source and its settings (whether that directory itself is copied, and which file
            types), so a copy you do often is set up in one step: choose the profile in the main window.
          </p>
          <p>Create one with “+ New profile”, or with “Save as new…” in the main window.</p>
        </EmptyState>
      {/if}
      {#if error}<Notice tone="danger">{error}</Notice>{/if}
    </Section>
  </div>

  {#snippet actions()}
    <ActionBar status={changed ? "Unsaved changes" : ""}>
      {#snippet start()}
        <Button icon="chevron-left" onclick={back}>Back</Button>
        {#if selected && selectedId !== NEW}
          <Button variant="danger" onclick={() => remove(selected)}>Delete…</Button>
        {/if}
      {/snippet}
      {#snippet end()}
        {#if selectedId === NEW || selected}
          <Button disabled={!changed} onclick={() => editor?.revert()}>Revert</Button>
          <Button variant="primary" type="submit" form={FORM} disabled={!canSave}>Save</Button>
        {/if}
      {/snippet}
    </ActionBar>
  {/snippet}
</AppShell>

<style>
  .panes {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: var(--space-3);
    align-items: start;
  }

  .list {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-1);
  }

  .list :global(.link) {
    align-self: flex-start;
    margin-top: var(--space-2);
  }

  .item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    text-align: left;
    font: inherit;
    color: var(--text);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    padding: var(--space-2);
    cursor: pointer;
  }

  .item.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .muted {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }

  /* A long source keeps its end (the directory that matters) visible. */
  .path {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
</style>
