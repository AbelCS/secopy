// The summary's status line (RFD §5.4): unambiguous about what happened.

import type { SummaryView } from "./bindings";
import { formatCount, plural } from "./format";

export function headline(s: SummaryView): string {
  const c = s.check;
  if (c) {
    if (s.outcome === "cancelled") return "Cancelled";
    if (s.outcome === "stopped") return `Stopped: ${s.stoppedBecause ?? "the check could not continue"}`;
    // The summary lists the first 1,000 problems; the rest are only counted.
    const problems = c.problems.length + (c.moreProblems ?? 0);
    const parts = [
      c.changed > 0 ? plural(c.changed, "file") + " changed" : "",
      c.missing > 0 ? `${formatCount(c.missing)} missing` : "",
      c.failed > 0 ? `${formatCount(c.failed)} couldn't be read` : "",
      problems > 0 ? plural(problems, "checksum file problem") : "",
    ].filter(Boolean);
    return parts.length === 0 ? `All ${plural(c.intact, "file")} intact` : parts.join(" · ");
  }
  const m = s.mirror;
  // Failures other than files: what couldn't be read, removals, the checksum file.
  if (s.outcome === "failures" && s.failed === 0) {
    if (s.unread > 0) return `${plural(s.unread, "item")} couldn't be read`;
    if (m && m.removalFailures.length > 0) return `${plural(m.removalFailures.length, "file")} couldn't be removed`;
    if (s.dirErrors > 0) return `${plural(s.dirErrors, "empty directory", "empty directories")} couldn't be created`;
    if (s.checksumError) return "The checksum file couldn't be written";
    if (s.durabilityError) return "The destination couldn't confirm the files are saved";
  }
  if (m && s.outcome === "complete") {
    const parts = [
      m.new > 0 ? `${formatCount(m.new)} new` : "",
      m.updated > 0 ? `${formatCount(m.updated)} updated` : "",
      m.removed > 0 ? `${formatCount(m.removed)} ${m.archived ? "archived" : "deleted"}` : "",
    ].filter(Boolean);
    return parts.length === 0 ? "Already in sync" : `Mirrored: ${parts.join(", ")}`;
  }
  switch (s.outcome) {
    case "stopped":
      return `Stopped: ${s.stoppedBecause ?? "the copy could not continue"}`;
    case "cancelled":
      if (!s.undone) return "Cancelled";
      return s.undone.notRestored === 0 && s.undone.failed === 0
        ? "Cancelled: the destination is back as it was"
        : "Cancelled: the copied files were removed";
    case "failures":
      return `${plural(s.failed, "file")} failed`;
    case "complete": {
      const done = s.copied + s.verified;
      // Skip left files out: a different file has their name. Never "All" then.
      if (s.skippedDifferent > 0) {
        const kept = `${plural(s.skippedDifferent, "different file")} left as ${s.skippedDifferent === 1 ? "it was" : "they were"}`;
        if (done === 0) return `Nothing copied: ${kept}`;
        return `${plural(done, "file")} ${s.verify ? "copied and verified" : "copied"}; ${kept}`;
      }
      if (done === 0) return "Nothing to copy: everything was already there";
      return s.verify
        ? `All ${plural(done, "file")} copied and verified`
        : `All ${plural(done, "file")} copied`;
    }
  }
}
