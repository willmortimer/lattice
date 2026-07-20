//! Hive-style Parquet partitions under `facts/`.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use arrow::array::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use walkdir::WalkDir;

use crate::error::Error;
use crate::manifest::{dataset_manifest_path, PartitionEntry};
use crate::package::{Dataset, FACTS_DIR};
use crate::Result;

pub const DEFAULT_PARTITION_FILE: &str = "part-000.parquet";

/// Build a package-relative `facts/` path from ordered Hive keys.
///
/// Keys are emitted in caller order as `key=value` path segments.
pub fn hive_facts_relative_path(keys: &[(String, String)], file_name: &str) -> String {
    let mut segments = Vec::with_capacity(keys.len() + 2);
    segments.push(FACTS_DIR.to_string());
    for (key, value) in keys {
        segments.push(format!("{key}={value}"));
    }
    segments.push(file_name.to_string());
    segments.join("/")
}

/// Parse partition key pairs from CLI-style `key=value` strings.
pub fn parse_partition_key_specs(specs: &[String]) -> Result<Vec<(String, String)>> {
    let mut keys = Vec::with_capacity(specs.len());
    for spec in specs {
        let (key, value) = spec.split_once('=').ok_or_else(|| {
            Error::invalid_argument(format!("expected partition key=value, got {spec:?}"))
        })?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(Error::invalid_argument(format!(
                "partition keys and values must be non-empty: {spec:?}"
            )));
        }
        keys.push((key.to_string(), value.to_string()));
    }
    Ok(keys)
}

/// Parse `key=value` segments from a package-relative facts path.
pub fn hive_keys_from_relative_path(relative: &str) -> BTreeMap<String, String> {
    let mut keys = BTreeMap::new();
    for segment in relative.split(['/', '\\']) {
        if let Some((key, value)) = segment.split_once('=') {
            if !key.is_empty() && !value.is_empty() {
                keys.insert(key.to_string(), value.to_string());
            }
        }
    }
    keys
}

