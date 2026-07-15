# Design Review Addendum: Reconciliation, Promotion, Branching, and Product Discipline

**Status:** Accepted  
**Scope:** This addendum resolves the first comprehensive external design review of the full Lattice architecture package. Its decisions override any earlier ambiguity in the thematic documents and should be incorporated into those documents during future editorial consolidation.

## 1. Strategic correction

Lattice does not obtain Notion, Airtable, OneNote, Tableau, or Power BI merely by selecting open formats.

Markdown, SQLite, Parquet, Arrow, Vega-Lite, Jupyter, and JSON Canvas are commodity primitives. They provide interoperability, local ownership, broad tooling, dense model training exposure, and a durable substrate. They do not provide excellent product experiences by themselves.

The product thesis is therefore:

> Lattice hand-builds a small number of exceptional general-purpose surfaces over open primitives. The runtime, command system, composition model, and hero user experiences are the product. Agents, plugins, capability packs, and Lattice Apps supply the long tail of domain-specific surfaces that centralized platforms must either build themselves or refuse to support.

The initial hero workflows remain:

1. **Notes plus spatial canvas:** a credible Obsidian and OneNote replacement with polished Markdown editing, mixed resources, strong capture, and eventually first-class ink.
2. **Typed tables and data applications:** a credible Airtable and Notion-database replacement with excellent grid interaction, linked records, views, forms, interfaces, and actions.
3. **Analytical exploration and dashboards:** a local, open, composable Tableau/Power BI-like experience built around DuckDB, Parquet, Arrow, semantic models, notebooks, and declarative visualizations.

Jupyter, publishing, remote data, agent-generated applications, and domain packs reinforce these workflows rather than becoming unrelated product identities.

## 2. Progressive promotion is a core product principle

Lattice must not force users to select among implementation categories before their work requires those distinctions.

The user begins with a familiar object. As requirements emerge, Lattice offers a loss-preserving promotion into a richer resource. Promotion preserves identity, links, history, layout, and as much compatible presentation as possible.

The initial creation vocabulary should remain small:

- Page
- Canvas
- Table
- Notebook
- File

Drawing may appear contextually on pen-capable platforms, but should not expand the general creation menu unnecessarily.

### 2.1 Promotion ladders

```text
Canvas text       -> Page
Simple note       -> Structured page
Table             -> Data application
Large table       -> Analytical dataset plus mutable annotation layer
Grid              -> Sheet when coordinate formulas become necessary
Interface         -> HTML artifact when custom code becomes necessary
HTML artifact     -> Lattice App when routes, dependencies, build steps, or standalone publishing become necessary
Code block        -> Task, notebook, or app
Static chart      -> Bound live chart
Local report      -> Published site or standalone application
```

### 2.2 `/table` creates a real typed table

The `/table` command creates a SQLite-backed typed table immediately. It does not create an ephemeral Markdown table that later requires a disruptive migration.

Users initially see a polished grid called a Table. As they add relations, alternate views, forms, actions, and interfaces, it becomes a full data application without requiring a conceptual rename.

Markdown tables remain supported as ordinary document syntax. They are appropriate for small static tabular content, but are not the default interactive Table object.

### 2.3 One custom-interface entry point

Users should not initially choose among interface, dashboard, artifact, and app.

The primary action is **Create interface** or **Build view**. Lattice begins with declarative components on a canvas or dashboard. It only exposes promotion when a requirement crosses a capability boundary:

- Arbitrary JavaScript or custom rendering promotes to an HTML artifact.
- Routes, source dependencies, build tooling, or standalone deployment promote to a Lattice App.

The product should explain the reason at the point of promotion rather than requiring users to understand the architecture in advance.

## 3. Semantic commands and external writers

The earlier statement that every mutation is a semantic command is narrowed:

> Every mutation performed through Lattice is a semantic command. Every external mutation becomes a first-class external revision, with the most semantic reconciliation its format adapter can safely produce.

