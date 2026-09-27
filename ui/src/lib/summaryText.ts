// What a summary says in short: its figures, and the notification when a copy ends.
import type { SummaryView } from "./bindings";
import { formatBytes, formatCount, formatDuration, formatSpeed, plural } from "./format";
import { headline } from "./headline";

export function summaryStats(s: SummaryView): string[] {
  const speed = formatSpeed(s.millis > 0 ? (s.bytesWritten * 1000) / s.millis : null);
  const items = [plural(s.files, "file"), `${formatBytes(s.bytesWritten)} written`, `took ${formatDuration(s.millis)}`, `${speed} average`];
  if (s.skippedIdentical > 0) items.push(`${formatCount(s.skippedIdentical)} already at the destination, not checked`);
  if (s.skippedDifferent > 0) items.push(`${plural(s.skippedDifferent, "different file")} left as they were`);
  if (s.notStarted > 0) items.push(`${formatCount(s.notStarted)} not started`);
  return items;
}

export function notificationFor(s: SummaryView): { title: string; body: string } {
  const mark = s.outcome === "complete" ? "✓" : "✗";
  return { title: `${mark} ${headline(s)}`, body: summaryStats(s).slice(0, 3).join(" · ") };
}
