// The summary's status line (RFD §5.4): unambiguous about what happened.

import type { SummaryView } from "./bindings";
import { formatCount, plural } from "./format";

export function headline(s: SummaryView): string {
  const m = s.mirror;
  // Failures other than files: what couldn't be read, removals, the checksum file.
  if (s.outcome === "failures" && s.failed === 0) {
    if (s.unread > 0) return `${plural(s.unread, "item")} couldn't be read`;
    if (m && m.removalFailures.length > 0) return `${plural(m.removalFailures.length, "file")} couldn't be removed`;
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
      if (done === 0) return "Nothing to copy: everything was already there";
      return s.verify
        ? `All ${plural(done, "file")} copied and verified`
        : `All ${plural(done, "file")} copied`;
    }
  }
}