/// Normalize a path to package-relative `/`-separated form under `facts/`.
pub fn normalize_facts_relative(path: &Path, package_root: &Path) -> Result<String> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        package_root.join(path)
    };
    let rel = abs
        .strip_prefix(package_root)
        .map_err(|_| {
            Error::invalid_package(
                package_root,
                format!("path {} is outside the dataset package", abs.display()),
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");
    if !rel.starts_with(&format!("{FACTS_DIR}/")) && rel != FACTS_DIR {
        return Err(Error::invalid_package(
            package_root,
            format!("parquet path must be under {FACTS_DIR}/, got {rel}"),
        ));
    }
    Ok(rel)
}

impl Dataset {
    /// Write a RecordBatch to a Hive-style path under `facts/` and update the manifest.
    pub fn write_partition_batch(
        &mut self,
        keys: &[(String, String)],
        batch: &RecordBatch,
        file_name: Option<&str>,
    ) -> Result<PartitionEntry> {
        self.write_partition_batches(keys, std::slice::from_ref(batch), file_name)
    }

    /// Write one or more RecordBatches (same schema) to a Hive-style Parquet file.
    pub fn write_partition_batches(
        &mut self,
        keys: &[(String, String)],
        batches: &[RecordBatch],
        file_name: Option<&str>,
    ) -> Result<PartitionEntry> {
        self.validate_partition_keys(keys)?;
        if batches.is_empty() {
            return Err(Error::invalid_package(
                self.path(),
                "cannot write an empty partition (no record batches)",
            ));
        }
        let file_name = file_name.unwrap_or(DEFAULT_PARTITION_FILE);
        if !file_name.ends_with(".parquet") {
            return Err(Error::invalid_package(
                self.path(),
                format!("partition file name must end with .parquet, got {file_name}"),
            ));
        }
        let relative = hive_facts_relative_path(keys, file_name);
        self.write_batches_at_relative(&relative, batches)
    }

    /// Write RecordBatches to an explicit package-relative path under `facts/`.
    pub fn write_batches_at_relative(
        &mut self,
        relative_path: &str,
        batches: &[RecordBatch],
    ) -> Result<PartitionEntry> {
        let relative = normalize_facts_relative(Path::new(relative_path), self.path())?;
        if !relative.ends_with(".parquet") {
            return Err(Error::invalid_package(
                self.path(),
                format!("expected a .parquet path, got {relative}"),
            ));
        }
        if batches.is_empty() {
            return Err(Error::invalid_package(
                self.path(),
                "cannot write an empty partition (no record batches)",
            ));
        }

        let abs = package_join(self.path(), &relative);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
        }

        let schema = batches[0].schema();
        let file = File::create(&abs).map_err(|source| Error::io(&abs, source))?;
        let mut writer =
            ArrowWriter::try_new(file, schema, None).map_err(|source| Error::parquet(&abs, source))?;
        let mut row_count = 0u64;
        for batch in batches {
            writer
                .write(batch)
                .map_err(|source| Error::parquet(&abs, source))?;
            row_count += batch.num_rows() as u64;
        }
        writer
            .close()
            .map_err(|source| Error::parquet(&abs, source))?;

        let meta = std::fs::metadata(&abs).map_err(|source| Error::io(&abs, source))?;
        let entry = PartitionEntry {
            path: relative.clone(),
            keys: hive_keys_from_relative_path(&relative),
            rows: Some(row_count),
            bytes: Some(meta.len()),
        };
        self.persist_partition(entry)
    }

    /// Read all RecordBatches from a package-relative Parquet path under `facts/`.
    pub fn read_partition(&self, relative_path: &str) -> Result<Vec<RecordBatch>> {
        let relative = normalize_facts_relative(Path::new(relative_path), self.path())?;
        let abs = package_join(self.path(), &relative);
        if !abs.is_file() {
            return Err(Error::invalid_package(
                self.path(),
                format!("missing parquet file {relative}"),
            ));
        }

        let file = File::open(&abs).map_err(|source| Error::io(&abs, source))?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|source| Error::parquet(&abs, source))?;
        let reader = builder
            .build()
            .map_err(|source| Error::parquet(&abs, source))?;

        let mut batches = Vec::new();
        for batch in reader {
            batches.push(batch.map_err(|source| Error::arrow(&abs, source.to_string()))?);
        }
        Ok(batches)
    }

    /// Scan `facts/**/*.parquet`, refresh the manifest partition list, and return it.
    pub fn discover_partitions(&mut self) -> Result<Vec<PartitionEntry>> {
        let facts = self.path().join(FACTS_DIR);
        let mut entries = Vec::new();

        if facts.is_dir() {
            for walk in WalkDir::new(&facts).follow_links(false).into_iter() {
                let entry = walk.map_err(|source| {
                    Error::io(&facts, std::io::Error::other(source.to_string()))
                })?;
                if !entry.file_type().is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy();
                if !name.ends_with(".parquet") {
                    continue;
                }
                let relative = normalize_facts_relative(entry.path(), self.path())?;
                let meta = entry
                    .metadata()
                    .map_err(|source| Error::io(entry.path(), source.into()))?;
                let rows = parquet_row_count(entry.path())?;
                entries.push(PartitionEntry {
                    path: relative.clone(),
                    keys: hive_keys_from_relative_path(&relative),
                    rows: Some(rows),
                    bytes: Some(meta.len()),
                });
            }
        }

        entries.sort_by(|a, b| a.path.cmp(&b.path));
        self.manifest_mut().partitions = entries.clone();
        self.save_manifest()?;
        Ok(entries)
    }

    pub(crate) fn persist_partition(&mut self, entry: PartitionEntry) -> Result<PartitionEntry> {
        self.manifest_mut().upsert_partition(entry.clone());
        self.save_manifest()?;
        Ok(entry)
    }

    pub(crate) fn save_manifest(&self) -> Result<()> {
        let path = dataset_manifest_path(self.path());
        self.manifest().save(&path)
    }

    fn validate_partition_keys(&self, keys: &[(String, String)]) -> Result<()> {
        for (key, value) in keys {
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                return Err(Error::invalid_package(
                    self.path(),
                    "partition keys and values must be non-empty",
                ));
            }
            if key.contains(['=', '/', '\\']) || value.contains(['=', '/', '\\']) {
                return Err(Error::invalid_package(
                    self.path(),
                    format!("partition key/value may not contain '=', '/', or '\\\\': {key}={value}"),
                ));
            }
        }
        Ok(())
    }
}

fn package_join(package_root: &Path, relative_slash: &str) -> PathBuf {
    let mut abs = package_root.to_path_buf();
    for segment in relative_slash.split('/') {
        if !segment.is_empty() {
            abs.push(segment);
        }
    }
    abs
}

fn parquet_row_count(path: &Path) -> Result<u64> {
    let file = File::open(path).map_err(|source| Error::io(path, source))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|source| Error::parquet(path, source))?;
    Ok(builder.metadata().file_metadata().num_rows() as u64)
}
