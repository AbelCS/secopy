<script lang="ts">
  // One phase's bar (RFD §5.3): bytes, percent, current and average speed, ETA. The fill
  // animates for as long as the gap between updates, so two updates a second look smooth.
  import { formatBytes, formatDuration, formatPercent, formatSpeed } from "../lib/format";

  let {
    label,
    done,
    total,
    speed,
    average,
    eta,
  }: {
    label: string;
    done: number;
    total: number;
    speed: number | null;
    average: number | null;
    eta: number | null;
  } = $props();

  const fraction = $derived(total === 0 ? 1 : Math.min(1, done / total));
</script>

<div class="bar-row">
  <span class="label">{label}</span>
  <div
    class="track"
    role="progressbar"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(fraction * 100)}
  >
    <div class="fill" style:width="{fraction * 100}%"></div>
  </div>
  <span class="figures">
    {formatBytes(done)} / {formatBytes(total)} · {formatPercent(done, total)} ·
    {formatSpeed(speed)} (avg {formatSpeed(average)}) · ETA {formatDuration(eta)}
  </span>
</div>

<style>
  .bar-row {
    display: grid;
    grid-template-columns: 70px 1fr;
    gap: 4px 12px;
    align-items: center;
    margin: 8px 0;
  }

  .label {
    color: var(--text-muted);
  }

  .track {
    height: 8px;
    border-radius: 4px;
    background: var(--surface-raised);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.5s linear;
  }

  .figures {
    grid-column: 2;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
