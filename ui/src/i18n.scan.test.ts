import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import en from "./locales/en.json";

/** Visible text a component writes itself: text between tags, the values of attributes that
 *  show words when they hold letters outside their {expressions}, and string literals in
 *  markup expressions that look like words (a space, or a first letter in capitals:
 *  "(no source yet)"; codes such as "mirror" and keys such as "a.b" pass). Symbols alone ("→", "·", "…") are fine. */
/** Keyboard keys compared in markup (`e.key === "Escape"`): names, not words. */
const KEYS = new Set(["Escape", "Enter", "Tab", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"]);

export function hardCoded(source: string): string[] {
  const markup = source
    .replace(/<script[\s\S]*?<\/script>/g, "")
    .replace(/<style[\s\S]*?<\/style>/g, "")
    .replace(/<!--[\s\S]*?-->/g, "");
  // Split {expressions} (including nested braces) from the plain markup.
  let plain = "";
  let code = "";
  let depth = 0;
  for (const ch of markup) {
    if (ch === "{") {
      if (depth > 0) code += ch;
      depth += 1;
    } else if (ch === "}") {
      depth = Math.max(0, depth - 1);
      code += depth > 0 ? ch : "\n";
    } else if (depth === 0) plain += ch;
    else code += ch;
  }
  const found: string[] = [];
  for (const m of plain.matchAll(/>([^<>]+)</g)) {
    const text = m[1].trim();
    if (/\p{L}/u.test(text)) found.push(text);
  }
  const attrs = /\b(label|aria-label|placeholder|title|help|legend|alt|text|status|hint|error|meta)="([^"]*)"/g;
  for (const m of markup.matchAll(attrs)) {
    if (/\p{L}/u.test(m[2].replace(/\{[^}]*\}/g, ""))) found.push(`${m[1]}="${m[2]}"`);
  }
  for (const m of code.matchAll(/"([^"\n]*)"|'([^'\n]*)'|`([^`]*)`/g)) {
    const text = (m[1] ?? m[2] ?? m[3])
      .replace(/\$\{[^}]*\}/g, "")
      .replace(/\\u[0-9a-fA-F]{4}/g, "")
      .trim();
    if (KEYS.has(text)) continue;
    const letter = text.match(/\p{L}/u)?.[0];
    if (letter && (/\s/.test(text) || letter !== letter.toLowerCase())) found.push(m[0]);
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
    expect(hardCoded(`<button aria-label="Remove {label}">✕</button>`)).toEqual([`aria-label="Remove {label}"`]);
    expect(hardCoded(`<FormRow hint="Two words" error="Bad" meta="Some" />`)).toHaveLength(3);
    expect(hardCoded(`<p>{p.source ? p.source : "(no source yet)"}</p>`)).toEqual([`"(no source yet)"`]);
    expect(hardCoded(`<p>{t(x ? "a.b" : "a.c")}</p>{#if kind === "mirror"}{/if}`)).toEqual([]);
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
