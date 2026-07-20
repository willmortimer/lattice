//! Seed First Look `Events.dataset` with Hive Parquet + annotation overlay.
//!
//! Run from repo root:
//! ```sh
//! cargo run -p lattice-datasets --example seed_demo_events
//! pnpm compile-templates
//! ```

use std::path::{Path, PathBuf};

use lattice_datasets::{Dataset, EventAnnotation};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../templates/workspaces/demo/files/Data/Events.dataset");
    let csv = root.join("sources/signups.csv");
    assert!(
        csv.is_file(),
        "missing source CSV at {}",
        csv.display()
    );

    // Wipe previous facts / annotations so re-runs are idempotent.
    let facts = root.join("facts");
    if facts.exists() {
        std::fs::remove_dir_all(&facts).expect("remove facts/");
    }
    let annotations = root.join("annotations.sqlite");
    if annotations.exists() {
        std::fs::remove_file(&annotations).expect("remove annotations.sqlite");
    }

    // Package already exists in the template tree — restore required empty dirs.
    for folder in ["facts", "views", "queries"] {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| {
            panic!("create {}: {err}", dir.display());
        });
    }

    let mut dataset = Dataset::open(&root).expect("open Events.dataset");
    let keys = vec![
        ("year".to_string(), "2026".to_string()),
        ("month".to_string(), "07".to_string()),
    ];
    let entry = dataset
        .import_csv(&csv, &keys, Some("signups.parquet"))
        .expect("import_csv");
    println!("wrote {}", entry.path);

    for annotation in [
        EventAnnotation::new(
            "evt-north",
            Some("keep".into()),
            Some("Strongest region in the sample month.".into()),
            true,
        ),
        EventAnnotation::new(
            "evt-west",
            Some("review".into()),
            Some("Lowest signups — check campaign coverage.".into()),
            false,
        ),
        EventAnnotation::new(
            "evt-central",
            Some("keep".into()),
            None,
            true,
        ),
    ] {
        dataset
            .upsert_annotation(&annotation)
            .expect("upsert annotation");
        println!("annotated {}", annotation.event_id);
    }

    write_readme(&root);
    println!("seeded {}", root.display());
}

fn write_readme(root: &Path) {
    let body = r#"# Events

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
"#;
    std::fs::write(root.join("README.md"), body).expect("write README.md");
}
