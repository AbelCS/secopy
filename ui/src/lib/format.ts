// How figures are shown (RFD §5.3): decimal units like Finder, tabular digits in the CSS, and
// the separators and words of the current language (#84).
import { locale, t } from "./i18n";

/** The current language's number format, with `min`–`max` decimals. */
const numbers = (min = 0, max = 0) =>
  new Intl.NumberFormat(locale(), { minimumFractionDigits: min, maximumFractionDigits: max });

/** One decimal, rounded as `toFixed(1)` always did (1.15 → 1.1; Intl would say 1.2), in the
 *  language's separators. */
const oneDecimal = (value: number) => numbers(1, 1).format(Number(value.toFixed(1)));

/** 1284 → "1,284" */
export function formatCount(n: number): string {
  return numbers().format(n);
}

const UNITS = [
  "format.bytes.b",
  "format.bytes.kb",
  "format.bytes.mb",
  "format.bytes.gb",
  "format.bytes.tb",
  "format.bytes.pb",
] as const;

/** Decimal units, like Finder: 212400000000 → "212.4 GB". */
export function formatBytes(n: number): string {
  let value = n;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const figure = unit === 0 ? numbers().format(n) : oneDecimal(value);
  return t(UNITS[unit], { value: figure });
}

/** Bytes per second → "1.2 GB/s"; "—" when unknown. */
export function formatSpeed(bytesPerSecond: number | null): string {
  return bytesPerSecond === null
    ? t("format.unknown")
    : t("format.speed", { size: formatBytes(Math.round(bytesPerSecond)) });
}

/** Milliseconds → "0:07", "4:12", "1:02:03"; "—" when unknown. */
export function formatDuration(ms: number | null): string {
  if (ms === null || !Number.isFinite(ms)) return t("format.unknown");
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** done/total → "69.8 %"; an empty total counts as complete. */
export function formatPercent(done: number, total: number): string {
  const p = total === 0 ? 100 : (done * 100) / total;
  return t("format.percent", { value: oneDecimal(p) });
}

/** The message of something thrown: the app's commands throw Errors with its words. */
export function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** The last name in a path: "CLIP" for "/Volumes/CARD/CLIP"; "" for "/". */
export function baseName(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? "";
}
