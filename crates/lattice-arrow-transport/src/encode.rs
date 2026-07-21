use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch as ArrowRecordBatch;
use lattice_duckdb::{RecordBatch, ScalarValue};
use serde::{Deserialize, Serialize};

use crate::cancel::{CancelCheck, NeverCancel};
use crate::convert::{schema_meta_from_batch, to_arrow_batch, truncate_rows};
use crate::limits::{DEFAULT_MAX_BYTES, DEFAULT_MAX_ROWS};
use crate::schema_meta::SchemaMeta;
use crate::{Error, Result};

/// Default number of preview rows attached as a JSON control message.
pub const DEFAULT_SAMPLE_ROWS: usize = 5;

/// Options for bounded Arrow IPC encoding.
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// Maximum rows retained in the encoded batch.
    pub max_rows: usize,
    /// Maximum encoded IPC byte length; row count shrinks until the payload fits.
    pub max_bytes: usize,
    /// Rows included in [`EncodedBatch::sample_rows`] (JSON control preview).
    pub sample_rows: usize,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            max_rows: DEFAULT_MAX_ROWS,
            max_bytes: DEFAULT_MAX_BYTES,
            sample_rows: DEFAULT_SAMPLE_ROWS,
        }
    }
}

/// Result of encoding a DuckDB columnar batch to Arrow IPC stream bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodedBatch {
    /// JSON-friendly schema summary (control message; not the full batch).
    pub schema_meta: SchemaMeta,
    /// Arrow IPC stream bytes. Tauri maps `Vec<u8>` to `Uint8Array` / `number[]`.
    pub ipc_bytes: Vec<u8>,
    /// Rows present in `ipc_bytes` after caps.
    pub row_count: usize,
    /// True when the source batch had more rows than were encoded.
    pub truncated: bool,
    /// True when encoding stopped early because [`CancelCheck`] fired.
    pub cancelled: bool,
    /// Encoded payload length (same as `ipc_bytes.len()`).
    pub byte_length: usize,
    /// Bounded JS-friendly preview (default ≤5 rows) for desktop dumps.
    /// The full batch must stay in `ipc_bytes`, not this field.
    pub sample_rows: Vec<Vec<serde_json::Value>>,
}

fn cancelled_batch(schema_meta: SchemaMeta) -> EncodedBatch {
    EncodedBatch {
        schema_meta,
        ipc_bytes: Vec::new(),
        row_count: 0,
        truncated: false,
        cancelled: true,
        byte_length: 0,
        sample_rows: Vec::new(),
    }
}

/// Encode a `lattice-duckdb` [`RecordBatch`] as a bounded Arrow IPC stream.
pub fn encode_duckdb_batch(batch: &RecordBatch, options: &EncodeOptions) -> Result<EncodedBatch> {
    encode_duckdb_batch_with_cancel(batch, options, &NeverCancel)
}

/// Encode with an explicit cancellation check (stub-friendly for UI cancel hooks).
///
/// When [`CancelCheck`] fires, returns `Ok` with `cancelled: true` and an empty
/// payload so callers can surface the response flag without treating cancel as a
/// hard transport failure.
pub fn encode_duckdb_batch_with_cancel(
    batch: &RecordBatch,
    options: &EncodeOptions,
    cancel: &dyn CancelCheck,
) -> Result<EncodedBatch> {
    if cancel.is_cancelled() {
        return Ok(cancelled_batch(SchemaMeta { fields: Vec::new() }));
    }

    let max_rows = options.max_rows.max(1);
    let max_bytes = options.max_bytes.max(1);
    let row_truncated = batch.num_rows > max_rows;
    let mut working = truncate_rows(batch, max_rows);
    let schema_meta = schema_meta_from_batch(&working);

    loop {
        if cancel.is_cancelled() {
            return Ok(cancelled_batch(schema_meta));
        }

        let arrow_batch = to_arrow_batch(&working)?;
        let ipc_bytes = write_ipc_stream(&arrow_batch)?;
        if ipc_bytes.len() <= max_bytes || working.num_rows <= 1 {
            let truncated = row_truncated || working.num_rows < batch.num_rows;
            let byte_length = ipc_bytes.len();
            let sample_rows = sample_rows_json(&working, options.sample_rows);
            return Ok(EncodedBatch {
                schema_meta,
                ipc_bytes,
                row_count: working.num_rows,
                truncated,
                cancelled: false,
                byte_length,
                sample_rows,
            });
        }

        // Shrink by half until the byte budget fits (minimum one row).
        let next_rows = (working.num_rows / 2).max(1);
        working = truncate_rows(&working, next_rows);
    }
}

fn write_ipc_stream(batch: &ArrowRecordBatch) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut buffer, batch.schema().as_ref())?;
        writer.write(batch)?;
        writer.finish()?;
    }
    Ok(buffer)
}

fn sample_rows_json(batch: &RecordBatch, limit: usize) -> Vec<Vec<serde_json::Value>> {
    let take = limit.min(batch.num_rows);
    let mut rows = Vec::with_capacity(take);
    for row_idx in 0..take {
        let mut row = Vec::with_capacity(batch.columns.len());
        for column in &batch.columns {
            row.push(scalar_to_json(&column[row_idx]));
        }
        rows.push(row);
    }
    rows
}

fn scalar_to_json(value: &ScalarValue) -> serde_json::Value {
    match value {
        ScalarValue::Null => serde_json::Value::Null,
        ScalarValue::Boolean(value) => serde_json::Value::Bool(*value),
        ScalarValue::Int64(value) => serde_json::json!(*value),
        ScalarValue::Float64(value) => serde_json::json!(*value),
        ScalarValue::Utf8(value) => serde_json::Value::String(value.clone()),
    }
}

/// Decode an Arrow IPC stream produced by [`encode_duckdb_batch`] (tests / round-trips).
pub fn decode_ipc_stream(bytes: &[u8]) -> Result<ArrowRecordBatch> {
    let mut reader = StreamReader::try_new(std::io::Cursor::new(bytes), None)?;
    let batch = reader
        .next()
        .ok_or_else(|| Error::message("IPC stream contained no record batches"))??;
    if reader.next().is_some() {
        return Err(Error::message(
            "IPC stream contained more than one record batch",
        ));
    }
    Ok(batch)
}