External tools remain legitimate first-class writers. Lattice must not pretend that an external byte replacement or uninstrumented database mutation originated as a semantic command.

### 3.1 Three revision layers

Lattice distinguishes:

- **Working revision:** unmaterialized editor state and recovery-journal entries.
- **Materialization revision:** the exact canonical file or database state currently persisted.
- **Semantic revision:** logical command history known to Lattice.

These revisions often advance together but are not interchangeable.

### 3.2 Three undo operations

Lattice must present three different behaviors honestly:

1. **Editor undo** reverses uncommitted changes in the active editor or surface.
2. **Command undo** applies the inverse of a committed semantic command only while its preconditions remain valid.
3. **Revert revision** creates a new proposed change that restores or merges content from an earlier revision.

After an external edit invalidates command preconditions, the old command is no longer blindly undoable. Reversion becomes a three-way operation using the prior base, current state, and desired restored state.

### 3.3 External-edit reconciliation pipeline

Each format adapter implements the strongest safe version of:

```text
detect external change
    -> wait for a stable materialization
    -> compare with the last known materialization
    -> create a structured external revision where possible
    -> fall back to opaque replacement where necessary
    -> invalidate or rebase pending proposals
    -> update catalog, dependencies, indexes, and open surfaces
```

Textual and structured formats may produce section, block, field, node, or manifest-level revisions. Binary assets may produce only a replacement revision with old and new fingerprints. Both are legitimate.

### 3.4 SQLite operating profiles

Lattice adopts two SQLite profiles.

#### Portable SQLite profile

- The database remains ordinary, uninstrumented SQLite.
- External tools may change it freely.
- Lattice detects schema and data changes after the fact.
- Stable primary keys and row hashes may permit synthetic row-level diffs for manageable datasets.
- Large, ambiguous, or unstable databases may reconcile as snapshot replacements.
- External changes do not receive guaranteed row-level undo or semantic collaborative merging.

#### Collaborative SQLite profile

A data application participating in semantic multiwriter synchronization uses documented, inspectable instrumentation:

- Stable primary keys.
- Schema version metadata.
- Reserved `_lattice_*` tables.
- Audit triggers recording external inserts, updates, and deletes.
- Explicit schema migration records.
- Change-log sequence numbers and compaction.

Because SQLite triggers execute for ordinary external connections, this profile can capture writes from `sqlite3`, Python, Drizzle, and other conventional clients. If instrumentation is removed, bypassed, corrupted, or contains a history gap, Lattice falls back to snapshot reconciliation.

The collaborative profile remains valid SQLite and must not require a proprietary database engine.

### 3.5 Schema-change discipline

Row edits may occur offline against an established schema. Destructive schema changes receive stricter treatment:

- Destructive or ambiguous schema migrations occur on a workspace branch.
- Collaborative schema changes require a lease, serialized migration workflow, or explicit merge.
- Concurrent incompatible migrations are never silently ordered.
- External DDL creates a schema-reconciliation revision.
- Schema conflicts use the same user-facing conflict envelope as other resource conflicts.

## 4. Workspace branches are a core safety mechanism

Small proposed changes and compound workspace changes require different review experiences.

### 4.1 Inline proposals

Inline review is suitable for bounded edits such as:

- Updating a few paragraphs.
- Adding a limited number of records.
- Creating one view.
- Moving several canvas nodes.

### 4.2 Workspace branches

Large changes use a copy-on-write workspace branch implemented through the overlay storage architecture.

Examples:

- Build or redesign a complete data application.
- Reorganize a notebook or documentation site.
- Normalize a large database.
- Import a workspace from another product.
- Apply destructive schema migrations.
- Generate a multi-file application, dashboard, workflow, or capability pack.
- Resolve a compound conflict.

A branch is browsable and executable as a coherent alternate workspace. The user can:

- Navigate generated pages.
- Interact with proposed views and applications.
- Query proposed databases.
- Preview canvases visually.
- Run tests and validations.
- Inspect capabilities and dependencies.
- Compare semantic changes by resource kind.
- Merge all, merge selected changes, retain the branch, or discard it.

