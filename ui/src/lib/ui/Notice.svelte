<script lang="ts">
  // A message with an icon and words, never colour alone. Only danger interrupts (an alert);
  // the rest are polite status. `announce={false}` for text that is there when the screen
  // opens, which a screen reader reads with the rest.
  import type { Snippet } from "svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    tone = "info",
    announce = true,
    children,
  }: { tone?: "info" | "success" | "warning" | "danger"; announce?: boolean; children: Snippet } = $props();
  const icons: Record<string, IconName> = { info: "info", success: "check", warning: "alert", danger: "alert" };
</script>

<!-- No whitespace between the icon and the text: the notice's text is exactly the message. -->
<div class="notice {tone}" role={announce ? (tone === "danger" ? "alert" : "status") : undefined}><Icon name={icons[tone]} /><div class="text">{@render children()}</div></div>

<style>
  .notice {
    display: flex;
    gap: var(--space-2);
    align-items: flex-start;
    margin: var(--space-2) 0;
    line-height: 1.4;
  }

  .notice :global(.icon) {
    margin-top: 2px;
  }

  .text {
    min-width: 0;
  }

  .info {
    color: var(--text-muted);
  }

  .success {
    color: var(--success);
  }

  .warning {
    color: var(--warning);
  }

  .danger {
    color: var(--danger);
  }
</style>
