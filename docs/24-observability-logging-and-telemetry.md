# Observability, Logging, and Telemetry

## Two responsibilities

1. Lattice must be observable itself.
2. Lattice should consume and present external operational telemetry.

## OpenTelemetry instrumentation

Instrument from the beginning:

- Rust commands and transactions.
- File reads/writes and reconciliation.
- SQLite/DuckDB queries.
- Arrow transfer.
- Frontend rendering and long tasks.
- Plugins, artifacts, apps, and scripts.
- Workflows and schedules.
- Sync operations.
- Remote connectors.
- Jupyter kernels.
- Publishing and builds.

A single action should carry trace context across boundaries:

```text
Refresh dashboard
  └── command
      ├── workflow
      ├── remote query
      ├── Arrow transfer
      ├── artifact build
      └── canvas repaint
```

Useful attributes:

```text
workspace_id
resource_id
command_id
transaction_id
plugin_id
task_id
query_id
connector_id
```

Do not include content or secret values by default.

## Local diagnostics store

```text
.lattice/diagnostics/
├── traces/
├── crashes/
├── profiles/
└── diagnostics.sqlite
```

A bounded local store supports:

- Structured logs.
- Traces and spans.
- Query timings and plans.
- Slow frame diagnostics.
- Memory by capability.
- Plugin/app errors.
- Workflow history.
- Kernel activity.
- Sync queues.
- Cache behavior.
- File watcher events.

## Diagnostics Workbench

Built-in views:

- Problems panel.
- Log explorer.
- Trace waterfall.
- Query profiler.
- Job history.
- Plugin console.
- Artifact/app console.
- Sync inspector.
- Memory/capability inspector.
- Crash and recovery viewer.

No Grafana deployment is required to diagnose Lattice.

## External telemetry export

Optional OTLP export to:

- OpenTelemetry Collector.
- Grafana stack.
- Jaeger.
- Tempo.
- Prometheus-compatible metrics pipeline.
- Organization observability platform.

Export is opt-in or administrator-configured.

## Telemetry ingestion

`latticed` may expose local OTLP receivers and ingest:

- OTLP traces, logs, metrics.
- Prometheus metrics.
- Loki logs.
- Tempo/Jaeger traces.
- OpenSearch/Elasticsearch.
- ClickHouse telemetry tables.
- systemd journal.
- JSONL logs.

Normalize to Arrow-oriented schemas and render through log, trace, chart, notebook, and SQL views.

## Grafana support

### Web embed

Generic authenticated web embed for existing dashboards.

### Native connector

Prefer native access to Grafana APIs or underlying Prometheus/Loki/Tempo data when Lattice needs:

- Offline snapshots.
- Cross-filtering.
- Export.
- AI/MCP access.
- Combined workspace analysis.
- Consistent theme.

## Observability use cases

- Drop JSONL logs on a canvas.
- Query logs with DuckDB.
- Open trace waterfall beside architecture docs.
- Link incident page to telemetry snapshot.
- Build live service dashboard.
- Send selected traces to Jupyter.
- Correlate deploy records with latency.

## Privacy

Defaults:

- No document bodies.
- No SQL result values.
- Hashed or omitted paths where possible.
- Attribute allowlist.
- Local redaction before export.
- Per-connector content rules.
- User-accessible telemetry preview.

## Performance observability

Every performance budget should have measurable spans or counters:

- Launch phases.
- Editor readiness.
- Search latency.
- Canvas frame times.
- Worker queue depth.
- Query time and bytes scanned.
- Arrow bytes transferred.
- WebView count and memory.
- Plugin CPU time.
- Cache hit rates.
