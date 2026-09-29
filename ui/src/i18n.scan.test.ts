import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import en from "./locales/en.json";

/** Visible text a component writes itself: text between tags, and the values of attributes
 *  that show words, when they hold letters. Symbols alone ("→", "·", "…", "✓", "✗") are fine. */
export function hardCoded(source: string): string[] {
  const markup = source
    .replace(/<script[\s\S]*?<\/script>/g, "")
    .replace(/<style[\s\S]*?<\/style>/g, "")
    .replace(/<!--[\s\S]*?-->/g, "");
  // Drop {expressions}, including nested braces.
  let plain = "";
  let depth = 0;
  for (const ch of markup) {
    if (ch === "{") depth += 1;
    else if (ch === "}") depth = Math.max(0, depth - 1);
    else if (depth === 0) plain += ch;
  }
  const found: string[] = [];
  for (const m of plain.matchAll(/>([^<>]+)</g)) {
    const text = m[1].trim();
    if (/\p{L}/u.test(text)) found.push(text);
  }
  const attrs = /\b(label|aria-label|placeholder|title|help|legend|alt|text|status)="([^"]*)"/g;
  for (const m of markup.matchAll(attrs)) {
    if (/\p{L}/u.test(m[2]) && !m[2].includes("{")) found.push(`${m[1]}="${m[2]}"`);
  }
  return found;
}

const ROOT = join(__dirname);
function components(dir = ROOT): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === "gallery" || name === "test" ? [] : components(path);
    return name.endsWith(".svelte") ? [path.slice(ROOT.length + 1)] : [];
  });
}

describe("no text written directly in components", () => {
  test("the scan finds written text and ignores expressions", () => {
    expect(hardCoded(`<p>Hello</p><p>{t("x")}</p><p> → </p>`)).toEqual(["Hello"]);
    expect(hardCoded(`<Button label="Save" title={t("a")} />`)).toEqual([`label="Save"`]);
    expect(hardCoded(`<script>const a = "<p>Hi</p>";</script><style>p{}</style>`)).toEqual([]);
  });

  for (const file of components()) {
    test(file, () => {
      const found = hardCoded(readFileSync(join(ROOT, file), "utf8"));
      expect(found, `${file} writes text itself; use t()`).toEqual([]);
    });
  }
});

function leaves(node: unknown, prefix = ""): string[] {
  if (typeof node === "string") return [prefix];
  if (node && typeof node === "object" && "other" in node) return [prefix];
  return Object.entries(node as Record<string, unknown>).flatMap(([k, v]) => leaves(v, prefix ? `${prefix}.${k}` : k));
}

function sources(dir = ROOT): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === "test" ? [] : sources(path);
    return /\.(svelte|ts)$/.test(name) && !/\.test\.ts$/.test(name) ? [readFileSync(path, "utf8")] : [];
  });
}

test("every word in the catalog is used", () => {
  const code = sources().join("\n");
  const unused = leaves(en).filter((key) => !key.startsWith("test.") && !code.includes(`"${key}"`));
  expect(unused).toEqual([]);
});
