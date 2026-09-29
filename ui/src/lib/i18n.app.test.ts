import { afterEach, expect, test, vi } from "vitest";

// Outside tests nothing throws: a missing key shows the key, a missing value its placeholder,
// so a screen never goes blank.
afterEach(() => {
  vi.unstubAllEnvs();
  vi.resetModules();
});

async function appI18n() {
  vi.stubEnv("MODE", "production");
  vi.resetModules();
  return await import("./i18n");
}

test("in the app, a missing key shows the key", async () => {
  const { t, tParts } = await appI18n();
  // @ts-expect-error — not a key
  expect(t("test.nope")).toBe("test.nope");
  // @ts-expect-error — not a key
  expect(tParts("test.nope")).toEqual([{ text: "test.nope", value: false }]);
});

test("in the app, a missing value shows its placeholder", async () => {
  const { t } = await appI18n();
  expect(t("test.named")).toBe("Hello “{name}”");
});
