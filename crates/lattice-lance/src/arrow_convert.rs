use std::sync::Arc;

use lancedb::arrow::arrow_array::builder::{
    FixedSizeListBuilder, Float32Builder, Int64Builder, StringBuilder, UInt32Builder,
    UInt64Builder,
};
use lancedb::arrow::arrow_array::{
    Array, FixedSizeListArray, Float32Array, Int64Array, RecordBatch, RecordBatchIterator,
    StringArray, UInt32Array, UInt64Array,
};
use lancedb::arrow::arrow_schema::{DataType, Field, Schema};

use crate::error::{LanceError, Result};
use crate::types::{SearchElementBatch, SearchElementRow, DEFAULT_ELEMENT_KIND};

pub(crate) const COL_ELEMENT_ID: &str = "element_id";
pub(crate) const COL_WORKSPACE_ID: &str = "workspace_id";
pub(crate) const COL_RESOURCE_ID: &str = "resource_id";
pub(crate) const COL_RESOURCE_VERSION_ID: &str = "resource_version_id";
pub(crate) const COL_ELEMENT_KIND: &str = "element_kind";
pub(crate) const COL_ORDINAL: &str = "ordinal";
pub(crate) const COL_TEXT: &str = "text";
pub(crate) const COL_EMBEDDING: &str = "embedding";
pub(crate) const COL_SOURCE_START_BYTE: &str = "source_start_byte";
pub(crate) const COL_SOURCE_END_BYTE: &str = "source_end_byte";
pub(crate) const COL_CONTENT_HASH: &str = "content_hash";
pub(crate) const COL_EMBEDDING_MODEL: &str = "embedding_model";
pub(crate) const COL_EMBEDDING_VERSION: &str = "embedding_version";
pub(crate) const COL_NAMESPACE_KEY: &str = "namespace_key";
pub(crate) const COL_DIMS: &str = "dims";
pub(crate) const COL_CREATED_AT_MS: &str = "created_at_ms";

pub(crate) fn search_elements_schema(dims: u32) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new(COL_ELEMENT_ID, DataType::Utf8, false),
        Field::new(COL_WORKSPACE_ID, DataType::Utf8, false),
        Field::new(COL_RESOURCE_ID, DataType::Utf8, false),
        Field::new(COL_RESOURCE_VERSION_ID, DataType::Utf8, true),
        Field::new(COL_ELEMENT_KIND, DataType::Utf8, false),
        Field::new(COL_ORDINAL, DataType::Int64, false),
        Field::new(COL_TEXT, DataType::Utf8, false),
        Field::new(
            COL_EMBEDDING,
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dims as i32,
            ),
            false,
        ),
        Field::new(COL_SOURCE_START_BYTE, DataType::UInt64, false),
        Field::new(COL_SOURCE_END_BYTE, DataType::UInt64, false),
        Field::new(COL_CONTENT_HASH, DataType::Utf8, false),
        Field::new(COL_EMBEDDING_MODEL, DataType::Utf8, false),
        Field::new(COL_EMBEDDING_VERSION, DataType::Utf8, false),
        Field::new(COL_NAMESPACE_KEY, DataType::Utf8, false),
        Field::new(COL_DIMS, DataType::UInt32, false),
        Field::new(COL_CREATED_AT_MS, DataType::Int64, false),
    ]))
}

pub(crate) fn validate_batch(batch: &SearchElementBatch) -> Result<u32> {
    if batch.rows.is_empty() {
        return Err(LanceError::invalid_input("append batch is empty"));
    }

    let dims = batch.rows[0].dims;
    if dims == 0 {
        return Err(LanceError::invalid_input("dims must be greater than zero"));
    }

    for row in &batch.rows {
        if row.dims != dims {
            return Err(LanceError::invalid_input(format!(
                "inconsistent dims in batch: expected {dims}, found {}",
                row.dims
            )));
        }
        if row.embedding.len() != dims as usize {
            return Err(LanceError::invalid_input(format!(
                "embedding length {} does not match dims {} for element_id {}",
                row.embedding.len(),
                row.dims,
                row.element_id
            )));
        }
    }

    Ok(dims)
}

