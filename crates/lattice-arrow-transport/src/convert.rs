use std::sync::Arc;

use arrow::array::{
    ArrayRef, BooleanBuilder, Float64Builder, Int64Builder, NullArray, StringBuilder,
};
use arrow::datatypes::{DataType as ArrowDataType, Field as ArrowField, Schema as ArrowSchema};
use arrow::record_batch::RecordBatch as ArrowRecordBatch;
use lattice_duckdb::{DataType, RecordBatch, ScalarValue};

use crate::schema_meta::{FieldMeta, SchemaMeta};
use crate::{Error, Result};

pub(crate) fn schema_meta_from_batch(batch: &RecordBatch) -> SchemaMeta {
    SchemaMeta {
        fields: batch
            .schema
            .fields
            .iter()
            .map(|field| FieldMeta {
                name: field.name.clone(),
                data_type: data_type_label(&field.data_type),
                nullable: field.nullable,
            })
            .collect(),
    }
}

pub(crate) fn to_arrow_batch(batch: &RecordBatch) -> Result<ArrowRecordBatch> {
    let fields: Vec<ArrowField> = batch
        .schema
        .fields
        .iter()
        .map(|field| {
            ArrowField::new(
                field.name.clone(),
                arrow_data_type(&field.data_type),
                field.nullable,
            )
        })
        .collect();
    let schema = Arc::new(ArrowSchema::new(fields));

    if batch.schema.fields.is_empty() {
        return Ok(ArrowRecordBatch::new_empty(schema));
    }

    let mut columns: Vec<ArrayRef> = Vec::with_capacity(batch.columns.len());
    for (field, column) in batch.schema.fields.iter().zip(&batch.columns) {
        if column.len() != batch.num_rows {
            return Err(Error::message(format!(
                "column {:?} length {} does not match num_rows {}",
                field.name,
                column.len(),
                batch.num_rows
            )));
        }
        columns.push(build_array(&field.data_type, column)?);
    }

    ArrowRecordBatch::try_new(schema, columns).map_err(|err| Error::arrow(err.to_string()))
}

fn data_type_label(data_type: &DataType) -> String {
    match data_type {
        DataType::Null => "null".to_string(),
        DataType::Boolean => "boolean".to_string(),
        DataType::Int64 => "int64".to_string(),
        DataType::Float64 => "float64".to_string(),
        DataType::Utf8 => "utf8".to_string(),
        DataType::Other(name) => format!("other:{name}"),
    }
}

fn arrow_data_type(data_type: &DataType) -> ArrowDataType {
    match data_type {
        DataType::Null => ArrowDataType::Null,
        DataType::Boolean => ArrowDataType::Boolean,
        DataType::Int64 => ArrowDataType::Int64,
        DataType::Float64 => ArrowDataType::Float64,
        // Preserve unknown DuckDB types as Utf8 until a dedicated mapping lands.
        DataType::Utf8 | DataType::Other(_) => ArrowDataType::Utf8,
    }
}

fn build_array(data_type: &DataType, column: &[ScalarValue]) -> Result<ArrayRef> {
    match data_type {
        DataType::Null => Ok(Arc::new(NullArray::new(column.len()))),
        DataType::Boolean => {
            let mut builder = BooleanBuilder::with_capacity(column.len());
            for value in column {
                match value {
                    ScalarValue::Null => builder.append_null(),
                    ScalarValue::Boolean(v) => builder.append_value(*v),
                    other => {
                        return Err(Error::message(format!(
                            "expected boolean scalar, got {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Int64 => {
            let mut builder = Int64Builder::with_capacity(column.len());
            for value in column {
                match value {
                    ScalarValue::Null => builder.append_null(),
                    ScalarValue::Int64(v) => builder.append_value(*v),
                    other => {
                        return Err(Error::message(format!(
                            "expected int64 scalar, got {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Float64 => {
            let mut builder = Float64Builder::with_capacity(column.len());
            for value in column {
                match value {
                    ScalarValue::Null => builder.append_null(),
                    ScalarValue::Float64(v) => builder.append_value(*v),
                    other => {
                        return Err(Error::message(format!(
                            "expected float64 scalar, got {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Utf8 | DataType::Other(_) => {
            let mut builder = StringBuilder::with_capacity(column.len(), column.len() * 8);
            for value in column {
                match value {
                    ScalarValue::Null => builder.append_null(),
                    ScalarValue::Utf8(v) => builder.append_value(v),
                    ScalarValue::Boolean(v) => builder.append_value(v.to_string()),
                    ScalarValue::Int64(v) => builder.append_value(v.to_string()),
                    ScalarValue::Float64(v) => builder.append_value(v.to_string()),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
    }
}

pub(crate) fn truncate_rows(batch: &RecordBatch, max_rows: usize) -> RecordBatch {
    if batch.num_rows <= max_rows {
        return batch.clone();
    }
    RecordBatch {
        schema: batch.schema.clone(),
        columns: batch
            .columns
            .iter()
            .map(|column| column[..max_rows].to_vec())
            .collect(),
        num_rows: max_rows,
    }
}
