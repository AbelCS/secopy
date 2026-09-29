// What stopping a job leaves behind, for the Cancel and quit questions, in the words of the
// job's kind. Only mentions the checksum file when the job writes one (RFD §5.5). Whole
// sentences per kind, so each language can phrase them its own way (#84).
import { t, type Key } from "./i18n";

/** What a running job does: copy, mirror, or check (Verify). */
export type JobKind = "copy" | "mirror" | "check";

const QUESTION: Record<JobKind, Key> = {
  copy: "progress.stop.question.copy",
  mirror: "progress.stop.question.mirror",
  check: "progress.stop.question.check",
};
const QUESTION_QUEUE: Record<JobKind, Key> = {
  copy: "progress.stop.questionQueue.copy",
  mirror: "progress.stop.questionQueue.mirror",
  check: "progress.stop.questionQueue.check",
};
const QUIT: Record<JobKind, [Key, Key, Key]> = {
  copy: ["progress.quit.question.copy", "progress.quit.stop.copy", "progress.quit.keep.copy"],
  mirror: ["progress.quit.question.mirror", "progress.quit.stop.mirror", "progress.quit.keep.mirror"],
  check: ["progress.quit.question.check", "progress.quit.stop.check", "progress.quit.keep.check"],
};

/** A queue job that is still being checked before it starts: nothing of it is written. */
export const notStarted = (): string => t("progress.stop.notStarted");

/** Cancel's question: "Stop copying?", "Stop verifying and stop the queue?". */
export function stopQuestion(kind: JobKind, inQueue: boolean): string {
  return t(inQueue ? QUESTION_QUEUE[kind] : QUESTION[kind]);
}

/** Quitting's question and its two answers: "Stop copying and quit?", "Stop copying", "Keep copying". */
export function quitQuestion(kind: JobKind): { title: string; stop: string; keep: string } {
  const [title, stop, keep] = QUIT[kind];
  return { title: t(title), stop: t(stop), keep: t(keep) };
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
