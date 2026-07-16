import type { SearchHit, WorkspaceSnapshot } from "./types";
import type { DataAppSnapshot } from "./data/types";

/**
 * Dev-only stand-in used when the frontend runs in a plain browser
 * (`pnpm dev` / `nix run .#desktop-web`) where the Tauri IPC bridge
 * doesn't exist. Mirrors the `demo` workspace template + Lattice home
 * path layout so browser review matches first-run desktop. Never bundled
 * into release builds.
 */
export const inBrowser = import.meta.env.DEV && !("__TAURI_INTERNALS__" in window);

/** `?empty` reviews the empty state instead of the demo workspace. */
export const demoStartEmpty =
  inBrowser && new URLSearchParams(window.location.search).has("empty");

/** Matches `~/Lattice/Workspaces/Personal` after `ensure_lattice_home`. */
export const demoSnapshot: WorkspaceSnapshot = {
  root: "/Users/you/Lattice/Workspaces/Personal",
  title: "Personal",
  id: "0198-demo",
  resources: [
    { path: "Home.md", kind: "page" },
    { path: "Inbox", kind: "folder" },
    { path: "Projects", kind: "folder" },
    { path: "Product", kind: "folder" },
    { path: "Product/Vision.md", kind: "page" },
    { path: "Product/Roadmap.md", kind: "page" },
    { path: "Research", kind: "folder" },
    { path: "Research/Competitor Analysis.md", kind: "page" },
    { path: "Notebooks", kind: "folder" },
    { path: "Canvases", kind: "folder" },
    { path: "Canvases/Product Strategy.canvas", kind: "canvas" },
    { path: "CRM.data", kind: "data-app" },
    { path: "Resources", kind: "folder" },
    { path: "Archive", kind: "folder" },
  ],
};

/**
 * `Canvases/Product Strategy.canvas` — same shape as the demo template seed.
 */
export const demoCanvas = {
  nodes: [
    {
      id: "intro",
      type: "text",
      x: 60,
      y: 60,
      width: 260,
      height: 120,
      text: "Sample canvas — double-click a file node to open it.",
    },
    {
      id: "vision",
      type: "file",
      file: "Product/Vision.md",
      x: 360,
      y: 60,
      width: 220,
      height: 120,
    },
    {
      id: "roadmap",
      type: "file",
      file: "Product/Roadmap.md",
      x: 620,
      y: 60,
      width: 220,
      height: 120,
    },
  ],
  edges: [
    { id: "e1", fromNode: "intro", toNode: "vision" },
    { id: "e2", fromNode: "vision", toNode: "roadmap" },
  ],
};

/** `CRM.data` — in-memory grid for browser review. */
export const demoDataApp: DataAppSnapshot = {
  title: "CRM",
  default_table: "contacts",
  package_revision: "demo:0",
  columns: [
    { name: "id", field_type: "text", sqlite_type: "TEXT" },
    { name: "name", field_type: "text", sqlite_type: "TEXT" },
    { name: "status", field_type: "text", sqlite_type: "TEXT" },
  ],
  rows: [
    {
      id: "0198-demo-ada",
      values: {
        id: { Text: "0198-demo-ada" },
        name: { Text: "Ada Lovelace" },
        status: { Text: "Active" },
      },
    },
    {
      id: "0198-demo-grace",
      values: {
        id: { Text: "0198-demo-grace" },
        name: { Text: "Grace Hopper" },
        status: { Text: "Active" },
      },
    },
  ],
  available_views: ["All"],
  active_view: "All",
  filters: [],
};

/** Body used when opening demo pages in the browser shell. */
export const demoPages: Record<string, string> = {
  "Home.md": `---
title: Home
---

# Home

A sample Lattice workspace. Try search (**⌘K**), the canvas under \`Canvases/\`, and quick notes (**⌘N**).

## Map

| Path | Kind |
| --- | --- |
| [[Product/Vision]] | page |
| [[Product/Roadmap]] | page |
| [[Research/Competitor Analysis]] | page |
| \`Canvases/Product Strategy.canvas\` | canvas |
`,
  "Product/Vision.md": `---
title: Vision
---

# Vision

A fast local workspace that treats documents, data, notebooks, and canvases as ordinary files.

See also [[Product/Roadmap]] and [[Research/Competitor Analysis]].
`,
  "Product/Roadmap.md": `---
title: Roadmap
---

# Roadmap

1. Daily-driver editing and search
2. First data-app surface
3. Capture from anywhere

Back to [[Home]].
`,
  "Research/Competitor Analysis.md": `---
title: Competitor Analysis
tags: [research]
---

# Competitor Analysis

| Tool | Keeps | Traps |
| --- | --- | --- |
| Obsidian | plain files | rich data |
| Notion | interaction | your files |
| Airtable | typed records | API lock-in |

#research
`,
};

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
