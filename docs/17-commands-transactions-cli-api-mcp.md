# Commands, Transactions, CLI, API, and MCP

## Core rule

Every important GUI action is a semantic command handled by the Rust core.

Examples:

```text
page.create
page.append-section
resource.move
canvas.place-resource
dataset.create
dataset.insert-records
dataset.alter-schema
view.create
query.execute
notebook.run-cell
artifact.build
workflow.run
publish.deploy
```

## Command envelope

```json
{
  "command": "view.create",
  "workspace": "019b...",
  "input": {
    "source": "Research/Companies.data/database.sqlite",
    "table": "companies",
    "layout": "board",
    "groupBy": "category"
  },
  "idempotencyKey": "019b..."
}
```

## Transaction model

A command may produce one or more operations:

```json
{
  "transactionId": "019b...",
  "summary": "Create company research page and link it",
  "preconditions": [
    {"resource": "lattice://...", "revision": "sha256:..."}
  ],
  "operations": [
    {"type": "page.create", "path": "Research/Companies/Example.md"},
    {"type": "dataset.update-record", "table": "companies", "id": "example"}
  ]
}
```

Transactions include:

- Preconditions.
- Permission requirements.
- Validation warnings.
- Human-readable diff.
- Structured operations.
- Idempotency.
- Audit metadata.
- Undo/compensation information.

## Proposed transactions

Scripts, apps, workflows, and agents should prefer returning a proposal rather than writing directly.

The desktop persists general transaction proposals under
`<workspace>/.lattice/proposals/` (sibling to link-repair). Review accepts a
command subset into one `CommandEngine` transaction; reject/dismiss removes the
pending file. MCP and the localhost HTTP API can create and inspect proposals
(`create_proposal`, `list_proposals`, `get_proposal`, `propose_page`) with
`source.type: mcp`; they do not apply proposals. Apply remains desktop-only.

The review UI shows:

- Files changed.
- Blocks changed.
- Rows inserted/updated/deleted.
- Schema changes.
- App or artifact source generated.
- Network or secret permissions.
- Estimated affected row count.

Policies may auto-approve trusted narrow operations.

## CLI

```bash
lattice init
lattice validate
lattice search "distributed tracing"
lattice page create "Incident Review"
lattice query run Queries/Active.sql
lattice view create --source Data/CRM.data --table customers --layout board
lattice task run Scripts/Normalize.task
lattice context build --canvas "Product Strategy" --output context.md
lattice docs build Docs/
```

CLI output should support human, JSON, and machine-stream modes.

## Local API

Versioned HTTP API:

```text
GET  /v1/resources
GET  /v1/resources/{id}
POST /v1/commands/{command}
POST /v1/transactions/preview
POST /v1/transactions/commit
GET  /v1/jobs/{id}
GET  /v1/streams/query/{id}
```

Use streaming or custom local protocols for large Arrow and binary outputs rather than JSON.

## MCP

### Transports

- Local stdio.
- Local authenticated HTTP.
- Remote OAuth-protected endpoint.

### Resources

```text
lattice://workspace/current
lattice://workspace/current/pages
lattice://workspace/current/resource/{id}
lattice://workspace/current/dataset/{id}/schema
lattice://workspace/current/canvas/{id}/outline
lattice://workspace/current/search?q=deployment
```

### Tools

Read:

```text
search_workspace
get_page
get_outline
get_blocks
get_backlinks
list_relationship_edges
get_dataset_schema
query_dataset
get_canvas_outline
get_recent_changes
```

Mutation:

```text
create_page
update_page
insert_blocks
move_resource
create_dataset
alter_dataset
insert_records
create_view
place_resource
create_artifact
run_task
propose_transaction
```

### Context representations

Agents can request:

- Markdown.
- Plain text.
- Outline.
- Structured AST.
- Changed blocks only.
- Dataset schema and sample.
- Query aggregate.
- Canvas linear outline.
- Artifact README/manifest.

## Direct file escape hatch

The filesystem remains available to external tools. The command model provides validation, undo, permissions, and semantic conflict handling but is not compulsory.

## Events and subscriptions

API clients can subscribe to semantic events:

```text
resource.created
resource.changed
selection.changed
dataset.updated
query.completed
workflow.failed
kernel.started
sync.completed
telemetry.received
```

No client should depend on private React events or DOM structure.

## Versioning

- Version commands and schemas.
- Advertise supported capabilities.
- Provide deprecation windows.
- Keep format and API migrations separate.
- Publish conformance fixtures.
