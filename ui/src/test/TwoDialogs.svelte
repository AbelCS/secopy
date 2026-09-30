<script lang="ts">
  // Two dialogs at once, the second over the first: for the Esc test.
  import Button from "../lib/ui/Button.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";

  let { onFirst, onSecond }: { onFirst: () => void; onSecond: () => void } = $props();
</script>

<Dialog title="First" onClose={onFirst}>
  <p>Below</p>
  {#snippet actions()}<Button data-autofocus onclick={onFirst}>Keep</Button>{/snippet}
</Dialog>
<Dialog title="Second" onClose={onSecond}>
  <p>On top</p>
  {#snippet actions()}<Button data-autofocus onclick={onSecond}>Keep</Button>{/snippet}
</Dialog>
