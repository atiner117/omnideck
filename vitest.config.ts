import { defineConfig } from "vitest/config";

// Minimal, standalone vitest config (not the SvelteKit vite.config) — the current suite is
// pure TypeScript logic extracted from the page, so no Svelte plugin or DOM env is needed.
// Add `environment: "jsdom"` + the svelte plugin here when component tests arrive.
export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
