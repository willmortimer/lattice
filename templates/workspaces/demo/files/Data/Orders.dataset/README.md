# Orders

Synthetic retail orders for the First Look analytics demo (~3 000 rows).

**Attribution:** Lattice-generated synthetic data for product demos — not a
third-party or production dump.

| Path | Role |
| --- | --- |
| `sources/orders.csv` | Editable source CSV (re-import via `seed_demo_orders`) |
| `facts/year=2026/month=0{1,2,3}/orders.parquet` | Hive Parquet partitions (DuckDB `read_parquet`) |

Columns: `order_id`, `ordered_at`, `region`, `category`, `channel`, `revenue`,
`units`. Date range spans January–March 2026 for multi-month time charts.

Re-seed from the repo root:

```sh
cargo run -p lattice-datasets --example seed_demo_orders
pnpm compile-templates
```