Git remains useful for source-oriented history, but Lattice branches provide semantic review across Markdown, SQLite, canvas manifests, Parquet datasets, notebooks, workflows, artifacts, and application packages.

### 4.3 Specialized review surfaces

Review must be format-aware:

- Documents: outline changes, section moves, inline text, links, and generated-region ownership.
- Databases: schema migrations, row counts, samples, aggregate effects, relationships, and constraint checks.
- Canvases: before/after overview, node additions, removals, movement, reading order, and references.
- Apps and artifacts: live sandboxed preview, source tree, dependencies, capabilities, build output, tests, and security findings.
- Workflows: triggers, side effects, schedules, maximum scope, and permissions.
- Publications: public routes, included snapshots, excluded private resources, and destination.

A generic file-count summary is not sufficient for compound agent output.

## 5. Auto-approval policy is part of the security model

Auto-approval is not a convenience setting. It is an executable policy language and a practical authority boundary.

A policy may constrain:

- Actor or client identity.
- Command allowlist.
- Workspace, path, notebook, table, and resource-kind scopes.
- Maximum files, bytes, records, rows, operations, and runtime.
- Creation, update, move, deletion, and permanent-deletion permissions.
- Schema modification.
- Executable-content creation.
- Dependency addition or upgrade.
- Network destinations.
- Remote writes.
- Publishing.
- Required tests, validation, and policy checks.
- Rate, recurrence, and expiration.
- Generated versus human-authored regions.
- Whether a workspace branch is mandatory.

The following remain nondelegable by default:

- Granting new capabilities.
- Reading a new secret.
- Publishing previously private content.
- Writing to a new remote system.
- Destructive schema migration.
- Permanent deletion.
- Installing unsigned native code.
- Weakening an approval policy.

Lattice must not offer a broad, unscoped “always allow this agent” control.

## 6. Identity, paths, renames, and repair

Lattice adopts progressive identity.

### 6.1 Self-identifying resources

Markdown front matter, canvas manifests, view manifests, workflow manifests, data-app manifests, notebook metadata, artifact manifests, and App manifests may carry stable IDs directly.

### 6.2 Plain files

An unpromoted image, PDF, archive, or other ordinary file may initially be path-addressed and content-fingerprinted.

When stable identity becomes necessary for annotations, durable relations, collaboration, metadata, or long-lived links, Lattice creates a portable sidecar:

```text
report.pdf
report.pdf.lattice.yaml
```

Sidecars may contain stable ID, fingerprint, aliases, annotation resources, provenance, and link metadata. Extended filesystem attributes must not be the only source of canonical identity.

### 6.3 Rename through Lattice

A rename initiated through Lattice is a semantic refactor.

- Lattice computes affected path references.
- The user reviews the operation as one transaction.
- By default, parseable canonical resources are rewritten so the workspace remains portable outside Lattice.
- Large rewrites may be deferred, leaving an explicit alias map and a visible “portable path stale” state.

A large Git diff is sometimes the honest cost of keeping path references correct and external-tool-compatible.

### 6.4 External rename

When a move occurs outside Lattice:

- IDs, sidecars, hashes, and filesystem events are used to rediscover the resource.
- The derived index updates immediately.
- Lattice does not silently rewrite a large set of source files.
- It creates a repair proposal listing stale materialized paths.

This avoids surprising source mutation while preserving a path to portable repair.

## 7. One conflict-revision mental model

Different resource formats retain different merge mechanics, but the user sees one concept:

> A conflict is a resource revision with multiple incompatible descendants.

Every conflict includes:

- Conflict ID.
- Resource identity.
- Base revision.
- Local descendant.
- Incoming, remote, or external descendant.
- Affected semantic units where known.
- Reason automatic merge failed.
- Resolution actions.

Universal actions are:

