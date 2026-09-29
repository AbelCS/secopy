import { afterEach, describe, expect, test } from "vitest";
import en from "../locales/en.json";
import { setLocale } from "./i18n";
import type { Message } from "./bindings";
import { AppError, fieldOf, say } from "./message";

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
});
