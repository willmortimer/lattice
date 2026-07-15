# Search, Links, Context, and AI Interoperability

## Organization model

Lattice combines several complementary systems:

```text
Directories   primary location and project boundary
Links         explicit relationships
Tags          cross-cutting classification
Properties    structured metadata
Relations     typed resource or record connections
Views         dynamic presentations
Canvases      authored arrangements
Search        retrieval across all resources
```

Folders remain useful and are not replaced by graph ideology.

## Search stack

Start with:

- Exact filename and title matching.
- SQLite FTS5 or equivalent full-text index.
- Heading and block index.
- Tags and properties.
- Backlinks and relation graph.
- Code symbols.
- Dataset schemas and column names.
- Notebook cells and outputs.
- PDF text and citations.
- Ink recognition text.
- Artifact/app manifest and README.
- Optional embeddings and semantic ranking.

Search indexes are derived and rebuildable.

## Stable links

Support:

- Relative Markdown links.
- Wiki links.
- Heading anchors.
- Block IDs.
- Stable Lattice URIs.
- Dataset record and query links.
- Notebook cell references.
- PDF and source anchors.
- Canvas node references.

Moves and renames update index mappings and can repair human-readable paths.

## Transclusion

Pages and blocks may be embedded elsewhere without duplication. Source identity remains visible.

## Optional typing

Inspired by Tana-style typed notes, a page or block can declare a type:

```yaml
---
type: decision
status: accepted
project: "[[Lattice]]"
---
```

Typed pages can receive templates, property editors, saved views, and commands. Users are never required to make the whole workspace an object graph.

When structure becomes genuinely tabular or relational, Lattice should offer promotion into a SQLite data app.

## Capture and inbox

Inspired by capture-first tools:

- Global inbox.
- Clipboard and share-sheet capture.
- Browser clipper.
- Email/forwarding connector.
- Voice and transcript import.
- CLI/API capture.
- Source URL and retrieval metadata.
- Suggested destination and links.

Suggestions are non-authoritative.

## Context bundles

A context compiler builds human-readable bounded context:

```bash
lattice context build \
  --pages architecture,roadmap \
  --dataset inventory \
  --max-tokens 24000 \
  --output context.md
```

Representation strategies:

- Full Markdown for short pages.
- Outline and selected sections for long pages.
- Schema, statistics, and samples for datasets.
- SQL aggregates instead of raw tables.
- Mermaid source rather than image.
- Canvas reading order rather than coordinates.
- Artifact README and manifest rather than bundled JavaScript.
- Recent diffs rather than complete unchanged resources.

Canonical source remains human-readable; context optimization happens at retrieval time.

## AI interaction

External agents can:

- Read raw files.
- Query through CLI/API/MCP.
- Create resources.
- Propose transactions.
- Generate schemas, views, dashboards, notebooks, apps, and workflows.
- Organize folders and links.
- Build context bundles.
- Run approved scripts.

Lattice does not require a chat sidebar. A chat client may be a plugin using the same APIs.

## Provenance

Generated content records:

- Sources.
- Generator/actor.
- Time.
- Model or tool when relevant.
- Human-reviewed state.
- Input revision.
- Staleness.

Generated summaries never silently become canonical truth.

## Optional derived intelligence

Permitted rebuildable features:

- Embeddings.
- Suggested related resources.
- Duplicate detection.
- Entity extraction.
- Contradiction warnings.
- Broken citation detection.
- Staleness detection.
- Suggested organization.

They remain derived indexes or proposed changes, not an invisible authoritative brain.

## Workspace catalog

Expose the workspace itself as queryable derived tables:

```text
workspace.resources
workspace.pages
workspace.headings
workspace.blocks
workspace.links
workspace.tags
workspace.properties
workspace.canvases
workspace.datasets
workspace.tables
workspace.columns
workspace.artifacts
workspace.revisions
```

This allows SQL and notebooks to analyze workspace structure without making the catalog canonical.
