// Settings › Always ignore when copying (#158): the checks the app makes on a typed pattern,
// the same the engine makes (crates/secopy-core/src/ignore.rs).

/** Characters a pattern has at most. */
export const MAX_LEN = 255;
/** Patterns the list holds at most. */
export const MAX_PATTERNS = 200;

/** Why `pattern` can't be added to `list`, or null. Spaces at its ends don't count. */
export function patternProblem(
  pattern: string,
  list: string[],
): "slash" | "tooLong" | "tooMany" | "badChar" | "repeat" | null {
  const p = pattern.replace(/^ +| +$/g, "");
  if (p.includes("/")) return "slash";
  if ([...p].some((c) => c < " ")) return "badChar";
  if ([...p].length > MAX_LEN) return "tooLong";
  const key = (q: string) => q.normalize("NFC").toLowerCase();
  if (list.some((q) => key(q) === key(p))) return "repeat";
  if (list.length >= MAX_PATTERNS) return "tooMany";
  return null;
}