pub(crate) fn rows_to_record_batch(batch: &SearchElementBatch, dims: u32) -> Result<RecordBatch> {
    let schema = search_elements_schema(dims);
    let row_count = batch.rows.len();

    let mut element_id = StringBuilder::with_capacity(row_count, row_count * 16);
    let mut workspace_id = StringBuilder::with_capacity(row_count, row_count * 8);
    let mut resource_id = StringBuilder::with_capacity(row_count, row_count * 8);
    let mut resource_version_id = StringBuilder::new();
    let mut element_kind = StringBuilder::with_capacity(row_count, row_count * 8);
    let mut ordinal = Int64Builder::with_capacity(row_count);
    let mut text = StringBuilder::new();
    let mut embedding =
        FixedSizeListBuilder::with_capacity(Float32Builder::new(), dims as i32, row_count);
    let mut source_start_byte = UInt64Builder::with_capacity(row_count);
    let mut source_end_byte = UInt64Builder::with_capacity(row_count);
    let mut content_hash = StringBuilder::new();
    let mut embedding_model = StringBuilder::new();
    let mut embedding_version = StringBuilder::new();
    let mut namespace_key = StringBuilder::with_capacity(row_count, row_count * 8);
    let mut dims_builder = UInt32Builder::with_capacity(row_count);
    let mut created_at_ms = Int64Builder::with_capacity(row_count);

    for row in &batch.rows {
        element_id.append_value(&row.element_id);
        workspace_id.append_value(&row.workspace_id);
        resource_id.append_value(&row.resource_id);
        if let Some(version_id) = &row.resource_version_id {
            resource_version_id.append_value(version_id);
        } else {
            resource_version_id.append_null();
        }
        element_kind.append_value(&row.element_kind);
        ordinal.append_value(row.ordinal);
        text.append_value(&row.text);
        for value in &row.embedding {
            embedding.values().append_value(*value);
        }
        embedding.append(true);
        source_start_byte.append_value(row.source_start_byte);
        source_end_byte.append_value(row.source_end_byte);
        content_hash.append_value(&row.content_hash);
        embedding_model.append_value(&row.embedding_model);
        embedding_version.append_value(&row.embedding_version);
        namespace_key.append_value(&row.namespace_key);
        dims_builder.append_value(row.dims);
        created_at_ms.append_value(row.created_at_ms);
    }

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(element_id.finish()),
            Arc::new(workspace_id.finish()),
            Arc::new(resource_id.finish()),
            Arc::new(resource_version_id.finish()),
            Arc::new(element_kind.finish()),
            Arc::new(ordinal.finish()),
            Arc::new(text.finish()),
            Arc::new(embedding.finish()),
            Arc::new(source_start_byte.finish()),
            Arc::new(source_end_byte.finish()),
            Arc::new(content_hash.finish()),
            Arc::new(embedding_model.finish()),
            Arc::new(embedding_version.finish()),
            Arc::new(namespace_key.finish()),
            Arc::new(dims_builder.finish()),
            Arc::new(created_at_ms.finish()),
        ],
    )
    .map_err(|err| LanceError::Store {
        message: format!("failed to build search-elements record batch: {err}"),
    })
}

pub(crate) fn record_batch_reader(
    batch: RecordBatch,
) -> Box<dyn lancedb::arrow::arrow_array::RecordBatchReader + Send> {
    let schema = batch.schema();
    Box::new(RecordBatchIterator::new(vec![Ok(batch)].into_iter(), schema))
}

pub(crate) fn record_batches_to_rows(batches: Vec<RecordBatch>) -> Result<Vec<SearchElementRow>> {
    let mut rows = Vec::new();
    for batch in batches {
        rows.extend(record_batch_to_rows(&batch)?);
    }
    Ok(rows)
}

