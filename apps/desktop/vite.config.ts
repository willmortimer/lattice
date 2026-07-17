import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

// Tauri expects a fixed dev server port and a build that ignores its own
// src-tauri directory. See https://v2.tauri.app/start/frontend/vite/
export default defineConfig(async () => ({
  plugins: [react()],

  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  worker: {
    format: "es",
  },
  build: {
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        "quick-note": resolve(__dirname, "quick-note.html"),
      },
    },
  },
}));
