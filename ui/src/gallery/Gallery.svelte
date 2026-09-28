<script lang="ts">
  // Pick a page with the hash: #components (default), #setup, #progress, #summary,
  // #settings, #profiles, #profiles-empty.
  import { createRawSnippet } from "svelte";
  import App from "../App.svelte";
  import JobProgress from "../components/JobProgress.svelte";
  import ProfilesScreen from "../components/ProfilesScreen.svelte";
  import QueueScreen from "../components/QueueScreen.svelte";
  import QueueSummary from "../components/QueueSummary.svelte";
  import SettingsScreen from "../components/SettingsScreen.svelte";
  import Summary from "../components/Summary.svelte";
  import { provideApi } from "../lib/api";
  import ActionBar from "../lib/ui/ActionBar.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Checkbox from "../lib/ui/Checkbox.svelte";
  import Chip from "../lib/ui/Chip.svelte";
  import EmptyState from "../lib/ui/EmptyState.svelte";
  import Notice from "../lib/ui/Notice.svelte";
  import ProgressBar from "../lib/ui/ProgressBar.svelte";
  import RadioGroup from "../lib/ui/RadioGroup.svelte";
  import Section from "../lib/ui/Section.svelte";
  import SegmentedControl from "../lib/ui/SegmentedControl.svelte";
  import Select from "../lib/ui/Select.svelte";
  import Stats from "../lib/ui/Stats.svelte";
  import TextField from "../lib/ui/TextField.svelte";
  import { fakeApi, profiles, progress, queue, queueSummary, settings, summary } from "./fake";

  const page = location.hash.slice(1) || "components";
  const api = fakeApi(page === "profiles-empty" ? { profiles: [] } : {});
  provideApi(api);

  const colours = ["bg", "surface", "surface-raised", "border", "text", "text-muted", "text-faint", "accent", "accent-soft", "success", "warning", "danger"];
  let mode: "copy" | "verify" = $state("verify");
  let all = $state(false);
  let on = $state(true);
  const help = createRawSnippet(() => ({ render: () => "<span>A line of help under the option.</span>" }));
</script>

{#if page === "setup"}
  <App {api} />
{:else if page === "progress"}
  <JobProgress {progress} />
{:else if page === "summary"}
  <Summary {summary} onRetry={() => {}} onNewCopy={() => {}} onSettings={() => {}} />
{:else if page === "settings"}
  <SettingsScreen {settings} onSettings={() => {}} onDone={() => {}} />
{:else if page === "queue-summary"}
  <QueueSummary summary={queueSummary} onOpen={() => {}} onDone={() => {}} />
{:else if page === "queue"}
  <QueueScreen {queue} onQueue={() => {}} onRun={() => {}} onSettings={() => {}} />
{:else if page === "profiles" || page === "profiles-empty"}
  <ProfilesScreen profiles={page === "profiles" ? profiles : []} onProfiles={() => {}} onView={() => {}} onDone={() => {}} />
{:else}
  <div class="gallery">
    <h1>Secopy design system</h1>

    <Section title="Colour tokens">
      <div class="swatches">
        {#each colours as c (c)}
          <div class="swatch"><span style:background="var(--{c})"></span>{c}</div>
        {/each}
      </div>
    </Section>

    <Section title="Buttons">
      <div class="row">
        <Button variant="primary">Primary</Button>
        <Button>Secondary</Button>
        <Button variant="danger">Delete…</Button>
        <Button icon="chevron-left">Back</Button>
        <Button variant="link">+ New profile</Button>
        <Button icon="settings">Settings</Button>
        <Button variant="primary" disabled>Disabled</Button>
      </div>
    </Section>

    <Section title="Choices">
      <div class="column">
        <SegmentedControl
          label="Mode"
          options={[
            { value: "copy", label: "Copy" },
            { value: "verify", label: "Copy & Verify" },
          ]}
          value={mode}
          onChange={(v) => (mode = v)}
        />
        <Checkbox label="An option" checked={on} {help} onChange={(v) => (on = v)} />
        <RadioGroup
          legend="File types"
          options={[
            { value: true, label: "All types" },
            { value: false, label: "Only these" },
          ]}
          value={all}
          onChange={(v) => (all = v)}
        />
        <div class="row">
          <Chip label=".mp4" meta="106 · 180.0 GB" selected onToggle={() => {}} />
          <Chip label=".xml" meta="106 · 400 KB" onToggle={() => {}} />
          <Chip label=".mov" onRemove={() => {}} />
        </div>
        <Select label="Profile" value="fx3" options={[{ value: "fx3", label: "Sony FX3" }]} onChange={() => {}} />
      </div>
    </Section>

    <Section title="Fields">
      <div class="column">
        <TextField label="Name" value="Sony FX3" help="Shown in the Profile menu." />
        <TextField label="Source" value="DCIM" mono error="The source must be a full path, like /Volumes/CARD_A/DCIM.">
          {#snippet trailing()}<Button>Choose…</Button>{/snippet}
        </TextField>
      </div>
    </Section>

    <Section title="Feedback">
      <Notice tone="info">Something worth knowing.</Notice>
      <Notice tone="success">Saved.</Notice>
      <Notice tone="warning">The destination already contains 12 items.</Notice>
      <Notice tone="danger">Stopped: the source is no longer available.</Notice>
      <Stats items={["3 files", "7.0 GB written", "took 0:06", "1.2 GB/s average"]} />
      <ProgressBar label="Copied" done={72e9} total={180e9} speed={1.1e9} />
      <EmptyState><p>Nothing here yet: this is where an empty part of a screen says how to fill it.</p></EmptyState>
    </Section>

    <Section title="Action bar">
      <ActionBar status="Status in the middle">
        {#snippet start()}<Button variant="danger">Delete…</Button>{/snippet}
        {#snippet end()}<Button>Revert</Button><Button variant="primary">Save</Button>{/snippet}
      </ActionBar>
    </Section>
  </div>
{/if}

<style>
  .gallery {
    max-width: 900px;
    margin: 0 auto;
    padding: var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  h1 {
    font-size: var(--text-lg);
    margin: 0 0 var(--space-2);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--space-2);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .swatch {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .swatch span {
    width: 24px;
    height: 24px;
    border-radius: var(--radius-control);
    border: 1px solid var(--border);
  }
</style>