fn record_batch_to_rows(batch: &RecordBatch) -> Result<Vec<SearchElementRow>> {
    let element_ids = batch
        .column_by_name(COL_ELEMENT_ID)
        .ok_or_else(|| missing_column(COL_ELEMENT_ID))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_ELEMENT_ID))?;
    let workspace_ids = batch
        .column_by_name(COL_WORKSPACE_ID)
        .ok_or_else(|| missing_column(COL_WORKSPACE_ID))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_WORKSPACE_ID))?;
    let resource_ids = batch
        .column_by_name(COL_RESOURCE_ID)
        .ok_or_else(|| missing_column(COL_RESOURCE_ID))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_RESOURCE_ID))?;
    let resource_version_ids = batch
        .column_by_name(COL_RESOURCE_VERSION_ID)
        .ok_or_else(|| missing_column(COL_RESOURCE_VERSION_ID))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_RESOURCE_VERSION_ID))?;
    let element_kinds = batch
        .column_by_name(COL_ELEMENT_KIND)
        .ok_or_else(|| missing_column(COL_ELEMENT_KIND))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_ELEMENT_KIND))?;
    let ordinals = batch
        .column_by_name(COL_ORDINAL)
        .ok_or_else(|| missing_column(COL_ORDINAL))?
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| invalid_column(COL_ORDINAL))?;
    let texts = batch
        .column_by_name(COL_TEXT)
        .ok_or_else(|| missing_column(COL_TEXT))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_TEXT))?;
    let embeddings = batch
        .column_by_name(COL_EMBEDDING)
        .ok_or_else(|| missing_column(COL_EMBEDDING))?
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .ok_or_else(|| invalid_column(COL_EMBEDDING))?;
    let source_start_bytes = batch
        .column_by_name(COL_SOURCE_START_BYTE)
        .ok_or_else(|| missing_column(COL_SOURCE_START_BYTE))?
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| invalid_column(COL_SOURCE_START_BYTE))?;
    let source_end_bytes = batch
        .column_by_name(COL_SOURCE_END_BYTE)
        .ok_or_else(|| missing_column(COL_SOURCE_END_BYTE))?
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| invalid_column(COL_SOURCE_END_BYTE))?;
    let content_hashes = batch
        .column_by_name(COL_CONTENT_HASH)
        .ok_or_else(|| missing_column(COL_CONTENT_HASH))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_CONTENT_HASH))?;
    let embedding_models = batch
        .column_by_name(COL_EMBEDDING_MODEL)
        .ok_or_else(|| missing_column(COL_EMBEDDING_MODEL))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_EMBEDDING_MODEL))?;
    let embedding_versions = batch
        .column_by_name(COL_EMBEDDING_VERSION)
        .ok_or_else(|| missing_column(COL_EMBEDDING_VERSION))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_EMBEDDING_VERSION))?;
    let namespace_keys = batch
        .column_by_name(COL_NAMESPACE_KEY)
        .ok_or_else(|| missing_column(COL_NAMESPACE_KEY))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| invalid_column(COL_NAMESPACE_KEY))?;
    let dims = batch
        .column_by_name(COL_DIMS)
        .ok_or_else(|| missing_column(COL_DIMS))?
        .as_any()
        .downcast_ref::<UInt32Array>()
        .ok_or_else(|| invalid_column(COL_DIMS))?;
    let created_at_ms = batch
        .column_by_name(COL_CREATED_AT_MS)
        .ok_or_else(|| missing_column(COL_CREATED_AT_MS))?
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| invalid_column(COL_CREATED_AT_MS))?;

    let mut rows = Vec::with_capacity(batch.num_rows());
    for index in 0..batch.num_rows() {
        let embedding_values = embeddings.value(index);
        let embedding_array = embedding_values
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or_else(|| invalid_column(COL_EMBEDDING))?;
        let embedding = embedding_array.values().to_vec();

        rows.push(SearchElementRow {
            element_id: element_ids.value(index).to_string(),
            workspace_id: workspace_ids.value(index).to_string(),
            resource_id: resource_ids.value(index).to_string(),
            resource_version_id: if resource_version_ids.is_null(index) {
                None
            } else {
                Some(resource_version_ids.value(index).to_string())
            },
            element_kind: if element_kinds.is_null(index) {
                DEFAULT_ELEMENT_KIND.to_string()
            } else {
                element_kinds.value(index).to_string()
            },
            ordinal: ordinals.value(index),
            text: texts.value(index).to_string(),
            embedding,
            source_start_byte: source_start_bytes.value(index),
            source_end_byte: source_end_bytes.value(index),
            content_hash: content_hashes.value(index).to_string(),
            embedding_model: embedding_models.value(index).to_string(),
            embedding_version: embedding_versions.value(index).to_string(),
            namespace_key: namespace_keys.value(index).to_string(),
            dims: dims.value(index),
            created_at_ms: created_at_ms.value(index),
        });
    }

    Ok(rows)
}

fn missing_column(name: &str) -> LanceError {
    LanceError::Store {
        message: format!("search-elements batch missing column {name}"),
    }
}

fn invalid_column(name: &str) -> LanceError {
    LanceError::Store {
        message: format!("search-elements column {name} has unexpected type"),
    }
}
