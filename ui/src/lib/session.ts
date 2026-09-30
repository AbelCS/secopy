// New copy's view comes back from several commands, in any order (#138): the newest wins.
import type { SessionView } from "./bindings";

/** `next` if it's at least as new as `current` (and not a stale scan's); else `current`. */
export function newest(current: SessionView, next: SessionView): SessionView {
  return !next.stale && next.revision >= current.revision ? next : current;
}
