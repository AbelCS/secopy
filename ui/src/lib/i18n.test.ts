import { afterEach, describe, expect, test } from "vitest";
import { locale, setLocale, t } from "./i18n";

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

  test("the language is English when there's no catalog for the Mac's", () => {
    expect(locale()).toBe("en");
  });
});
