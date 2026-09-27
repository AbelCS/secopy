import { expect, test } from "vitest";
import { render, screen } from "@testing-library/svelte";
import App from "./App.svelte";

test("shows the app name", () => {
  render(App);
  expect(screen.getByRole("heading", { name: "Secopy" })).toBeTruthy();
});
