// Every translation has the English catalog's keys and placeholders, and its plurals use the
// forms its language has (#84, Spanish).
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, test } from "vitest";
import en from "../locales/en.json";
import { languageFor, languages, setLocale, t } from "./i18n";
import { formatBytes, formatPercent } from "./format";

type Node = string | { [k: string]: Node };
const FORMS = ["zero", "one", "two", "few", "many", "other"];
const isPlural = (n: Node): n is Record<string, string> =>
  typeof n === "object" && Object.keys(n).every((k) => FORMS.includes(k)) && "other" in n;

/** Leaf key → its texts (one, or a plural's forms). */
function leaves(node: Node, prefix = "", out = new Map<string, Record<string, string>>()) {
  if (typeof node === "string") out.set(prefix, { text: node });
  else if (isPlural(node)) out.set(prefix, node);
  else for (const [k, v] of Object.entries(node)) leaves(v, prefix ? `${prefix}.${k}` : k, out);
  return out;
}
const placeholders = (text: string) => new Set([...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]));

const dir = join(__dirname, "../locales");
const translations = readdirSync(dir).filter((f) => f.endsWith(".json") && !["en.json", "rust-keys.json"].includes(f));

test("there is a translation", () => {
  expect(translations).toContain("es.json");
});

describe.each(translations)("%s", (file) => {
  const tag = file.replace(".json", "");
  const theirs = leaves(JSON.parse(readFileSync(join(dir, file), "utf8")));
  const ours = leaves(en as Node);

  test("has every English key, and no other", () => {
    expect([...theirs.keys()].sort()).toEqual([...ours.keys()].sort());
  });

  test("keeps the English spaces at either end (separators like “ · ”)", () => {
    const edges = (s: string) => [/^\s/.test(s), /\s$/.test(s)];
    const wrong = [...theirs].filter(([key, texts]) => {
      const english = ours.get(key)?.text;
      return english !== undefined && JSON.stringify(edges(texts.text)) !== JSON.stringify(edges(english));
    });
    expect(wrong.map(([key]) => key)).toEqual([]);
  });

  test("uses the English placeholders, and a plural only where there's a count", () => {
    const wrong: string[] = [];
    const forms = new Intl.PluralRules(tag).resolvedOptions().pluralCategories as string[];
    for (const [key, texts] of theirs) {
      const english = Object.values(ours.get(key) ?? {});
      const allowed = new Set(english.flatMap((s) => [...placeholders(s)]));
      // An English plural is picked by its count, said or not.
      if (!("text" in (ours.get(key) ?? {}))) allowed.add("count");
      for (const [form, text] of Object.entries(texts)) {
        for (const p of placeholders(text)) if (!allowed.has(p)) wrong.push(`${key}.${form}: {${p}}`);
        if (form !== "text" && !forms.includes(form)) wrong.push(`${key}: no form “${form}” in ${tag}`);
      }
      if (!("text" in texts) && !allowed.has("count")) wrong.push(`${key}: a plural without {count}`);
      // A plural has every form its language needs (Spanish: one and other), never "1 tareas".
      if (!("text" in texts)) for (const f of forms) if (f !== "many" && !(f in texts)) wrong.push(`${key}: no “${f}”`);
      // Every English placeholder is said somewhere (a form may leave out {count}: "one file").
      const said = new Set(Object.values(texts).flatMap((s) => [...placeholders(s)]));
      for (const p of allowed) if (p !== "count" && !said.has(p)) wrong.push(`${key}: no {${p}}`);
    }
    expect(wrong).toEqual([]);
  });
});

describe("Spanish", () => {
  afterEach(() => setLocale("en"));

  test("#181: each language is named in its own words", () => {
    setLocale("es");
    expect(languages()).toEqual([
      { tag: "en", name: "English" },
      { tag: "es", name: "Español" },
    ]);
  });

  test("a Mac in Spanish gets Spanish; other languages English", () => {
    expect(languageFor(["es-ES", "en-US"])).toBe("es");
    expect(languageFor(["es-419"])).toBe("es");
    expect(languageFor(["de-DE", "es-ES"])).toBe("es");
    expect(languageFor(["de-DE"])).toBe("en");
  });

  test("words, plurals and numbers are Spanish", () => {
    setLocale("es");
    expect(t("test.files", { count: 1 })).toBe("1 archivo");
    expect(t("test.files", { count: 12845 })).toBe("12.845 archivos");
    expect(t("format.days", { count: 1 })).toBe("1 día");
    expect(formatBytes(212_400_000_000)).toBe("212,4 GB");
    expect(formatPercent(698, 1000)).toBe("69,8 %");
  });
});
