// @vitest-environment node
// Reads the tokens from app.css; no DOM needed (happy-dom's URL can't open files).
import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";

const css = readFileSync(new URL("./app.css", import.meta.url), "utf8");
const tokens = Object.fromEntries([...css.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/gi)].map((m) => [m[1], m[2]]));

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const f = (c: number) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}
const contrast = (a: string, b: string) => {
  const [hi, lo] = [luminance(tokens[a]), luminance(tokens[b])].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

describe("colour tokens (WCAG AA, RFD §5.6)", () => {
  const surfaces = ["bg", "surface", "surface-raised"];
  for (const text of ["text", "text-muted", "accent", "success", "warning", "danger"]) {
    for (const surface of surfaces) {
      test(`${text} on ${surface}`, () => expect(contrast(text, surface)).toBeGreaterThanOrEqual(4.5));
    }
  }
  test("on-accent on accent-strong (filled buttons)", () =>
    expect(contrast("on-accent", "accent-strong")).toBeGreaterThanOrEqual(4.5));
});
