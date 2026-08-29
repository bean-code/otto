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
});
