// The summary's status line (RFD §5.4): unambiguous about what happened.

import type { SummaryView } from "./bindings";
import { t } from "./i18n";
import { say } from "./message";

export function headline(s: SummaryView): string {
  const c = s.check;
  if (c) {
    if (s.outcome === "cancelled") return t("summary.headline.cancelled");
    if (s.outcome === "stopped")
      return t("summary.headline.stopped", {
        why: s.stoppedBecause ? say(s.stoppedBecause) : t("summary.headline.checkCannotContinue"),
      });
    // The summary lists the first 1,000 problems; the rest are only counted.
    const problems = c.problems.length + (c.moreProblems ?? 0);
    const parts = [
      c.changed > 0 ? t("summary.headline.check.changed", { count: c.changed }) : "",
      c.missing > 0 ? t("summary.headline.check.missing", { count: c.missing }) : "",
      c.failed > 0 ? t("summary.headline.check.unreadable", { count: c.failed }) : "",
      problems > 0 ? t("summary.headline.check.problems", { count: problems }) : "",
    ].filter(Boolean);
    return parts.length === 0 ? t("summary.headline.check.intact", { count: c.intact }) : parts.join(t("format.dot"));
  }
  const m = s.mirror;
  // Failures other than files: what couldn't be read, removals, the checksum file.
  if (s.outcome === "failures" && s.failed === 0) {
    if (s.unread > 0) return t("summary.headline.unread", { count: s.unread });
    if (m && m.removalFailures.length > 0) return t("summary.headline.notRemoved", { count: m.removalFailures.length });
    if (s.dirErrors > 0) return t("summary.headline.dirErrors", { count: s.dirErrors });
    if (s.checksumError) return t("summary.headline.checksumFailed");
    if (s.durabilityError) return t("summary.headline.notDurable");
  }
  if (m && s.outcome === "complete") {
    const parts = [
      m.new > 0 ? t("summary.headline.mirror.new", { count: m.new }) : "",
      m.updated > 0 ? t("summary.headline.mirror.updated", { count: m.updated }) : "",
      m.removed > 0
        ? t(m.archived ? "summary.headline.mirror.archived" : "summary.headline.mirror.deleted", { count: m.removed })
        : "",
    ].filter(Boolean);
    return parts.length === 0
      ? t("summary.headline.mirror.inSync")
      : t("summary.headline.mirror.done", { parts: parts.join(t("format.comma")) });
  }
  switch (s.outcome) {
    case "stopped":
      return t("summary.headline.stopped", {
        why: s.stoppedBecause ? say(s.stoppedBecause) : t("summary.headline.copyCannotContinue"),
      });
    case "cancelled":
      if (!s.undone) return t("summary.headline.cancelled");
      return s.undone.notRestored === 0 && s.undone.failed === 0
        ? t("summary.headline.undoneBack")
        : t("summary.headline.undoneRemoved");
    case "failures":
      return t("summary.headline.failed", { count: s.failed });
    case "complete": {
      const done = s.copied + s.verified;
      // Skip left files out: a different file has their name. Never "All" then.
      if (s.skippedDifferent > 0) {
        const kept = t("summary.headline.keptDifferent", { count: s.skippedDifferent });
        if (done === 0) return t("summary.headline.nothingCopied", { kept });
        return t(s.verify ? "summary.headline.someVerified" : "summary.headline.someCopied", { count: done, kept });
      }
      if (done === 0) return t("summary.headline.nothingToCopy");
      return t(s.verify ? "summary.headline.allVerified" : "summary.headline.allCopied", { count: done });
    }
  }
}
