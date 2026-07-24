//! Seed `Engineering/Build Status.dataset` with deterministic Hive Parquet.
//!
//! Run from the repository root:
//! ```sh
//! cargo run -p lattice-datasets --example seed_demo_build_status
//! pnpm compile-templates
//! ```

use std::path::{Path, PathBuf};

use lattice_datasets::Dataset;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../templates/workspaces/demo/files/Engineering/Build Status.dataset",
    );
    let csv = root.join("sources/builds.csv");
    assert!(csv.is_file(), "missing source CSV at {}", csv.display());

    let facts = root.join("facts");
    if facts.exists() {
        std::fs::remove_dir_all(&facts).expect("remove facts/");
    }
    for folder in ["facts", "views", "queries"] {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir)
            .unwrap_or_else(|err| panic!("create {}: {err}", dir.display()));
    }

    let mut dataset = Dataset::open(&root).expect("open Build Status.dataset");
    let keys = vec![
        ("year".to_string(), "2026".to_string()),
        ("month".to_string(), "07".to_string()),
    ];
    let entry = dataset
        .import_csv(&csv, &keys, Some("builds.parquet"))
        .expect("import builds.csv");
    println!("wrote {} ({} rows)", entry.path, entry.rows.unwrap_or(0));

    let discovered = dataset
        .discover_partitions()
        .expect("discover build partitions");
    assert_eq!(discovered.len(), 1, "expected one build partition");

    write_readme(&root);
    println!("seeded {}", root.display());
}

fn write_readme(root: &Path) {
    let body = r#"# Build status

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
"#;
    std::fs::write(root.join("README.md"), body).expect("write README.md");
}
