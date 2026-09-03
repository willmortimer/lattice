# Public contracts (surface 3)

Authoritative **public** contracts for humans, agents, and the docs site:

- CLI, MCP, and HTTP API surfaces
- Format support matrix (shipped vs near-roadmap)
- Plugins / integrations matrix
- Open format layouts under `open/` for agent fetch, skills, MCP embed, and
  Google-indexed URLs

## Audience

Power users, integrators, and agents. Not private strategy. Not end-user Help
tone (see `docs/help/`).

## Voice

- Precise and technical enough to implement against.
- Mark near-roadmap rows **unchecked** — do not present them as shipped.
- No YC/GTM, no private Cell/KernelFS runbooks, no secrets, no internal sprint
  DAGs.
- Prefer stable vocabulary that matches the public product.

## Layout

```text
docs/contracts/
├── README.md                 # this file
├── navigation.json           # pages published into the site reference section
├── cli.md
├── mcp.md
├── api.md
├── formats.md                # support matrix
├── integrations.md           # plugins / connectors matrix
├── open/                     # agent-fetchable open format pack
│   ├── README.md
│   ├── llms.txt              # curated index (also mirrored to site/public)
│   ├── workspace/
│   ├── page/
│   ├── canvas/
│   ├── data/
│   ├── dataset/
│   ├── notebook/
│   ├── chart/
│   ├── artifact/
│   ├── task/
│   └── docs-project/
└── generated/                # script output; do not hand-edit
```

## Status markers

| Marker | Meaning |
| --- | --- |
| `[x]` **Shipped** | In a current public build or documented public API |
| `[ ]` **Near** | Near-roadmap; listed so agents and users can plan |
| Omitted / Later | Deferred; do not pad the matrix |

Deep engineering inventory remains in
`lattice/docs/37-capability-and-format-registry.md`. This tree is the *public*
subset with honest shipped/near labels.

## Publish path

`scripts/sync-docs-surfaces.mjs`:

1. Validates navigation + required open folders.
2. Copies selected contract pages into `site/docs/` (reference section).
3. Mirrors `open/` into `site/public/open/` for stable URLs.
4. Writes `site/public/llms.txt` and `site/public/llms-full.txt`.
5. Stages the in-app Help bundle under `generated/help-bundle/`.
6. When `lattice/docs/` exists (public client checkout), mirrors Help markdown +
   `navigation.json` to `lattice/docs/help/`, contract pages to
   `lattice/docs/contracts/`, and `open/` to `lattice/docs/open/` (overwrite
   colliding files only; pass `--skip-lattice` to skip).

Starlight still builds from `site/docs/` via `site/scripts/sync-docs.mjs`.

Public-client MCP embed reads `lattice/docs/contracts/` + `lattice/docs/open/`
(`crates/lattice-docs-pack`). Run this sync after editing umbrella contracts so
embedded `include_str!` content matches the published surfaces.
