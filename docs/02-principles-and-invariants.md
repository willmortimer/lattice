# Principles and Invariants

These rules constrain product and implementation decisions. A feature that violates them requires an explicit architecture decision.

## 1. Native resources are canonical

Desktop and mobile workspaces are normal directories. Lattice may maintain memory buffers, journals, indexes, and caches, but canonical content remains externally visible and independently usable.

## 2. Offline is a baseline, not a degraded mode

Local opening, editing, search, links, SQLite data access, Parquet queries, notebook execution where runtimes exist, and approved scripts should work without connectivity. Sync is replication.

## 3. Compose formats; do not homogenize them

- Narrative text belongs in Markdown.
- Mutable relational applications belong in SQLite.
- Large analytical facts belong in Parquet or other appropriate analytical formats.
- Notebook computation belongs in Jupyter resources.
- Spatial layout belongs in a canvas manifest.
- Interactive application source belongs in ordinary web files.
- Ink belongs in an open typed stroke representation.

Embeds and relations connect resources without duplicating or swallowing them.

## 4. Every substantial resource has an inspectable representation

A resource should expose:

- Identity and path.
- Kind and format version.
- Source or canonical entrypoint.
- Dependencies.
- Permissions where executable.
- Human-readable fallback.
- Provenance where generated.
- Current validation state.

## 5. `.lattice/` is operational, not canonical

Search indexes, thumbnails, compiled scene caches, logs, browser mirrors, derived embeddings, and render outputs must be rebuildable. Pending sync operations and recovery journals are operationally important but must not become the only copy of canonical content.

## 6. Every GUI mutation is a semantic command

The UI does not own privileged functionality. Commands are reusable by CLI, API, MCP, plugins, scripts, workflows, tests, and future clients.

## 7. Direct file editing remains legitimate

The semantic API is the safe, validated path. External edits remain supported through file watching and reconciliation. Lattice never requires all writers to use its API.

## 8. Writes are transactional and recoverable

- Text and manifest files use buffered editing, a recovery journal, temporary files, and atomic replacement.
- SQLite uses database transactions and WAL where appropriate.
- Parquet updates use partitions, overlays, or explicit compaction rather than destructive whole-dataset rewrites.
- Canvas motion uses patch logs and periodic snapshots.
- Scripts and agents preferably return proposed transactions.

## 9. AI is an interchangeable actor

No canonical workflow requires a particular model provider. AI-created content, schemas, views, workflows, and applications use the same files and commands as human-created equivalents.

## 10. Human readability outranks token optimization

Agents receive efficient outlines, projections, query windows, and context bundles through APIs. Canonical content should not become compressed or fragmented merely to reduce prompt tokens.

## 11. Derived knowledge is distinguishable from authored knowledge

Embeddings, suggested relations, generated summaries, extracted entities, and agent memories retain provenance and can be deleted or rebuilt independently.

## 12. Views do not own their data

A board, chart, canvas interface, form, notebook display, or dashboard is a projection over underlying resources. Moving or deleting a view does not silently duplicate or destroy source data.

## 13. Structured data deserves real database semantics

Typed fields, constraints, indexes, foreign keys, transactions, relations, lookups, and rollups should use database capabilities rather than thousands of loosely coordinated Markdown properties.

## 14. Narrative work deserves real documents

Long-form thought should not be reduced to large database text cells. Data records may reference documents, and documents may embed views.

## 15. Large data never becomes a JavaScript object pile

Queries are bounded, cancellable, and streamed through columnar representations. Frontends receive viewport-sized Arrow batches or summaries.

## 16. Specialized renderers own hot loops

React or another shell coordinates the application but does not mediate text editor transactions, pen strokes, canvas camera movement, table scrolling, chart animation, or large query streaming.

## 17. Capabilities are lazy and contextual

A feature may be powerful without being always visible or resident. Bundles, workers, kernels, WebViews, indexers, and background services load only when enabled and needed.

## 18. Untrusted code has no ambient authority

Artifacts, Lattice Apps, plugins, scripts, workflows, and MCP clients receive explicit capabilities. Generated web applications never gain raw Tauri access.

## 19. Automation is observable

Every task and workflow exposes trigger, inputs, permissions, logs, outputs, prior runs, next run, and changed resources. Failures are visible.

## 20. The default is local and private

No content telemetry, remote indexing, model inference, or cloud upload occurs silently. OpenTelemetry instrumentation exists, but external export is opt-in or self-host configured.

## 21. Lower layers survive missing upper layers

- Missing plugin: show the file and fallback.
- Missing app runtime: show source and README.
- Missing chart renderer: show the Vega-Lite specification or static export.
- Missing canvas profile support: preserve JSON Canvas placement.
- Missing notebook kernel: preserve `.ipynb` content.

## 22. Open standards are preferred, not worshipped

Lattice should adopt standards when they provide real interoperability: CommonMark/GFM, JSON Canvas, SQLite, Parquet, Arrow, Jupyter, Vega-Lite, OpenAPI, BPMN, GeoParquet, and others. A documented Lattice format is justified where no existing format expresses required semantics cleanly.

## 23. Compiled representations are caches

Binary scene graphs, spatial indexes, highlighted HTML, notebook output caches, or optimized query plans may improve performance. They are never the only canonical representation.

## 24. Unlimited composition is not unlimited shell complexity

Domain-specific systems remain capability packs, plugins, data apps, or Lattice Apps. Core navigation remains stable.

## 25. Performance budgets are product requirements

Warm launch, quick note, typing latency, search latency, canvas frame rate, memory reclamation, and table query behavior are continuously measured against benchmark workspaces.

## 26. Complexity is revealed through progressive promotion

Users begin with familiar resources rather than implementation categories. A Table may acquire relations, views, forms, interfaces, analytical storage, or sheet semantics as its requirements emerge. Promotion preserves identity, links, history, and compatible layout.

## 27. External changes are revisions, not invented commands

Every Lattice-originated mutation is a semantic command. External mutations become first-class external revisions with the strongest safe semantic reconciliation available. Opaque replacement is preferable to fabricated precision.

## 28. Compound changes use workspace branches

Small proposals may be reviewed inline. Large imports, reorganizations, generated applications, and schema changes use copy-on-write workspace branches that can be browsed, validated, partially merged, or discarded.

## 29. Bundled formats require hero experiences

A format is Bundled only when Lattice provides excellent default interaction, indexing, lifecycle behavior, Inspect support, fallback behavior, conformance fixtures, and performance budgets. Broad file recognition alone does not justify permanent product surface.

## 30. Advanced behavior is exposed through Inspect

Lattice has one product experience. Source, manifests, history, lineage, permissions, raw data, queries, logs, sync, and conflicts are revealed contextually through Inspect rather than a global Workbench mode.
