// Every word on every screen comes from the catalog (#84): with a catalog whose texts are all
// wrapped in ⟦…⟧, no visible word is left outside the brackets, except data (paths, names).
import { render } from "@testing-library/svelte";
import { afterEach, describe, expect, test } from "vitest";
import en from "./locales/en.json";
import { setLocale, type Catalog } from "./lib/i18n";
import Gallery from "./gallery/Gallery.svelte";

function pseudo(node: unknown): unknown {
  if (typeof node === "string") return node === "" ? node : `⟦${node}⟧`;
  return Object.fromEntries(Object.entries(node as Record<string, unknown>).map(([k, v]) => [k, pseudo(v)]));
}

/** `text` without its ⟦…⟧ parts, nested ones too. */
function outside(text: string): string {
  let out = text;
  for (let before = ""; before !== out; ) {
    before = out;
    out = out.replace(/⟦[^⟦⟧]*⟧/g, "");
  }
  return out;
}

/** Visible text outside ⟦…⟧ that has letters, with where it is. */
function unwrapped(root: HTMLElement): string[] {
  const found: string[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const el = n.parentElement!;
    // translate="no": data the user typed, like ignore patterns (#161).
    if (el.closest('script, style, .visually-hidden[aria-hidden], [translate="no"]')) continue;
    // Brackets can open and close in other text nodes of the same element.
    const text = outside(el.textContent ?? "");
    const own = (n.textContent ?? "").trim();
    if (own && /\p{L}/u.test(text) && !own.includes("⟦") && !own.includes("⟧")) found.push(own);
  }
  for (const el of root.querySelectorAll<HTMLElement>("[aria-label], [title], [placeholder]")) {
    for (const name of ["aria-label", "title", "placeholder"]) {
      const v = el.getAttribute(name);
      if (v && /\p{L}/u.test(outside(v))) found.push(`${name}="${v}"`);
    }
  }
  return found;
}

/** Data, not words: paths, file names and types, checksums, and the fixtures' preset names. */
function isData(text: string): boolean {
  const t = text.replace(/^(title|aria-label)="(.*)"$/, "$2").replace(/^\u200E/, "");
  return (
    t.startsWith("/") ||
    /\.[\w…]{1,8}$/.test(t) ||
    /^[0-9a-f]{16}$/.test(t) ||
    ["Sony FX3", "DJI Mini 4", "Old camera", "Footage", "Footage → NAS", "Photos → Backup", "xxhsum -c"].includes(t)
  );
}

// The gallery's own pages ("components", "tabs") show demo labels; every app screen is here.
const PAGES = ["setup", "progress", "summary", "settings", "presets", "presets-empty", "queue",
  "queue-summary", "mirror", "mirror-preview", "mirror-summary", "mirroring", "cancel", "verify", "verify-summary",
  "import", "export", "panel", "panel-done"];

describe("every word comes from the catalog", () => {
  afterEach(() => setLocale("en"));
  for (const page of PAGES) {
    test(page, async () => {
      setLocale("xx", pseudo(en) as Catalog);
      window.location.hash = page;
      const { container } = render(Gallery);
      await new Promise((r) => setTimeout(r, 50));
      expect(unwrapped(container).filter((text) => !isData(text))).toEqual([]);
    });
  }
});
