// Settings › Always ignore when copying (#158): the checks the app makes on a typed pattern,
// the same the engine makes (crates/secopy-core/src/ignore.rs).

/** Characters a pattern has at most. */
export const MAX_LEN = 255;
/** Patterns the list holds at most. */
export const MAX_PATTERNS = 200;

/** A pattern as shown: a carriage return (macOS's "Icon\r") as ␍. */
export function shownPattern(p: string): string {
  return p.replaceAll("\r", "␍");
}

/** Why `pattern` can't be added to `list`, or null. Spaces at its ends don't count. */
export function patternProblem(pattern: string, list: string[]): "slash" | "tooLong" | "tooMany" | "repeat" | null {
  const p = pattern.replace(/^ +| +$/g, "");
  if (p.includes("/")) return "slash";
  if ([...p].length > MAX_LEN) return "tooLong";
  if (list.some((q) => q.toLowerCase() === p.toLowerCase())) return "repeat";
  if (list.length >= MAX_PATTERNS) return "tooMany";
  return null;
}
