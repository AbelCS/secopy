// Translation (#84): every text the UI writes comes from a catalog, `ui/src/locales/<lang>.json`.
// The app uses the Mac's language when there's a catalog for it: English and Spanish.
import en from "../locales/en.json";
import es from "../locales/es.json";

export type Catalog = typeof en;
type Plural = { one: string; other: string; zero?: string; two?: string; few?: string; many?: string };
type Entry = string | Plural;

/** Every leaf key of the catalog: "copy.start", …; a plural's forms are one leaf. */
type Leaves<T, P extends string = ""> = {
  [K in keyof T & string]: T[K] extends string
    ? `${P}${K}`
    : T[K] extends { one: string; other: string }
      ? `${P}${K}`
      : Leaves<T[K], `${P}${K}.`>;
}[keyof T & string];
export type Key = Leaves<Catalog>;
export type Values = Record<string, string | number>;

// A translation's plurals have the forms its language uses, so its shape differs from English;
// a test checks its keys and placeholders.
const catalogs: Record<string, Catalog> = { en, es: es as unknown as Catalog };

/** The first of `wanted` (the Mac's languages, in order) that has a catalog; English otherwise. */
export function languageFor(wanted: readonly string[]): string {
  for (const tag of wanted) {
    const base = tag.split("-")[0];
    if (catalogs[tag]) return tag;
    if (catalogs[base]) return base;
  }
  return "en";
}

let current = languageFor(
  typeof navigator === "undefined" ? [] : (navigator.languages ?? [navigator.language]),
);
let catalog: Catalog = catalogs[current];
let plurals = new Intl.PluralRules(current);
let numbers = new Intl.NumberFormat(current);

export function locale(): string {
  return current;
}

/** Switches the language (tests, and a language setting one day). */
export function setLocale(tag: string, next: Catalog = catalogs[tag] ?? en): void {
  current = tag;
  catalog = next;
  plurals = new Intl.PluralRules(tag);
  numbers = new Intl.NumberFormat(tag);
}

const strict = import.meta.env.MODE === "test";

function lookup(key: string): Entry | undefined {
  let node: unknown = catalog;
  for (const part of key.split(".")) {
    if (node === null || typeof node !== "object") return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return typeof node === "string" || (node && typeof node === "object" && "other" in node)
    ? (node as Entry)
    : undefined;
}

/** The text for `key`, its `{placeholders}` filled from `values` (numbers in the locale's
 *  format). A plural entry picks its form from `values.count`. A missing key or placeholder
 *  throws in tests; in the app it shows the key or the placeholder, never a blank. */
export function t(key: Key, values: Values = {}): string {
  return tParts(key, values)
    .map((p) => p.text)
    .join("");
}

/** As `t()`, in pieces: the filled values apart from the words around them, so a screen can
 *  style a value (a file name in mono) inside a translated sentence. */
export function tParts(key: Key, values: Values = {}): { text: string; value: boolean }[] {
  const entry = lookup(key);
  if (entry === undefined) {
    if (strict) throw new Error(`No text for the key ${key}`);
    return [{ text: key, value: false }];
  }
  let text: string;
  if (typeof entry === "string") {
    text = entry;
  } else {
    const count = Number(values.count ?? 0);
    const form = plurals.select(count) as keyof Plural;
    text = entry[form] ?? entry.other;
  }
  const parts: { text: string; value: boolean }[] = [];
  let last = 0;
  for (const m of text.matchAll(/\{(\w+)\}/g)) {
    const value = values[m[1]];
    if (value === undefined && strict) throw new Error(`No value for ${m[0]} in ${key}`);
    if (m.index > last) parts.push({ text: text.slice(last, m.index), value: false });
    if (value === undefined) parts.push({ text: m[0], value: false });
    else parts.push({ text: typeof value === "number" ? numbers.format(value) : value, value: true });
    last = m.index + m[0].length;
  }
  if (last < text.length || parts.length === 0) parts.push({ text: text.slice(last), value: false });
  return parts;
}
