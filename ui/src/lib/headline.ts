// The summary's status line (RFD §5.4): unambiguous about what happened.

import type { SummaryView } from "./bindings";
import { plural } from "./format";

export function headline(s: SummaryView): string {
  switch (s.outcome) {
    case "stopped":
      return `Stopped: ${s.stoppedBecause ?? "the copy could not continue"}`;
    case "cancelled":
      return "Cancelled";
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
