import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";

// Tauri serves the dev build from a fixed port and loads `dist/` in release builds.
export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "safari16", outDir: "dist" },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test/setup.ts"],
  },
});
