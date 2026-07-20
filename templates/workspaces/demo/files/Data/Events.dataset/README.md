# Events

Sample analytical `.dataset` for the First Look demo.

| Path | Role |
| --- | --- |
| `sources/signups.csv` | Editable source CSV (re-import via `seed_demo_events`) |
| `facts/year=2026/month=07/signups.parquet` | Hive Parquet partition (DuckDB `read_parquet`) |
| `annotations.sqlite` | Review overlay (`event_annotations`) |

Open this package in the desktop app for **Preview** (Perspective), **Chart**
(Vega-Lite), and **Profile** (DuckDB `SUMMARIZE`). Or open
`Dashboards/Signups by region.vl.json` for a bound chart resource.

Re-seed from the repo root:

```sh
cargo run -p lattice-datasets --example seed_demo_events
pnpm compile-templates
```
