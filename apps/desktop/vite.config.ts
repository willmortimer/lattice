import { existsSync, lstatSync, realpathSync } from "node:fs";
import { createRequire } from "node:module";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { defineConfig, searchForWorkspaceRoot } from "vite";
import react from "@vitejs/plugin-react";

const require = createRequire(import.meta.url);

// WKWebView (Tauri on macOS) rejects default imports from CJS interop modules
// that Vite leaves as star-export wrappers. The datagrid ESM entry pulls in
// chroma-js that way; the CDN build is self-contained and loads cleanly.
const perspectiveDatagridCdn = require.resolve(
  "@finos/perspective-viewer-datagrid/dist/cdn/perspective-viewer-datagrid.js",
);

const repoRoot = resolve(__dirname, "../..");

/**
 * Vite's fs.allow defaults to the realpath workspace root. When this checkout
 * is also reached via a symlink (e.g. ~/Developer/lattice → ecosystem/lattice),
 * font/CSS request ids can use the symlink prefix and get 403'd — blank
 * WebView typography / missing xterm.css. Allow both path forms.
 */
function viteFsAllow(): string[] {
  const allow = new Set<string>([
    searchForWorkspaceRoot(process.cwd()),
    repoRoot,
  ]);

  const add = (path: string | undefined) => {
    if (!path) return;
    try {
      if (existsSync(path)) allow.add(resolve(path));
    } catch {
      // ignore missing candidates
    }
  };

  add(process.cwd());
  try {
    allow.add(realpathSync(repoRoot));
  } catch {
    // ignore
  }
  try {
    allow.add(realpathSync(process.cwd()));
  } catch {
    // ignore
  }

  // Common local alias into this nested checkout.
  add(join(homedir(), "Developer", "lattice"));

  // If cwd walks through a symlink that realpaths to the repo, allow that prefix.
  let dir = process.cwd();
  for (let i = 0; i < 8; i++) {
    try {
      if (lstatSync(dir).isSymbolicLink()) {
        const target = realpathSync(dir);
        const realRepo = realpathSync(repoRoot);
        if (target === realRepo || target.startsWith(`${realRepo}/`)) {
          allow.add(dir);
        }
      }
    } catch {
      // ignore
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }

  return [...allow];
}

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
    fs: {
      allow: viteFsAllow(),
    },
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
