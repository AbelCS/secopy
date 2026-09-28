// What a summary says in short: its figures, and the notification when a copy ends.
import type { QueueSummaryView, SummaryView } from "./bindings";
import { formatBytes, formatCount, formatDuration, formatSpeed, plural } from "./format";
import { headline } from "./headline";

import type { Stat } from "./ui/Stats.svelte";

export function summaryStats(s: SummaryView): Stat[] {
  // A check reads, it doesn't write: its files, its time, and what nothing lists.
  if (s.check) {
    const stats: Stat[] = [plural(s.files, "file"), `took ${formatDuration(s.millis)}`];
    if (s.check.notChecked > 0)
      stats.push({
        text: `${formatCount(s.check.notChecked)} not checked`,
        hint: "Files no checksum file lists: nothing to compare them with.",
      });
    return stats;
  }
  const speed = formatSpeed(s.millis > 0 ? (s.bytesWritten * 1000) / s.millis : null);
  const items = [plural(s.files, "file"), `${formatBytes(s.bytesWritten)} written`, `took ${formatDuration(s.millis)}`, `${speed} average`];
  if (s.skippedIdentical > 0) items.push(`${formatCount(s.skippedIdentical)} already at the destination, not checked`);
  if (s.skippedDifferent > 0) items.push(`${plural(s.skippedDifferent, "different file")} left as they were`);
  const stats: Stat[] = items;
  if (s.notStarted > 0) {
    stats.push({
      text: `${formatCount(s.notStarted)} not started`,
      hint: "Files the job didn't reach because it was cancelled or stopped.",
    });
  }
  return stats;
}

export function notificationFor(s: SummaryView): { title: string; body: string } {
  const mark = s.outcome === "complete" ? "✓" : "✗";
  const body = summaryStats(s)
    .slice(0, 3)
    .map((i) => (typeof i === "string" ? i : i.text))
    .join(" · ");
  return { title: `${mark} ${headline(s)}`, body };
}

/** The notification when a queue run ends (FR-43). */
export function queueNotification(s: QueueSummaryView): { title: string; body: string } {
  const title = `Queue done: ${s.complete} of ${plural(s.count, "job")} complete`;
  const failed = s.results.filter((r) => r.result === "failed").length;
  return { title, body: failed > 0 ? `${plural(failed, "job")} failed` : "Every job finished." };
}
