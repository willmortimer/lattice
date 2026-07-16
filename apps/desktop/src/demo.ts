import type { SearchHit, WorkspaceSnapshot } from "./types";

/**
 * Dev-only stand-in used when the frontend runs in a plain browser
 * (`pnpm dev` + localhost) where the Tauri IPC bridge doesn't exist.
 * Lets the shell be designed and reviewed without the native window.
 * Never bundled into release builds.
 */
export const inBrowser = import.meta.env.DEV && !("__TAURI_INTERNALS__" in window);

/** `?empty` reviews the empty state instead of the demo workspace. */
export const demoStartEmpty =
  inBrowser && new URLSearchParams(window.location.search).has("empty");

export const demoSnapshot: WorkspaceSnapshot = {
  root: "/Users/you/Engineering Workspace",
  title: "Engineering Workspace",
  id: "0198-demo",
  resources: [
    { path: "README.md", kind: "page" },
    { path: "Product/Vision.md", kind: "page" },
    { path: "Product/Roadmap.md", kind: "page" },
    { path: "Product/Product Strategy.canvas", kind: "canvas" },
    { path: "Research/Competitor Analysis.md", kind: "page" },
    { path: "Research/Competitors.data", kind: "data-app" },
    { path: "Analytics/Usage.dataset", kind: "dataset" },
    { path: "Notebooks/Usage Analysis.ipynb", kind: "notebook" },
    { path: "Drawings/Architecture Notes.ink", kind: "ink" },
    { path: "Artifacts/Market Map.artifact", kind: "artifact" },
    { path: "Apps/Customer Portal.app", kind: "app" },
    { path: "Automations/Refresh Research.workflow.yaml", kind: "workflow" },
    { path: "Scripts/Normalize Companies.task", kind: "task" },
    { path: "Assets/team-photo.png", kind: "file" },
  ],
};

/**
 * `Product/Product Strategy.canvas` fixture: text notes, file nodes back
 * onto the resources above, one group, and enough edges to review panning,
 * zooming, selection, and file-node double-click in a plain browser.
 */
export const demoCanvas = {
  nodes: [
    {
      id: "strategy-group",
      type: "group",
      label: "Strategy",
      x: 40,
      y: 40,
      width: 800,
      height: 360,
    },
    {
      id: "intro",
      type: "text",
      x: 60,
      y: 60,
      width: 260,
      height: 140,
      text: "Q3 push: grow from evaluation to daily use. Vision and roadmap anchor everything below — competitor data feeds the roadmap bets directly.",
    },
    {
      id: "vision",
      type: "file",
      file: "Product/Vision.md",
      x: 340,
      y: 60,
      width: 220,
      height: 140,
    },
    {
      id: "roadmap",
      type: "file",
      file: "Product/Roadmap.md",
      x: 580,
      y: 60,
      width: 220,
      height: 140,
    },
    {
      id: "competitors-page",
      type: "file",
      file: "Research/Competitor Analysis.md",
      x: 60,
      y: 240,
      width: 220,
      height: 130,
    },
    {
      id: "competitors-data",
      type: "file",
      file: "Research/Competitors.data",
      x: 320,
      y: 240,
      width: 220,
      height: 130,
    },
    {
      id: "usage-dataset",
      type: "file",
      file: "Analytics/Usage.dataset",
      x: 580,
      y: 240,
      width: 220,
      height: 130,
    },
    {
      id: "usage-notebook",
      type: "file",
      file: "Notebooks/Usage Analysis.ipynb",
      x: 340,
      y: 460,
      width: 240,
      height: 140,
    },
    {
      id: "note-market",
      type: "text",
      x: 620,
      y: 460,
      width: 220,
      height: 140,
      text: "Market map still needs the enterprise segment split out before the all-hands.",
    },
    {
      id: "market-map",
      type: "file",
      file: "Artifacts/Market Map.artifact",
      x: 880,
      y: 460,
      width: 220,
      height: 140,
    },
    {
      id: "spec-link",
      type: "link",
      url: "https://jsoncanvas.org",
      x: 60,
      y: 460,
      width: 220,
      height: 110,
    },
  ],
  edges: [
    { id: "e1", fromNode: "intro", toNode: "vision", fromSide: "right", toSide: "left" },
    { id: "e2", fromNode: "vision", toNode: "roadmap", fromSide: "right", toSide: "left" },
    {
      id: "e3",
      fromNode: "roadmap",
      toNode: "competitors-data",
      label: "informs",
    },
    { id: "e4", fromNode: "competitors-page", toNode: "competitors-data" },
    { id: "e5", fromNode: "competitors-data", toNode: "usage-dataset" },
    { id: "e6", fromNode: "usage-dataset", toNode: "usage-notebook" },
    { id: "e7", fromNode: "usage-notebook", toNode: "market-map", label: "feeds" },
    { id: "e8", fromNode: "note-market", toNode: "market-map" },
  ],
};

export const demoPage = `# Competitor Analysis

A running comparison of the tools Lattice draws from — and where each one
traps its users.

## The short version

| Tool | Keeps | Traps |
| --- | --- | --- |
| Obsidian | plain files, links | rich data, interfaces |
| Notion | interaction quality | your entire workspace |
| Airtable | typed records, views | records behind an API |
| Jupyter | open notebooks | everything around them |

> The unmet need is a fast local workspace that treats documents, data,
> computation, and composition as equally legitimate resources.

## Working notes

- Imported the pricing pages into \`Competitors.data\` — see the **By segment** view.
- The usage numbers in \`Usage.dataset\` go far beyond any hosted table limit.
- Next: wire the refresh workflow to pull fresh crawls every morning.

\`\`\`sql
SELECT segment, count(*) AS tools
FROM read_parquet('Analytics/Usage.dataset/facts/**/*.parquet')
GROUP BY 1 ORDER BY 2 DESC;
\`\`\`
`;

/**
 * The demo shell's stand-in for `search_workspace`: a plain path-substring
 * match over the fixture resources, since only `demoPage` has real body
 * text to search. Good enough to review the search pane's UI without a
 * real workspace.
 */
export function demoSearch(query: string): SearchHit[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return [];

  return demoSnapshot.resources
    .filter((resource) => resource.kind === "page" && resource.path.toLowerCase().includes(trimmed))
    .map((resource) => ({
      path: resource.path,
      title: resource.path.split("/").pop() ?? resource.path,
      snippet: null,
      rank: 0,
    }));
}