- Keep local.
- Keep incoming.
- Merge.
- Keep both.
- Open as branch.
- Defer.

Format adapters provide specialized presentations for documents, canvases, SQLite rows and schemas, Parquet partitions and manifests, source trees, and opaque files.

Lattice explicitly refuses a near-term promise of real-time collaboration across every format. Text and canvas collaboration come first; row collaboration follows after local semantic behavior is proven; schema collaboration remains serialized or branch-based; Parquet is partition/manifest-oriented; opaque files remain snapshot-oriented.

## 8. Product and UX corrections

### 8.1 Remove global Workbench mode

Lattice will not split into normal and workbench personalities.

Every substantial resource instead exposes **Inspect**, which can reveal:

- Source and manifest.
- Dependencies and lineage.
- History and branches.
- Permissions and capabilities.
- Queries and schemas.
- Raw files and data.
- Logs and diagnostics.
- Sync and conflict state.

Developer diagnostics may be an optional preference, but not a separate product mode.

### 8.2 Move capture earlier

Quick capture is part of the first hero workflow and cannot wait for the complete mobile application.

Early capture surfaces include:

- Desktop global shortcut.
- Menu-bar or tray quick note.
- CLI and stdin capture.
- Browser extension.
- macOS share service.
- Minimal iOS/iPadOS share extension.
- Email-to-inbox connector.
- Clipboard and file-drop inbox.

A lightweight share component may create portable inbox packages without loading the complete workspace runtime.

### 8.3 Define deletion and dangling references

Deletion is soft by default:

```text
Active -> Trash -> Tombstone -> Permanently removed
```

Before trashing, Lattice shows incoming references, dependent views, derived outputs, workflows, publications, and embedded uses.

Deleting a view never deletes its source data. Deleting a data source leaves dependents in a visible unresolved state with restore, relink, and remove-reference actions.

Tombstones retain enough identity and history to explain and repair links. Data without views remains discoverable through the resource browser, catalog, search, Inspect, and an unused-resource report.

## 9. Workspace catalog becomes foundational

The `workspace.*` catalog is promoted to foundational infrastructure and a public introspection contract.

It powers:

- Search and backlinks.
- Dependency and lineage analysis.
- Rename and deletion impact.
- Orphan and dangling-reference detection.
- Conflict inbox and branch comparison.
- Conformance and benchmark fixtures.
- AI context generation.
- Workspace self-analysis.

Expected relations include:

```text
workspace.resources
workspace.links
workspace.dependencies
workspace.revisions
workspace.commands
workspace.conflicts
workspace.branches
workspace.generated_resources
workspace.capabilities
workspace.tasks
workspace.publications
workspace.validation_issues
```

The catalog remains derived and rebuildable, but its schema is a stable API surface.

## 10. Generated resources form a knowledge-work build system

Lineage is not only metadata. Lattice treats generated and derived resources as build targets with explicit inputs, builder, output, input revisions, last successful build, staleness, and ownership.

Ownership modes are:

- **Generated:** may be rebuilt from declared inputs; direct edits fork or change ownership.
- **Human-owned:** automation may propose edits but never overwrite it as a build output.
- **Hybrid:** explicit generated regions coexist with human-authored regions.

States include:

- Current.
- Stale.
- Building.
- Failed.
- Human-modified.
- Forked.
- Source missing.

A builder must not overwrite a manually modified generated region without a merge or explicit approval.

This system applies to reports, dashboards, views, schemas, applications, documentation sites, analytical extracts, and AI-generated artifacts.

## 11. Bundled-format admission rule

Lattice must not become a file manager with mediocre viewers.

A format is Bundled only when Lattice provides:

- Excellent default opening experience.
- Excellent default editing, querying, or exploration experience.
- Lifecycle and memory discipline.
- Import/export behavior.
- Search and indexing behavior.
- Inspect surface.
- Security model.
- Fallback behavior.
- Conformance fixtures.
- Performance budgets.

Otherwise the format remains available through a capability pack or plugin.

