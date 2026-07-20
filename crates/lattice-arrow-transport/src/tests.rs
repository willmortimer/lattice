use arrow::array::{Array, Float64Array, Int64Array, StringArray};
use lattice_duckdb::{DataType, Field, RecordBatch, ScalarValue, Schema};

use crate::cancel::CancelCheck;
use crate::encode::{
    decode_ipc_stream, encode_duckdb_batch, encode_duckdb_batch_with_cancel, EncodeOptions,
};
use crate::Error;

fn sample_batch(rows: usize) -> RecordBatch {
    let mut ids = Vec::with_capacity(rows);
    let mut names = Vec::with_capacity(rows);
    let mut scores = Vec::with_capacity(rows);
    for index in 0..rows {
        ids.push(ScalarValue::Int64(index as i64));
        names.push(ScalarValue::Utf8(format!("row-{index}")));
        scores.push(ScalarValue::Float64(index as f64 * 1.5));
    }
    RecordBatch {
        schema: Schema {
            fields: vec![
                Field {
                    name: "id".into(),
                    data_type: DataType::Int64,
                    nullable: false,
                },
                Field {
                    name: "name".into(),
                    data_type: DataType::Utf8,
                    nullable: true,
                },
                Field {
                    name: "score".into(),
                    data_type: DataType::Float64,
                    nullable: true,
                },
            ],
        },
        columns: vec![ids, names, scores],
        num_rows: rows,
    }
}

#[test]
fn round_trip_n_row_batch_without_row_json() {
    let batch = sample_batch(128);
    let encoded = encode_duckdb_batch(&batch, &EncodeOptions::default()).unwrap();
    assert_eq!(encoded.row_count, 128);
    assert!(!encoded.truncated);
    assert!(!encoded.cancelled);
    assert_eq!(encoded.byte_length, encoded.ipc_bytes.len());
    assert_eq!(encoded.schema_meta.fields.len(), 3);
    assert_eq!(encoded.schema_meta.fields[0].name, "id");
    assert_eq!(encoded.schema_meta.fields[0].data_type, "int64");
    assert_eq!(encoded.sample_rows.len(), 5);
    assert_eq!(encoded.sample_rows[0][0], serde_json::json!(0));
    assert_eq!(encoded.sample_rows[0][1], serde_json::json!("row-0"));

    let decoded = decode_ipc_stream(&encoded.ipc_bytes).unwrap();
    assert_eq!(decoded.num_rows(), 128);
    assert_eq!(decoded.num_columns(), 3);

    let ids = decoded
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();
    let names = decoded
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let scores = decoded
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert_eq!(ids.value(0), 0);
    assert_eq!(ids.value(127), 127);
    assert_eq!(names.value(7), "row-7");
    assert_eq!(scores.value(2), 3.0);
}

#[test]
fn max_rows_truncates_and_sets_flag() {
    let batch = sample_batch(50);
    let encoded = encode_duckdb_batch(
        &batch,
        &EncodeOptions {
            max_rows: 10,
            max_bytes: 8 * 1024 * 1024,
            sample_rows: 5,
        },
    )
    .unwrap();
    assert_eq!(encoded.row_count, 10);
    assert!(encoded.truncated);
    let decoded = decode_ipc_stream(&encoded.ipc_bytes).unwrap();
    assert_eq!(decoded.num_rows(), 10);
}

#[test]
fn max_bytes_shrinks_row_count() {
    let batch = sample_batch(200);
    let encoded = encode_duckdb_batch(
        &batch,
        &EncodeOptions {
            max_rows: 200,
            // Force byte-budget truncation for a modest batch.
            max_bytes: 800,
            sample_rows: 5,
        },
    )
    .unwrap();
    assert!(encoded.truncated);
    assert!(encoded.row_count < 200);
    assert!(encoded.byte_length <= 800 || encoded.row_count == 1);
    let decoded = decode_ipc_stream(&encoded.ipc_bytes).unwrap();
    assert_eq!(decoded.num_rows(), encoded.row_count);
}

struct AlwaysCancel;

impl CancelCheck for AlwaysCancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}

#[test]
fn cancel_check_returns_cancelled_error() {
    let batch = sample_batch(4);
    let err = encode_duckdb_batch_with_cancel(&batch, &EncodeOptions::default(), &AlwaysCancel)
        .unwrap_err();
    assert!(matches!(err, Error::Cancelled));
}

#[test]
fn empty_batch_round_trips() {
    let batch = RecordBatch::empty();
    let encoded = encode_duckdb_batch(&batch, &EncodeOptions::default()).unwrap();
    assert_eq!(encoded.row_count, 0);
    assert!(!encoded.truncated);
    let decoded = decode_ipc_stream(&encoded.ipc_bytes).unwrap();
    assert_eq!(decoded.num_rows(), 0);
    assert_eq!(decoded.num_columns(), 0);
}
