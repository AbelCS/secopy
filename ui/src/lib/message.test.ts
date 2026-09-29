import { afterEach, describe, expect, test } from "vitest";
import en from "../locales/en.json";
import { setLocale } from "./i18n";
import type { Message } from "./bindings";
import { AppError, fieldOf, say } from "./message";
import rustKeys from "../locales/rust-keys.json";

describe("messages from the app", () => {
  afterEach(() => setLocale("en"));

  test("a key and its values become words", () => {
    expect(say({ key: "test.named", args: { name: "Sony FX3" } })).toBe("Hello “Sony FX3”");
    expect(say({ key: "test.files", args: { count: 1284 } })).toBe("1,284 files");
  });

  test("a size is formatted, a nested message translated, a list joined", () => {
    expect(say({ key: "test.named", args: { name: { bytes: 212_400_000_000 } } })).toBe("Hello “212.4 GB”");
    expect(say({ key: "test.named", args: { name: { key: "test.plain", args: {} } } })).toBe("Hello “Plain text”");
    const parts: Message[] = [
      { key: "test.files", args: { count: 1 } },
      { key: "format.raw", args: { text: "x" } },
    ];
    expect(say({ key: "test.named", args: { name: parts } })).toBe("Hello “1 file, x”");
  });

  test("a command's error is an Error with the words, keeping its message", () => {
    const m = { key: "test.named", args: { name: "A" } };
    const e = new AppError(m);
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("Hello “A”");
    expect(e.m).toBe(m);
  });

  test("a field's error says which field", () => {
    // Keys of later tasks; a catalog with them.
    setLocale("xx", { ...en, errors: { field: { origin: { notFull: "Full path" } }, save: { file: "Not saved" } } } as never);
    expect(fieldOf(new AppError({ key: "errors.field.origin.notFull", args: {} }))).toBe("origin");
    expect(fieldOf(new AppError({ key: "errors.save.file", args: {} }))).toBeNull();
    expect(fieldOf(new Error("The origin must be a full path"))).toBeNull();
  });

  test("every key the app can send has words, with its placeholders filled", () => {
    for (const key of rustKeys) {
      const entry = key.split(".").reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], en);
      const texts = typeof entry === "string" ? [entry] : Object.values(entry as Record<string, string>);
      const args: Message["args"] = {};
      for (const [, name] of texts.join(" ").matchAll(/\{(\w+)\}/g)) args[name] = name === "count" ? 2 : "x";
      if (typeof entry !== "string") args.count = 2;
      const words = say({ key, args });
      expect(words, key).not.toBe(key);
      expect(words, key).not.toMatch(/\{\w+\}/);
    }
  });
});
