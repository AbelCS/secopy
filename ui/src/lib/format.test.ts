import { afterEach, describe, expect, test } from "vitest";
import { setLocale } from "./i18n";
import en from "../locales/en.json";
import { formatBytes, formatCount, formatDuration, formatPercent, formatSpeed } from "./format";

describe("format", () => {
  test("counts use thousands separators", () => {
    expect(formatCount(1284)).toBe("1,284");
  });

  test("bytes use decimal units like Finder", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1000)).toBe("1.0 KB");
    expect(formatBytes(212_400_000_000)).toBe("212.4 GB");
  });

  test("speed and duration show a dash when unknown", () => {
    expect(formatSpeed(null)).toBe("—");
    expect(formatSpeed(1_210_000_000)).toBe("1.2 GB/s");
    expect(formatDuration(null)).toBe("—");
    expect(formatDuration(7_000)).toBe("0:07");
    expect(formatDuration(252_000)).toBe("4:12");
    expect(formatDuration(3_723_000)).toBe("1:02:03");
  });

  test("percent treats an empty total as done", () => {
    expect(formatPercent(0, 0)).toBe("100.0%");
    expect(formatPercent(1482, 2124)).toBe("69.8%");
  });

  test("halves round as they always did (toFixed), not the locale's way", () => {
    expect(formatBytes(1_150_000)).toBe("1.1 MB");
    expect(formatBytes(1_450_000)).toBe("1.4 MB");
    expect(formatPercent(23, 2000)).toBe("1.1%");
  });
});

describe("formatting follows the locale", () => {
  afterEach(() => setLocale("en"));

  test("English is exactly as before", () => {
    expect(formatCount(1284)).toBe("1,284");
    expect(formatBytes(212_400_000_000)).toBe("212.4 GB");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatSpeed(1_200_000_000)).toBe("1.2 GB/s");
    expect(formatSpeed(null)).toBe("—");
    expect(formatPercent(698, 1000)).toBe("69.8%");
  });

  test("another locale changes the separators", () => {
    setLocale("de", en);
    expect(formatCount(1284)).toBe("1.284");
    expect(formatBytes(212_400_000_000)).toBe("212,4 GB");
  });
});
