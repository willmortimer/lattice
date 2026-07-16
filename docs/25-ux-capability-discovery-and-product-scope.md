# UX, Capability Discovery, and Product Scope

## Product tension

Lattice can support a vast range of work without becoming Notion again only if breadth is structurally isolated and contextually revealed.

Principle:

> Everything supported; almost nothing shown by default.

## Initial experience

Workspace creation begins with a small, purpose-based gallery:

```text
Personal     capture, projects, areas, library, journal
Project      one finite outcome with mixed working resources
Research     questions, sources, notes, experiments, outputs
Data Lab     sources, data, queries, notebooks, dashboards, reports
Blank        manifest only
```

Personal is recommended. Each choice shows a one-sentence outcome and a small
structural preview rather than a feature inventory. Sample workspaces are
offered separately from organizational templates.

The second creation step asks only for title, location, and whether the new
workspace should become the user-level default. Templates initialize the
workspace once and never retain ownership of user content.

After creation, a workspace prominently offers:

```text
New page
New canvas
Open file
Quick capture
```

Advanced data, compute, publishing, and automation capabilities emerge through context, search, templates, or explicit capability enablement.

## Contextual slash commands

In a paragraph:

```text
Heading
List
Callout
Code
Image
Embed
```

On a canvas:

```text
Note
Page
File
Drawing
Data view
Artifact
```

In a data app:

```text
Table
Form
Board
Chart
Query
Interface
```

Do not show hundreds of global commands merely because they exist.

## Capability layers

### Core

Always available and small:

- Resources.
- Pages.
- Canvas skeleton.
- Embedding.
- Search.
- Command/transaction core.
- Security.
- CLI/API/MCP.
- Plugin host.

### Bundled lazy capabilities

- SQLite.
- DuckDB/Parquet/Arrow.
- Jupyter.
- PDF.
- Ink.
- Charts.
- Remote databases.
- Pandoc.
- Workflows.
- Documentation publishing.

### Capability packs and plugins

- Research.
- CRM.
- Software projects.
- Geospatial.
- Scientific computing.
- Observability.
- Publishing.
- Jira/Linear/GitHub integrations.

## Workspace capability manifest

```yaml
capabilities:
  enabled:
    - pages
    - canvas
    - sqlite
    - citations
    - jupyter
  disabled:
    - remote-databases
    - geospatial
```

This controls:

- Slash commands.
- Bundles loaded.
- Indexers.
- Background services.
- Settings and inspector panels.
- Plugin permissions.

## Immediate and workbench modes

### Immediate mode

- Click and type.
- Paste image.
- Draw.
- Add table.
- Filter.
- Ask external AI to organize or generate.
- Drag resources onto canvas.

### Workbench mode

Inspect:

- Raw Markdown.
- SQL schema and migrations.
- View YAML.
- Canvas profile.
- App source.
- Workflow definition.
- Permissions.
- Query plan.
- Revision history.
- Telemetry trace.

The product is friendly without becoming opaque.

## Inspectability

Every resource has **Inspect**:

- Source and manifest.
- Dependencies.
- Permissions.
- Current renderer.
- History.
- Lineage and staleness.
- Raw files.
- Logs or console where executable.

## Navigation

Stable shell areas:

- Workspace tree.
- Search/command.
- Open tabs/panes.
- Canvas/page area.
- Context inspector.
- Problems/jobs when relevant.

Avoid branded mini-products such as separate CRM, Mail, Calendar, AI, or Projects apps inside the shell. They are views, plugins, or packs.

## OneNote-inspired behavior

- Fast notebook/section/page navigation.
- Click-anywhere canvas text.
- Mixed typed and handwritten material.
- Page templates and paper styles.
- Quick capture.
- Mobile reading and iPad drawing.

## Notion-inspired behavior

- Polished slash menu.
- Drag/reorder/transform blocks.
- Rich embeds.
- Database views.
- Simple creation and inline editing.
- Strong keyboard operation.

## Airtable-inspired behavior

- Spreadsheet-like entry with typed schema.
- Progressive linked records.
- Views and interfaces.
- Forms and buttons.
- Automation discoverability.

## Performance UX

- Loading states explain what is loading.
- Stale previews remain usable.
- Long queries show progress and cancellation.
- Heavy capability activation is explicit.
- Memory-heavy kernels and apps show lifecycle controls.
- Safe mode is understandable.

## Product boundary

Core implements:

```text
resources
composition
commands
transactions
queries
execution
events
capabilities
observability
publishing
sync
```

Domains implement:

```text
CRM
issue tracking
research
observability dashboard
scientific viewer
customer portal
project methodology
```

This is how Lattice supports almost everything without making almost everything permanent UI.
