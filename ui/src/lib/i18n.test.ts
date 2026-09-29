import { afterEach, describe, expect, test } from "vitest";
import { locale, setLocale, t, tParts } from "./i18n";
import en from "../locales/en.json";
import { headline } from "./headline";
import { notificationFor } from "./summaryText";
import { stopMessage } from "./stopping";
import { summaryView } from "../test/fake-api";

describe("t()", () => {
  afterEach(() => setLocale("en"));

  test("a text, with its placeholders filled", () => {
    expect(t("test.plain")).toBe("Plain text");
    expect(t("test.named", { name: "Sony FX3" })).toBe("Hello “Sony FX3”");
  });

  test("plurals use the language's plural rules", () => {
    expect(t("test.files", { count: 1 })).toBe("1 file");
    expect(t("test.files", { count: 0 })).toBe("0 files");
    expect(t("test.files", { count: 1284 })).toBe("1,284 files");
  });

  test("numbers in placeholders use the locale", () => {
    expect(t("test.named", { name: 1234567 })).toBe("Hello “1,234,567”");
  });

  test("a missing key is an error in tests", () => {
    // @ts-expect-error — not a key
    expect(() => t("test.nope")).toThrow("test.nope");
  });

  test("a missing placeholder is an error in tests", () => {
    expect(() => t("test.named")).toThrow("{name}");
  });

  test("tParts keeps the filled values apart, for markup around them", () => {
    expect(tParts("test.named", { name: "Sony FX3" })).toEqual([
      { text: "Hello “", value: false },
      { text: "Sony FX3", value: true },
      { text: "”", value: false },
    ]);
    expect(tParts("test.plain")).toEqual([{ text: "Plain text", value: false }]);
  });

  test("the language is English when there's no catalog for the Mac's", () => {
    expect(locale()).toBe("en");
  });

  /** `text` in capitals, its {placeholders} untouched. */
  const upper = (text: string) => text.replace(/(^|\})([^{]*)/g, (m) => m.toUpperCase());
  
  test("builders use the catalog", () => {
    const plain = { headline: headline(summaryView()), stop: stopMessage("check", true), title: notificationFor(summaryView()).title };
    const shouty = JSON.parse(JSON.stringify(en), (_k, v) => (typeof v === "string" ? upper(v) : v));
    setLocale("en", shouty);
    expect(headline(summaryView())).toBe(plain.headline.toUpperCase());
    expect(stopMessage("check", true)).toBe(plain.stop.toUpperCase());
    expect(notificationFor(summaryView()).title).toBe(plain.title.toUpperCase());
  });
});
