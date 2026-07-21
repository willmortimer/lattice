# Reference Manifests and Examples

This file collects compact examples. Specifications should eventually live under `specifications/` with full schemas.

## Workspace

```yaml
format: lattice-workspace
version: 1
id: 019b...
title: Example Workspace
capabilities:
  enabled: [pages, canvas, sqlite, parquet, jupyter]
```

## Data app

```yaml
format: lattice-data-app
version: 1
id: 019b...
title: CRM
database: ./database.sqlite
schema: ./schema.sql
migrations: ./migrations
tables:
  companies:
    identity: id
    display_field: name
    fields:
      status:
        semantic_type: enum
        control: single-select
      research_page_id:
        semantic_type: workspace-resource
        allowed_kinds: [page]
```

## View

```yaml
format: lattice-view
version: 1
source:
  database: ../database.sqlite
  table: companies
layout:
  type: board
  group_by: status
sort:
  - field: updated_at
    direction: descending
```

## Form

```yaml
format: lattice-form
version: 1
title: Add company
destination:
  database: ../database.sqlite
  table: companies
sections:
  - fields:
      - column: name
        required: true
      - column: website
```

## Canvas profile

```yaml
format: lattice-canvas-profile
version: 1
canvas: ./Project.canvas
reading_order: [brief, data, dashboard]
nodes:
  data:
    renderer: data-view
    resource: ./Data/Project.data/views/Overview.view.yaml
```

## Artifact

```yaml
format: lattice-artifact
version: 1
title: Interactive chart
entrypoint: ./index.html
bindings:
  data:
    type: duckdb-query
    resources: [../../Data/Orders.dataset]
    sql: SELECT * FROM read_parquet('../../Data/Orders.dataset/facts/**/*.parquet') LIMIT 100
    limit: 100
permissions:
  network: []
  workspace_write: []
fallback:
  file: ./README.md
```

## App

```yaml
format: lattice-app
version: 1
title: Portal
entrypoint: ./dist/index.html
source: ./src
framework:
  name: react
build:
  task: ./Build.task.yaml
capabilities:
  network: []
  workspace_read: [../../Assets/**]
```

## Task

```yaml
format: lattice-task
version: 1
runtime:
  type: python
  provider: uv
entrypoint:
  command: [python, main.py]
permissions:
  workspace:
    read: [../../Data/**]
    write: []
outputs:
  changes:
    type: proposed-transaction
```

## Workflow

```yaml
format: lattice-workflow
version: 1
trigger:
  type: schedule
  cron: "0 8 * * *"
  timezone: America/Los_Angeles
steps:
  - action: connector.refresh
    with:
      connector: analytics
  - action: artifact.build
    with:
      resource: ../../Artifacts/Dashboard.artifact/artifact.yaml
```

## Semantic model

```yaml
format: lattice-semantic-model
version: 1
sources:
  orders:
    relation: ./facts/**/*.parquet
measures:
  revenue:
    expression: sum(total_cents) / 100
    format: currency
dimensions:
  order_date:
    field: created_at
    hierarchy: [year, quarter, month, day]
```

## Ink manifest

```json
{
  "format": "lattice-ink",
  "version": 1,
  "id": "019b...",
  "strokes": "./strokes.arrow",
  "preview": "./preview.svg",
  "coordinateSystem": {"unit": "point", "width": 2048, "height": 1536}
}
```

## Documentation project

```yaml
format: lattice-docs-project
version: 1
title: Product Docs
content:
  root: .
  home: index.md
renderer:
  preset: starlight
sources:
  - type: markdown
    path: .
  - type: openapi
    path: ../openapi.yaml
    mount: /api
```

## Proposed transaction

```json
{
  "transactionId": "019b...",
  "summary": "Create research app",
  "preconditions": [],
  "operations": [
    {"type": "resource.create", "path": "Research/Overview.md"},
    {"type": "dataset.create", "path": "Research/Sources.data"},
    {"type": "canvas.place-resource", "canvas": "Research.canvas"}
  ]
}
```

## Plugin manifest

```toml
id = "org.example.plugin"
version = "1.0.0"
api_version = "1"

[permissions]
workspace_read = ["Research/**"]
network = ["api.example.com"]

[contributions]
commands = ["example.import"]
renderers = ["example-resource"]
```
