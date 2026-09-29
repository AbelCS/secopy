// Translation (#84): every text the UI writes comes from a catalog, `ui/src/locales/<lang>.json`.
// Only English exists; the app uses the Mac's language when there's a catalog for it (none yet).
import en from "../locales/en.json";

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

const catalogs: Record<string, Catalog> = { en };

/** The Mac's first language that has a catalog; English otherwise. */
function pick(): string {
  const wanted = typeof navigator === "undefined" ? [] : navigator.languages ?? [navigator.language];
  for (const tag of wanted) {
    const base = tag.split("-")[0];
    if (catalogs[tag]) return tag;
    if (catalogs[base]) return base;
  }
  return "en";
}

let current = pick();
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
  const entry = lookup(key);
  if (entry === undefined) {
    if (strict) throw new Error(`No text for the key ${key}`);
    return key;
  }
  let text: string;
  if (typeof entry === "string") {
    text = entry;
  } else {
    const count = Number(values.count ?? 0);
    const form = plurals.select(count) as keyof Plural;
    text = entry[form] ?? entry.other;
  }
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = values[name];
    if (value === undefined) {
      if (strict) throw new Error(`No value for ${whole} in ${key}`);
      return whole;
    }
    return typeof value === "number" ? numbers.format(value) : value;
  });
}
