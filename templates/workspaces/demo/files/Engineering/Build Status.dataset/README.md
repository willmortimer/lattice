---
title: Build status
export_policy: allow
---

# Build status

Deterministic synthetic CI runs for the Lattice engineering demo.

| Path | Role |
| --- | --- |
| `sources/builds.csv` | Inspectable source rows |
| `facts/year=2026/month=07/builds.parquet` | Hive Parquet facts |

Columns include workflow, branch, runner, outcome, duration, test count and
failure count. Open the package for Perspective Preview, Vega-Lite Chart,
DuckDB Profile and EXPLAIN Plan.

Re-seed from the repository root:

```sh
cargo run -p lattice-datasets --example seed_demo_build_status
pnpm compile-templates
```
