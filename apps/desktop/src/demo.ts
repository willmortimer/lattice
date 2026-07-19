import { ipcMode } from "./lib/ipc";
import type { SearchHit } from "./types";
import {
  demoPages,
  demoSnapshot,
} from "./demoWorkspace.generated";

export {
  demoCanvas,
  demoDataApp,
  demoDataApps,
  demoNotebooks,
  demoPages,
  demoSnapshot,
  demoTextFiles,
} from "./demoWorkspace.generated";

/**
 * Dev-only fixture mode: plain browser without Tauri or `lattice-bridge`.
 * Bridge mode (`VITE_LATTICE_BRIDGE_URL`) talks to real Rust handlers and is
 * not the demo fixture. Snapshot bodies come from `templates/workspaces/demo/`
 * via `pnpm compile-templates` → `demoWorkspace.generated.ts`. Never bundled
 * into release builds.
 */
export const inBrowser = import.meta.env.DEV && ipcMode === "demo";

/** `?empty` reviews the empty state instead of the demo workspace. */
export const demoStartEmpty =
  inBrowser && new URLSearchParams(window.location.search).has("empty");

/** @deprecated Prefer `demoPages[path]`; kept for older call sites. */
export const demoPage = demoPages["Research/Competitor Analysis.md"];

/**
 * Demo search: substring match over fixture paths and known page bodies.
 */
export function demoSearch(query: string): SearchHit[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return [];

  const hits: SearchHit[] = [];
  for (const resource of demoSnapshot.resources) {
    if (resource.kind !== "page") continue;
    const body = demoPages[resource.path] ?? "";
    const hay = `${resource.path}\n${body}`.toLowerCase();
    if (!hay.includes(trimmed)) continue;
    hits.push({
      path: resource.path,
      title: resource.path.split("/").pop() ?? resource.path,
      snippet: null,
      rank: 0,
    });
  }
  return hits;
}
