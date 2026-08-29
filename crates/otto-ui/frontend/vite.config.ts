/// <reference types="vitest/config" />
import path from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// https://vite.dev/config/ - port/strictPort/clearScreen follow Tauri's own
// recommended Vite config so `tauri dev`'s terminal output stays readable and
// the dev server fails fast instead of silently drifting to another port
// that tauri.conf.json's devUrl wouldn't find.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    // @testing-library/react's auto-cleanup-after-each-test registers
    // itself against a global `afterEach` - without `globals: true` here it
    // never fires, so components from one test stay mounted into the next.
    globals: true,
  },
});
