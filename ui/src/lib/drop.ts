// Where a Finder drop landed, in the page's coordinates (CSS pixels).

/**
 * Tauri labels a drop's position as physical pixels, but on macOS wry reports it in points,
 * which are already the page's coordinates. Scaling it again on a Retina screen halved it,
 * so a drop on TO was looked up inside FROM (#29).
 */
export function dropPoint(
  position: { x: number; y: number },
  scale: number,
  macOS: boolean,
): { x: number; y: number } {
  return macOS ? { x: position.x, y: position.y } : { x: position.x / scale, y: position.y / scale };
}

/** Whether the app runs on macOS (the web view's user agent says so). */
export const isMacOS = (): boolean => /Mac OS X|Macintosh/.test(navigator.userAgent);
