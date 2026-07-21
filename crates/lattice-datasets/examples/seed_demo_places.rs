//! Seed First Look `Places.dataset` with WGS84 lon/lat point Parquet.
//!
//! Run from repo root:
//! ```sh
//! cargo run -p lattice-datasets --example seed_demo_places
//! pnpm compile-templates
//! ```

use std::path::{Path, PathBuf};

use lattice_datasets::Dataset;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../templates/workspaces/demo/files/Data/Places.dataset");
    let csv = root.join("sources/places.csv");
    assert!(
        csv.is_file(),
        "missing source CSV at {}",
        csv.display()
    );

    // Wipe previous facts so re-runs are idempotent.
    let facts = root.join("facts");
    if facts.exists() {
        std::fs::remove_dir_all(&facts).expect("remove facts/");
    }

    for folder in ["facts", "views", "queries"] {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| {
            panic!("create {}: {err}", dir.display());
        });
    }

    let mut dataset = Dataset::open(&root).expect("open Places.dataset");
    // Flat facts path (no Hive keys) — MapLibre reads plain lon/lat doubles.
    let entry = dataset
        .import_csv(&csv, &[], Some("places.parquet"))
        .expect("import_csv");
    println!("wrote {} ({} rows)", entry.path, entry.rows.unwrap_or(0));

    let discovered = dataset
        .discover_partitions()
        .expect("discover_partitions");
    println!(
        "manifest partitions: {}",
        discovered
            .iter()
            .map(|p| p.path.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    write_readme(&root);
    println!("seeded {}", root.display());
}

fn write_readme(root: &Path) {
    let body = r#"# Places

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
"#;
    std::fs::write(root.join("README.md"), body).expect("write README.md");
}
