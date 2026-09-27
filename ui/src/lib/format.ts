// How figures are shown (RFD §5.3): decimal units like Finder, tabular digits in the CSS.

const count = new Intl.NumberFormat("en-US");

/** 1284 → "1,284" */
export function formatCount(n: number): string {
  return count.format(n);
}

/** Decimal units, like Finder: 212400000000 → "212.4 GB". */
export function formatBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let value = n;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return unit === 0 ? `${n} B` : `${value.toFixed(1)} ${units[unit]}`;
}

/** Bytes per second → "1.2 GB/s"; "—" when unknown. */
export function formatSpeed(bytesPerSecond: number | null): string {
  return bytesPerSecond === null ? "—" : `${formatBytes(Math.round(bytesPerSecond))}/s`;
}

/** Milliseconds → "0:07", "4:12", "1:02:03"; "—" when unknown. */
export function formatDuration(ms: number | null): string {
  if (ms === null || !Number.isFinite(ms)) return "—";
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** done/total → "69.8 %"; an empty total counts as complete. */
export function formatPercent(done: number, total: number): string {
  const p = total === 0 ? 100 : (done * 100) / total;
  return `${p.toFixed(1)} %`;
}

/** "1 file", "2 files" */
export function plural(n: number, one: string, many = `${one}s`): string {
  return `${formatCount(n)} ${n === 1 ? one : many}`;
}
