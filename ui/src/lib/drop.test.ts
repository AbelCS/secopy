import { describe, expect, test } from "vitest";
import { dropPoint } from "./drop";

describe("dropPoint", () => {
  test("on macOS the position is already in points: a Retina screen doesn't halve it", () => {
    // A drop low in the window (on TO) must stay low, not land on FROM halfway up.
    expect(dropPoint({ x: 400, y: 650 }, 2, true)).toEqual({ x: 400, y: 650 });
  });

  test("elsewhere the position is in pixels and is scaled to points", () => {
    expect(dropPoint({ x: 400, y: 650 }, 2, false)).toEqual({ x: 200, y: 325 });
  });
});