## 12. Publishing, templates, and importers move earlier

### 12.1 Publishing

Static publishing is both a utility and Lattice's primary distribution loop. Early supported outputs should include:

- Markdown documentation sites.
- Read-only reports.
- Dashboards with bounded data snapshots.
- Canvas presentations.
- Standalone artifacts and Apps.
- Cloneable workspace templates.

Published resources must preserve provenance, dependency state, privacy boundaries, and reproducibility where practical.

### 12.2 Compound templates

Because workspaces are directories, complete templates can be ordinary cloneable repositories or packages. A gallery can demonstrate research vaults, founder operations, course workspaces, data laboratories, and other domains without placing each domain in the permanent shell.

### 12.3 Importers

Migration quality is go-to-market infrastructure rather than late cleanup. Priority importers include:

- Obsidian.
- Notion.
- OneNote.
- Airtable.
- Evernote.
- CSV and Excel.
- Jupyter.
- Generic Markdown directories.

Imports should execute on workspace branches so users can inspect fidelity, unresolved mappings, and generated resources before merge.

## 13. Domain packs accepted for the long-term registry

These capabilities fit Lattice but remain packs rather than core shell products.

### Finance

- Beancount or Ledger canonical text.
- OFX, QFX, and CSV imports.
- Reconciliation views.
- DuckDB analysis.
- Vega-Lite dashboards.
- Receipts and source-document links.

### Research and archives

- Maildir and mbox corpora.
- WARC and SingleFile archives.
- Read-only search, linking, citation, and attachment extraction.
- No full email-client commitment.

### Personal interchange

- vCard contacts.
- ICS calendars and subscriptions.
- User-selected Apple Health and comparable exports normalized into Parquet.

### Engineering references

Commit-anchored references bind narrative resources to repository, commit SHA, path, and symbol or line range. Lattice tracks whether the reference remains current, moved, changed, or unavailable.

## 14. Explicit non-goals retained

Lattice continues to refuse:

- Real-time collaboration across every format as an early promise.
- A mandatory bundled AI chat product.
- A hidden proprietary AI memory graph.
- Domain mini-products permanently embedded in the shell.
- Opaque canonical formats chosen solely for feature parity.
- A proprietary DAX-like analytical language.
- Automatic destructive external-edit interpretation where reliable semantics are unavailable.

## 15. Implementation and roadmap consequences

The following move earlier or become core architectural work:

1. External-edit reconciliation framework and format-adapter contracts.
2. Workspace catalog and stable introspection schema.
3. Progressive promotion UX and conversion invariants.
4. SQLite portable and collaborative profiles.
5. Workspace branches and format-aware review.
6. Auto-approval policy grammar and evaluator.
7. Resource identity, sidecars, rename repair, deletion, and tombstones.
8. Generated-resource ownership and knowledge-work build semantics.
9. Quick capture shims.
10. High-fidelity import branches and early static publishing.

The following remain deliberately staged:

- Real-time text and canvas collaboration before database collaboration.
- Database row collaboration before schema collaboration.
- Full mobile workspace after lightweight capture and sharing.
- Specialized domain packs after hero surfaces are excellent.

## 16. Decisions resolved by this addendum

The following are no longer open questions:

- Lattice uses two SQLite profiles: portable and collaborative.
- `/table` creates a real SQLite-backed typed table.
- Progressive promotion is a core product principle.
- External writes become external revisions rather than fictional semantic commands.
- Workspace branching is core infrastructure and a user-facing review mechanism.
- Auto-approval is a policy language, not a Boolean preference.
- IDs and paths use progressive identity with portable sidecars where needed.
- Rename-through-Lattice and external-rename repair have distinct policies.
- All formats share one user-facing conflict-revision model.
- Workbench mode is removed in favor of per-resource Inspect.
- Quick capture, publishing, and importers move earlier.
- Bundled formats require a hero surface.
- Generated resources have explicit ownership and overwrite rules.

