import { expect, test } from "vitest";
import { unwrap } from "./api";

test("ok results give their data", async () => {
  await expect(unwrap(Promise.resolve({ status: "ok", data: 42 }))).resolves.toBe(42);
});

test("error results throw the app's message", async () => {
  const failed = unwrap(Promise.resolve({ status: "error", error: "The destination is not an existing folder" }));
  await expect(failed).rejects.toThrow("The destination is not an existing folder");
});
