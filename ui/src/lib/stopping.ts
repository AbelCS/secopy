// What stopping a job leaves behind, for the Cancel and quit questions, in the words of the
// job's kind. Only mentions the checksum file when the job writes one (RFD §5.5).

/** What a running job does: copy, mirror, or check (Verify). */
export type JobKind = "copy" | "mirror" | "check";

/** A queue job that is still being checked before it starts: nothing of it is written. */
export const NOT_STARTED = "This job hasn't started yet: nothing was written for it.";

/** The job's work in a question: "Stop copying?", "Stop verifying and quit?". */
export function doing(kind: JobKind): string {
  return kind === "check" ? "verifying" : kind === "mirror" ? "mirroring" : "copying";
}

export function stopMessage(kind: JobKind, checksumFile: boolean): string {
  // A check only reads; a stopped mirror removes nothing (FR-49) and writes no checksum file.
  if (kind === "check") return "Nothing was changed: a check only reads files.";
  if (kind === "mirror")
    return "Files already copied stay; the file in progress is removed. Files deleted in the origin are left in the destination.";
  return checksumFile
    ? "Files already copied stay and are listed in the checksum file; the file in progress is removed."
    : "Files already copied stay; the file in progress is removed.";
}

/** Quitting while a job removes files: that can't be stopped, so Secopy finishes it first. */
export function finishingMessage(undoing: boolean, archiving: boolean): string {
  const what = undoing
    ? "putting the destination back as it was"
    : `${archiving ? "archiving" : "deleting"} the files gone from the origin`;
  return `Secopy finishes ${what} first, then quits.`;
}
