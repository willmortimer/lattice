import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createRequire } from "node:module";
import { resolve } from "node:path";

const require = createRequire(import.meta.url);

// WKWebView (Tauri on macOS) rejects default imports from CJS interop modules
// that Vite leaves as star-export wrappers. The datagrid ESM entry pulls in
// chroma-js that way; the CDN build is self-contained and loads cleanly.
const perspectiveDatagridCdn = require.resolve(
  "@finos/perspective-viewer-datagrid/dist/cdn/perspective-viewer-datagrid.js",
);

// Tauri expects a fixed dev server port and a build that ignores its own
// src-tauri directory. See https://v2.tauri.app/start/frontend/vite/
export default defineConfig(async () => ({
  plugins: [react()],

  clearScreen: false,
  server: {
    // 0.0.0.0 so DevCell / Docker published ports and Tailscale Serve work.
    // Local Tauri still reaches the server via http://localhost:5173.
    host: true,
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  worker: {
    format: "es",
  },
  resolve: {
    alias: {
      "@finos/perspective-viewer-datagrid": perspectiveDatagridCdn,
    },
  },
  // Perspective WASM modules require modern syntax (top-level await / esnext).
  build: {
    target: "esnext",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        "quick-note": resolve(__dirname, "quick-note.html"),
      },
    },
  },
  optimizeDeps: {
    exclude: [
      "@finos/perspective",
      "@finos/perspective-viewer",
      "@finos/perspective-viewer-datagrid",
    ],
  },
  assetsInclude: ["**/*.wasm"],
}));
