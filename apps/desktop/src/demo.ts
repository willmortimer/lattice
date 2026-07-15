import type { WorkspaceSnapshot } from "./types";

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
