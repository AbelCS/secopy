import { expect, test } from "vitest";
import { RateMeter } from "./rate";

test("unknown until two samples", () => {
  const m = new RateMeter();
  expect(m.current()).toBeNull();
  m.push(0, 0);
  expect(m.current()).toBeNull();
  expect(m.eta(100)).toBeNull();
});

test("speed is averaged over the last three seconds", () => {
  const m = new RateMeter();
  // 100 MB/s for 5 s, then 300 MB/s for 3 s.
  for (let t = 0; t <= 5000; t += 500) m.push(t, t * 100_000);
  const at5 = 5000 * 100_000;
  for (let t = 5500; t <= 8000; t += 500) m.push(t, at5 + (t - 5000) * 300_000);
  expect(m.current()).toBeCloseTo(300_000_000, -3);
});

test("eta from the current speed", () => {
  const m = new RateMeter();
  m.push(0, 0);
  m.push(1000, 50_000_000);
  expect(m.eta(100_000_000)).toBe(2000);
  expect(m.eta(0)).toBe(0);
});
