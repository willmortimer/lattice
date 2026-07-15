# Remote Data Connectors and Query

## Product opportunity

A first-class remote database viewer turns Lattice into a serious technical and analytical workspace rather than a local-only note/database app.

## Connector contract

Connectors expose optional capabilities:

```text
list
read
query
stream
mutate
subscribe
snapshot
schema
health
explain
cancel
```

Connector categories:

- PostgreSQL.
- MySQL/MariaDB.
- SQL Server.
- ClickHouse.
- Snowflake.
- BigQuery.
- Trino.
- DuckDB and SQLite.
- S3/object storage.
- Arrow Flight SQL.
- REST and GraphQL.
- OpenAPI-described endpoints.
- Prometheus, Loki, Tempo, OpenSearch, and Elasticsearch.
- Git, calendars, email, and Jupyter servers through domain plugins.

## Preferred data interface

Use ADBC where practical. Normalize tabular results to Arrow batches. Use native drivers when needed for source-specific capabilities.

## Access modes

### Live

Queries execute against the source at interaction time.

Best for current operational data and sources too large or sensitive to copy.

### Extract

Materialize a selected dataset locally as Parquet, SQLite, or DuckDB.

Best for:

- Offline use.
- Repeated fast analysis.
- Reproducible snapshots.
- Reducing production load.

### Composite

Combine remote facts with local dimensions, annotations, caches, and documents.

Example:

```sql
SELECT
    remote_orders.*,
    local_annotations.review_status,
    local_annotations.notes_page_id
FROM remote_postgres.orders AS remote_orders
LEFT JOIN local_sqlite.order_annotations AS local_annotations
ON remote_orders.id = local_annotations.order_id;
```

## Viewer UX

- Connection browser.
- Schema tree.
- Table and view preview.
- Relationship diagram.
- SQL editor.
- Saved queries.
- Query history.
- Explain-plan viewer.
- Result profiling.
- Result grid and charts.
- Create local extract.
- Create annotation layer.
- Send to notebook.
- Place on canvas.
- Export to Parquet/Arrow/CSV.
- AI-generated query with preview and source references.

## Query resources

Saved queries are files:

```text
Queries/Active Customers.sql
Queries/Active Customers.query.yaml
```

The sidecar may declare:

- Connector.
- Parameters.
- Output schema.
- Cache mode.
- Refresh policy.
- Row limits.
- Semantic-model context.

## Security defaults

- Read-only connections by default.
- Credentials stored in OS keychain or secret provider.
- Host allowlist.
- Query timeout and cancellation.
- Row/byte limits.
- No unrestricted SQL from untrusted artifacts.
- Explicit permission for writes and schema changes.
- Transaction preview for destructive or broad mutations.
- Production-write connections visually distinct.

## Snapshots and lineage

Extract manifests record:

- Source connection identity.
- Query or table.
- Snapshot time.
- Source revision if available.
- Schema.
- Row count.
- Output partition paths.
- Refresh policy.

## OpenAPI and GraphQL

OpenAPI can generate:

- Connector definitions.
- Request forms.
- Response schemas.
- Documentation pages.
- Typed app SDK clients.

GraphQL connectors expose schema browsing, saved operations, result profiling, and typed bindings.

## Object storage

Use OpenDAL or a comparable abstraction for S3, local filesystem, WebDAV, GCS, Azure, and other stores.

Object-store resources can be:

- Browsed.
- Mounted logically.
- Queried as Parquet.
- Snapshotted.
- Published.
- Bound to apps and notebooks.

## Remote execution

A query or notebook can target remote compute:

```yaml
execution:
  target: remote
  runner: analytics-cluster
```

Remote execution should use scoped credentials, bounded resources, Arrow-oriented results, and durable job logs.
