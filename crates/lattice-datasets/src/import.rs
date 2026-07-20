//! CSV → Parquet import into a `.dataset` package `facts/` tree.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::csv::reader::Format;
use arrow::csv::ReaderBuilder;
use arrow::datatypes::Schema;

use crate::error::Error;
use crate::manifest::PartitionEntry;
use crate::package::Dataset;
use crate::partition::DEFAULT_PARTITION_FILE;
use crate::Result;

impl Dataset {
    /// Import a CSV file into a Hive-style Parquet partition under `facts/`.
    ///
    /// Column types are inferred from the CSV body (Arrow CSV inference).
    /// Partition columns should not appear as CSV columns; they live in the path.
    pub fn import_csv(
        &mut self,
        csv_path: &Path,
        keys: &[(String, String)],
        file_name: Option<&str>,
    ) -> Result<PartitionEntry> {
        let batch_schema = infer_csv_schema(csv_path)?;
        let file = File::open(csv_path).map_err(|source| Error::io(csv_path, source))?;
        let mut reader = ReaderBuilder::new(batch_schema)
            .with_header(true)
            .build(file)
            .map_err(|source| Error::csv(csv_path, source.to_string()))?;

        let mut batches = Vec::new();
        while let Some(batch) = reader.next() {
            batches.push(batch.map_err(|source| Error::csv(csv_path, source.to_string()))?);
        }

        if batches.is_empty() {
            return Err(Error::csv(
                csv_path,
                "CSV produced no record batches (empty file?)",
            ));
        }

        self.write_partition_batches(
            keys,
            &batches,
            Some(file_name.unwrap_or(DEFAULT_PARTITION_FILE)),
        )
    }
}

fn infer_csv_schema(csv_path: &Path) -> Result<Arc<Schema>> {
    let mut file = File::open(csv_path).map_err(|source| Error::io(csv_path, source))?;
    let format = Format::default().with_header(true);
    let (schema, _) = format
        .infer_schema(&mut file, Some(1024))
        .map_err(|source| Error::csv(csv_path, source.to_string()))?;
    Ok(Arc::new(schema))
}
