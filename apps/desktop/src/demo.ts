import type { SearchHit } from "./types";
import {
  demoPages,
  demoSnapshot,
} from "./demoWorkspace.generated";

export {
  demoCanvas,
  demoDataApp,
  demoPages,
  demoSnapshot,
  demoTextFiles,
} from "./demoWorkspace.generated";

/**
 * Dev-only stand-in used when the frontend runs in a plain browser
 * (`pnpm dev` / `nix run .#desktop-web`) where the Tauri IPC bridge
 * doesn't exist. Snapshot bodies come from `templates/workspaces/demo/`
 * via `pnpm compile-templates` → `demoWorkspace.generated.ts`. Never
 * bundled into release builds.
 */
export const inBrowser = import.meta.env.DEV && !("__TAURI_INTERNALS__" in window);

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
