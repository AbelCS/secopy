// What cancelling a job leaves behind, for the Cancel and quit questions, in the words of the
// job's kind. Only mentions the checksum file when the job writes one (RFD §5.5). Whole
// sentences per kind, so each language can phrase them its own way (#84).
import { t } from "./i18n";

/** What a running job does: copy, mirror, or check (Verify). */
export type JobKind = "copy" | "mirror" | "check";

/** A queue job that is still being checked before it starts: nothing of it is written. */
export const notStarted = (): string => t("progress.stop.notStarted");

/** Cancel's question: "Cancel this job?", "Cancel this job and stop the queue?" (#172: the user
 *  cancels; a job that can't go on stops). */
export function stopQuestion(inQueue: boolean): string {
  return t(inQueue ? "progress.stop.questionQueue" : "progress.stop.question");
}

/** Quitting's question and its two answers: "Cancel this job and quit?", "Cancel job and quit", "Continue". */
export function quitQuestion(): { title: string; stop: string; keep: string } {
  return { title: t("progress.quit.question"), stop: t("progress.quit.stop"), keep: t("progress.quit.keep") };
}

export function stopMessage(kind: JobKind, checksumFile: boolean): string {
  // A check only reads; a stopped mirror removes nothing (FR-49) and writes no checksum file.
  if (kind === "check") return t("progress.stop.left.check");
  if (kind === "mirror") return t("progress.stop.left.mirror");
  return t(checksumFile ? "progress.stop.left.copyListed" : "progress.stop.left.copy");
}

/** Quitting while a job removes files: that can't be stopped, so Secopy finishes it first. */
export function finishingMessage(undoing: boolean, archiving: boolean): string {
  if (undoing) return t("progress.quit.finishing.undo");
  return t(archiving ? "progress.quit.finishing.archive" : "progress.quit.finishing.delete");
}
