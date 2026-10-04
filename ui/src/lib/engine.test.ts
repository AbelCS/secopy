// The engine's names and limits the UI keeps a copy of are the engine's (code review, #192).
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "vitest";
import { ARCHIVE_DIR } from "./engine";
import { MAX_LEN, MAX_PATTERNS } from "./patterns";

const core = (file: string) => readFileSync(join(__dirname, "../../../crates/secopy-core/src", file), "utf8");
const rust = (file: string, name: string) => core(file).match(new RegExp(`pub const ${name}: [^=]+= ([^;]+);`))?.[1];

test("the ignore list's limits are the engine's", () => {
  expect(String(MAX_LEN)).toBe(rust("ignore.rs", "MAX_LEN"));
  expect(String(MAX_PATTERNS)).toBe(rust("ignore.rs", "MAX_PATTERNS"));
});

test("a mirror's archive directory is the engine's", () => {
  expect(JSON.stringify(ARCHIVE_DIR)).toBe(rust("mirror.rs", "ARCHIVE_DIR"));
});
