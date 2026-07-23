# First Look — agent MCP transcript (AG2)

Sample `tools/call` sequence for `latticed` MCP against an open First Look
workspace. Replace `ROOT` with the absolute workspace path. These tools **read**
or **propose** only — apply stays in the desktop Proposals inbox.

Prerequisites: workspace seeded from the First Look template (`Data/Orders.dataset`,
`Data/Events.dataset`, `CRM.data`). See [first-look-demo.md](./first-look-demo.md).

## 1. Inspect schema (LIMIT 0)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_dataset_schema",
    "arguments": {
      "root": "ROOT",
      "path": "Data/Orders.dataset"
    }
  }
}
```

Repeat with `"path": "Data/Events.dataset"`. Response includes column names and
types without scanning Parquet facts.

## 2. Profile facts (bounded SUMMARIZE)

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "profile_dataset",
    "arguments": {
      "root": "ROOT",
      "path": "Data/Orders.dataset",
      "sample_rows": 500
    }
  }
}
```

Use the profile output to choose metrics or chart bindings before proposing.

## 3. Propose interface YAML

After inspecting Orders/Events, propose a CRM dashboard interface (validated;
writes `.lattice/proposals/{id}.json` only):

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "propose_interface",
    "arguments": {
      "root": "ROOT",
      "path": "CRM.data/interfaces/AgentDigest.interface.yaml",
      "content": "format: lattice-interface\nversion: 1\nname: AgentDigest\ntitle: Agent digest\ndescription: MCP-proposed digest over Orders and Events.\nlayout:\n  columns: 12\ncomponents:\n  - id: signups_total\n    type: metric\n    span: 6\n    title: Total signups (Events)\n    binding:\n      type: duckdb-query\n      resources:\n        - Data/Events.dataset\n      sql: |\n        SELECT COALESCE(SUM(signups), 0) AS value\n        FROM read_parquet('Data/Events.dataset/facts/**/*.parquet', hive_partitioning = true, union_by_name = true)\n      limit: 1\n  - id: revenue_total\n    type: metric\n    span: 6\n    title: Total revenue (Orders)\n    binding:\n      type: duckdb-query\n      resources:\n        - Data/Orders.dataset\n      sql: |\n        SELECT COALESCE(SUM(revenue), 0) AS value\n        FROM read_parquet('Data/Orders.dataset/facts/**/*.parquet', hive_partitioning = true, union_by_name = true)\n      limit: 1\n"
    }
  }
}
```

Alternative: `propose_workflow` with a `manual` trigger and `notification` step
when you only need to rehearse workflow validation.

## 4. Approve in desktop

1. Open the **Proposals** inbox in the native app.
2. Approve the pending bundle (`source.type: mcp` or `task`).
3. Open `CRM.data` → **Interfaces** → **Agent digest**.

## Task equivalent

`Tasks/AgentFirstLook.task` runs the same inspect → `propose_interface` flow via
the injected `lattice` package (no daemon). See [[Research/Agent first look]] in
the First Look workspace.

## HTTP API

The same payloads work on localhost HTTP (`POST /v1/datasets/schema`,
`POST /v1/datasets/profile`, `POST /v1/proposals/propose_interface`). See
[apps/daemon/README.md](../apps/daemon/README.md).
