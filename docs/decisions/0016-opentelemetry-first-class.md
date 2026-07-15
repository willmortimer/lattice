# ADR 0016: OpenTelemetry is the internal observability model

## Status
Accepted

## Context
A platform with plugins, queries, kernels, workflows, sync, remote connectors, and multiple renderers is impossible to debug through unrelated text logs.

## Decision
Instrument Rust, frontend, plugins, tasks, queries, sync, and remote operations with OpenTelemetry-compatible traces, metrics, and logs. Provide a privacy-preserving local diagnostics workbench. Support optional OTLP export and telemetry connectors for Prometheus, Loki, Tempo, Grafana, and related systems.

## Consequences
- Cross-boundary performance and failure analysis becomes coherent.
- Telemetry schemas and privacy redaction are core contracts.
- External export is disabled by default.
