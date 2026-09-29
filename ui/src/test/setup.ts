// Runs after every test: a message from the app shown without `say()` reads "[object Object]".
import { cleanup } from "@testing-library/svelte";
import { afterEach } from "vitest";

afterEach(() => {
  if (typeof document === "undefined") return;
  const shown = document.body.innerHTML.includes("[object Object]");
  cleanup();
  if (shown) throw new Error("A message from the app is shown as [object Object]: show it with say().");
});
