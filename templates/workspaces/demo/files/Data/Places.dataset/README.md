# Places

Sample geospatial `.dataset` for the First Look demo (~20 WGS84 points).

**Attribution:** Lattice-generated synthetic places for product demos — not a
third-party gazetteer dump.

| Path | Role |
| --- | --- |
| `sources/places.csv` | Editable source CSV (re-import via `seed_demo_places`) |
| `facts/places.parquet` | Point facts (`place_id`, `name`, `lon`, `lat`) |

Columns use plain `lon` / `lat` doubles (EPSG:4326 / WGS84) — enough for
MapLibre markers without DuckDB spatial extensions.

Re-seed from the repo root:

```sh
cargo run -p lattice-datasets --example seed_demo_places
pnpm compile-templates
```
