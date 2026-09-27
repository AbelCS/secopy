import { describe, expect, test } from "vitest";
import { relativeToDrive } from "./drive";

describe("relativeToDrive", () => {
  test("a folder on a card is given relative to the card", () => {
    expect(relativeToDrive("/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP")).toBe("PRIVATE/M4ROOT/CLIP");
    expect(relativeToDrive("/Volumes/CARD_A/DCIM/")).toBe("DCIM");
  });

  test("the card itself is the empty folder", () => {
    expect(relativeToDrive("/Volumes/CARD_A")).toBe("");
  });

  test("a folder that isn't on a card or drive has none", () => {
    expect(relativeToDrive("/Users/me/Desktop/A")).toBeNull();
    expect(relativeToDrive("/Volumes")).toBeNull();
  });
});
