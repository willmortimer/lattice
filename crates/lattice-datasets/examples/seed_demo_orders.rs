//! Seed First Look `Orders.dataset` with Hive Parquet facts from synthetic CSV.
//!
//! Run from repo root:
//! ```sh
//! cargo run -p lattice-datasets --example seed_demo_orders
//! pnpm compile-templates
//! ```

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use lattice_datasets::Dataset;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../templates/workspaces/demo/files/Data/Orders.dataset");
    let csv = root.join("sources/orders.csv");
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

    let month_csvs = split_csv_by_year_month(&csv).expect("split CSV by year/month");
    assert!(
        !month_csvs.is_empty(),
        "orders.csv produced no year/month partitions"
    );

    let mut dataset = Dataset::open(&root).expect("open Orders.dataset");
    for ((year, month), path) in &month_csvs {
        let keys = vec![
            ("year".to_string(), year.clone()),
            ("month".to_string(), month.clone()),
        ];
        let entry = dataset
            .import_csv(path, &keys, Some("orders.parquet"))
            .unwrap_or_else(|err| panic!("import_csv {}: {err}", path.display()));
        println!("wrote {} ({} rows)", entry.path, entry.rows.unwrap_or(0));
    }

    // Drop any stale manifest entries left from a prior seed shape.
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

    for (_, path) in &month_csvs {
        let _ = std::fs::remove_file(path);
    }
    if let Some(parent) = month_csvs.values().next().and_then(|p| p.parent()) {
        let _ = std::fs::remove_dir(parent);
    }

    write_readme(&root);
    println!("seeded {}", root.display());
}

/// Split `orders.csv` into temp CSVs keyed by `(year, month)` from `ordered_at`.
fn split_csv_by_year_month(csv: &Path) -> std::io::Result<BTreeMap<(String, String), PathBuf>> {
    let file = File::open(csv)?;
    let mut lines = BufReader::new(file).lines();
    let header = lines.next().transpose()?.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "orders.csv is empty")
    })?;
    let ordered_at_idx = header
        .split(',')
        .position(|col| col.trim() == "ordered_at")
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "orders.csv missing ordered_at column",
            )
        })?;

    let tmp_dir = csv
        .parent()
        .expect("sources/")
        .join(".seed-orders-tmp");
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir)?;
    }
    std::fs::create_dir_all(&tmp_dir)?;

    let mut writers: BTreeMap<(String, String), (PathBuf, File)> = BTreeMap::new();
    for line in lines {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let ordered_at = line
            .split(',')
            .nth(ordered_at_idx)
            .map(str::trim)
            .filter(|v| v.len() >= 7)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("row missing ordered_at: {line}"),
                )
            })?;
        let year = ordered_at[..4].to_string();
        let month = ordered_at[5..7].to_string();
        let key = (year.clone(), month.clone());
        if !writers.contains_key(&key) {
            let path = tmp_dir.join(format!("orders-{year}-{month}.csv"));
            let mut file = File::create(&path)?;
            writeln!(file, "{header}")?;
            writers.insert(key.clone(), (path, file));
        }
        let (_, file) = writers.get_mut(&key).expect("just inserted");
        writeln!(file, "{line}")?;
    }

    Ok(writers
        .into_iter()
        .map(|(key, (path, file))| {
            drop(file);
            (key, path)
        })
        .collect())
}

fn write_readme(root: &Path) {
    let body = r#"# Orders

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
"#;
    std::fs::write(root.join("README.md"), body).expect("write README.md");
}
