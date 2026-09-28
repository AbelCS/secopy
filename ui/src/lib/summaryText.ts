// What a summary says in short: its figures, and the notification when a copy ends.
import type { QueueSummaryView, SummaryView } from "./bindings";
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

/** The notification when a queue run ends (FR-43). */
export function queueNotification(s: QueueSummaryView): { title: string; body: string } {
  const title = `Queue done: ${s.complete} of ${plural(s.count, "job")} complete`;
  const failed = s.results.filter((r) => r.result === "failed").length;
  return { title, body: failed > 0 ? `${plural(failed, "job")} failed` : "Every job finished." };
}
