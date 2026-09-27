// Where a folder is on its card or drive, for a profile's folder (FR-38).

/** "/Volumes/CARD/PRIVATE/M4ROOT/CLIP" → "PRIVATE/M4ROOT/CLIP"; the card itself → "";
 *  `null` for a folder that isn't on a card or drive. */
export function relativeToDrive(path: string): string | null {
  const parts = path.split("/").filter(Boolean);
  if (parts[0] !== "Volumes" || parts.length < 2) return null;
  return parts.slice(2).join("/");
}
