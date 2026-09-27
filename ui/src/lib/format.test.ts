import { describe, expect, test } from "vitest";
import { formatBytes, formatCount, formatDuration, formatPercent, formatSpeed, plural } from "./format";

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
    expect(formatPercent(0, 0)).toBe("100.0 %");
    expect(formatPercent(1482, 2124)).toBe("69.8 %");
  });

  test("plural", () => {
    expect(plural(1, "file")).toBe("1 file");
    expect(plural(1284, "file")).toBe("1,284 files");
  });
});
