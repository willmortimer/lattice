# Terminology and Conventions

## Core terms

### Workspace

A portable root directory containing canonical resources and optional `.lattice/` operational state.

### Workspace template

A one-time scaffold used to initialize a new workspace. After creation, its
files and folders belong entirely to the user; the template does not retain
ownership or update instantiated content.

Built-in templates are versioned declarative packages under
`templates/workspaces/<id>/`. First Look is a separate sample action and Team
is a hidden legacy identifier, not a gallery choice.

### Starter project

A coherent project scaffold added to an existing workspace. Unlike a workspace
template, it does not define the whole workspace and must be previewed for
collisions before merge.

### Sample workspace

A curated demonstration used for evaluation, onboarding, screenshots, and
conformance testing. It is not presented as a normal organizational template.

### Resource

Any addressable item Lattice can inspect or render: page, dataset, view, canvas, notebook, ink drawing, artifact, app, workflow, task, file, query, form, or external connector target.

### Page

A Markdown-backed narrative document. A page can contain text, code, embeds, links, diagrams, and references to other resources.

### Canvas

A spatial or structured composition manifest. It positions and connects independent resources. Canvas placement is not canonical content ownership.

### Frame

A canvas container with its own internal layout: flow, stack, grid, dashboard, absolute, record detail, or nested canvas.

### Data application

A mutable relational application package, normally centered on SQLite, with schema, migrations, UI metadata, views, forms, interfaces, actions, and optional adapters.

### Analytical dataset

A large, often append-oriented or immutable collection such as partitioned Parquet, optionally paired with mutable SQLite annotations and a semantic model.

### View

A saved query and presentation over a source. Examples include grid, board, calendar, chart, map, dashboard, pivot, and record detail.

### Interface

A role- or task-specific canvas or dashboard over data and documents, inspired by Airtable Interface Designer.

### Notebook

A Jupyter-compatible `.ipynb` resource whose kernel and environment are selected independently from the file.

### Ink resource

A package containing open typed stroke data, a portable preview, layers, coordinate metadata, and optional platform-native caches.

### Artifact

A sandboxed HTML/CSS/JavaScript mini-application, usually focused on one visualization, simulation, calculator, or interactive report.

### Lattice App

A complete source-backed web project that can contain routes, dashboards, landing pages, internal tools, or customer-facing interfaces. It builds to browser assets and integrates through the Lattice App SDK.

### Task

A manually or programmatically invoked unit of execution implemented by Python, Node, native command, Nix environment, container, WASI component, notebook, or remote runner.

### Workflow

A durable graph or ordered sequence of triggers, conditions, and actions. Workflow definitions are open YAML or supported BPMN/DMN resources.

### Capability pack

A distributable bundle of schemas, views, canvases, templates, workflows, tasks, plugins, documentation, and commands implementing a domain use case.

### Plugin

An extension contributing explicit resource handlers, renderers, commands, indexers, connectors, automation actions, or inspectors under a capability grant.

### Connector

An adapter for an external system such as a database, object store, REST API, GraphQL endpoint, Jupyter server, Git repository, observability backend, calendar, or filesystem.

### Command

A semantic operation accepted by the command core. Commands are validated and may produce a transaction.

### Transaction

An atomic, auditable group of operations with preconditions, permissions, diffs, results, and history.

### Proposed transaction

A transaction returned by a script, app, plugin, or agent for inspection and approval before canonical mutation.

### Resource revision

A stable representation of a resource state, usually including content hash, logical revision, writer, and timestamp.

### Derived resource

An output generated from declared inputs by a task or workflow. It has lineage and a staleness state.

### Context bundle

A human-readable and bounded representation of selected workspace resources for an external agent or export.

## Path and identity conventions

Resources use both:

- Human-readable relative paths.
- Stable sortable identifiers, preferably UUIDv7 or an equivalent documented scheme.

Paths are the primary human interface. IDs preserve identity through moves and renames.

Example URI:

```text
lattice://workspace-id/resource/resource-id
```

Human-facing links may remain relative:

```markdown
[Architecture](../Architecture.md)
```

## Naming conventions

Recommended package suffixes:

```text
Name.data/         mutable relational application
Name.dataset/      analytical dataset
Name.artifact/     HTML artifact
Name.app/          full Lattice App
Name.ink/          open ink package
Name.task/         executable task package
```

Recommended manifest suffixes:

```text
*.view.yaml
*.form.yaml
*.workflow.yaml
*.task.yaml
*.artifact.yaml
*.semantic-model.yaml
*.lattice.yaml
```

Existing open formats keep their established extensions:

```text
*.md
*.canvas
*.ipynb
*.mmd
*.dot
*.vl.json
*.bpmn
*.dmn
*.ics
*.parquet
*.arrow
*.feather
*.sqlite
*.duckdb
```

## Manifest conventions

Every Lattice manifest should include:

```yaml
format: lattice-view
version: 1
id: 019b...
title: Active customers
```

Rules:

- `format` is a stable format identifier.
- `version` is an integer or semantically versioned documented value.
- Relative paths resolve from the manifest directory.
- Unknown fields are preserved where practical.
- Extensions use namespaces.
- Schemas are published.
- Canonical serialization is deterministic where useful for Git and hashing.

## Canonical versus derived

Manifests should explicitly distinguish:

```yaml
role: canonical
```

or:

```yaml
role: derived
lineage:
  inputs:
    - ../Data/events/**/*.parquet
  builder: ../Tasks/Build Summary.task/task.yaml
```

## Fallback conventions

Rich resources should offer a fallback:

- Markdown embed: normal link or text.
- Artifact: `README.md` and optional static preview.
- Ink: `preview.svg` or PDF.
- Canvas: JSON Canvas base file and linear reading order.
- Chart: Vega-Lite spec and SVG/PNG export.
- Data view: readable YAML plus source reference.
- App: built static assets where possible and source README.

## Capability terminology

Permissions should use verbs and resource scopes:

```yaml
workspace:
  read:
    - Research/**
  write:
    - Research/Generated/**
datasets:
  query:
    - Analytics/Usage.dataset/**
  mutate: []
network:
  hosts:
    - api.example.com
```

Avoid ambiguous `full_access` grants.

## Status vocabulary

Generated or executable resources may use:

```text
current
stale
building
failed
unavailable
untrusted
human-reviewed
human-edited
```

Data generation fields may use:

```text
fresh
stale
running
failed
human-edited
human-approved
```

## Product nouns

Prefer:

- Workspace.
- Notebook.
- Page.
- Dataset.
- Data app.
- Sheet.
- Canvas.
- View.
- Artifact.
- App.
- Query.
- Workflow.

Avoid making the product vocabulary revolve around:

- Second brain.
- Life operating system.
- AI teammate.
- Productivity score.
- Mandatory methodology.
