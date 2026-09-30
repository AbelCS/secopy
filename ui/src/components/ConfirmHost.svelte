<script lang="ts">
  // Shows the question `ask` holds (#113): once, over the whole window.
  import { asking } from "../lib/confirm.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";
</script>

{#if asking.current}
  {@const q = asking.current}
  <Dialog title={q.title} onClose={() => q.answer(false)}>
    <p>{q.message}</p>
    {#snippet actions()}
      <Button data-autofocus onclick={() => q.answer(false)}>{q.cancel}</Button>
      <Button variant="danger" onclick={() => q.answer(true)}>{q.ok}</Button>
    {/snippet}
  </Dialog>
{/if}
