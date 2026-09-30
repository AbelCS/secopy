// What a summary says in short: its figures, and the notification when a copy ends.
import type { QueueSummaryView, SummaryView } from "./bindings";
import { formatBytes, formatDuration, formatSpeed } from "./format";
import { t } from "./i18n";
import { headline } from "./headline";
import { say } from "./message";

import type { Stat } from "./ui/Stats.svelte";

export function summaryStats(s: SummaryView): Stat[] {
  // A check reads, it doesn't write: its files, its time, and what nothing lists.
  if (s.check) {
    const stats: Stat[] = [
      t("summary.stats.files", { count: s.files }),
      t("summary.stats.took", { time: formatDuration(s.millis) }),
    ];
    if (s.check.notChecked > 0)
      stats.push({
        text: t("summary.stats.notChecked", { count: s.check.notChecked }),
        hint: t("summary.stats.notCheckedHint"),
      });
    return stats;
  }
  const speed = formatSpeed(s.millis > 0 ? (s.bytesWritten * 1000) / s.millis : null);
  const items = [
    t("summary.stats.files", { count: s.files }),
    t("summary.stats.written", { size: formatBytes(s.bytesWritten) }),
    t("summary.stats.took", { time: formatDuration(s.millis) }),
    t("summary.stats.average", { speed }),
  ];
  if (s.skippedIdentical > 0) items.push(t("summary.stats.alreadyThere", { count: s.skippedIdentical }));
  if (s.skippedDifferent > 0) items.push(t("summary.stats.keptDifferent", { count: s.skippedDifferent }));
  const stats: Stat[] = items;
  if (s.notStarted > 0) {
    stats.push({
      text: t("summary.stats.notStarted", { count: s.notStarted }),
      hint: t("summary.stats.notStartedHint"),
    });
  }
  return stats;
}

export function notificationFor(s: SummaryView): { title: string; body: string } {
  const mark = s.outcome === "complete" ? "✓" : "✗";
  // A report that couldn't be saved comes first: the proof is missing (#116).
  const lost = s.reportErrors.length > 0 ? [t("notify.job.reportNotSaved")] : [];
  const body = [...lost, ...summaryStats(s).map((i) => (typeof i === "string" ? i : i.text))]
    .slice(0, 3)
    .join(t("format.dot"));
  return { title: t("notify.job.title", { mark, headline: headline(s) }), body };
}

/** The notification when a queue run ends (FR-43). */
export function queueNotification(s: QueueSummaryView): { title: string; body: string } {
  const title = t("notify.queue.title", { complete: s.complete, count: s.count });
  const count = (r: QueueSummaryView["results"][number]["result"]) => s.results.filter((x) => x.result === r).length;
  // "Every job finished." only when each is complete; otherwise what wasn't (#116).
  const parts = [
    [count("failed"), "notify.queue.failed"],
    [count("cancelled"), "notify.queue.cancelled"],
    [count("notRun"), "notify.queue.notRun"],
  ] as const;
  const said = parts.filter(([n]) => n > 0).map(([n, key]) => t(key, { count: n }));
  const body = s.saveError
    ? say(s.saveError)
    : said.length > 0
      ? said.join(t("format.dot"))
      : t("notify.queue.allFinished");
  return { title, body };
}
