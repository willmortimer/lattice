//! Arrow-ready columnar batches (schema + typed columns).
//!
//! P3-03 maps these into Arrow IPC without changing the public query surface.

/// Logical column type aligned with Arrow primitives Lattice will transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    Null,
    Boolean,
    Int64,
    Float64,
    Utf8,
    /// DuckDB type name preserved until a dedicated Arrow mapping lands.
    Other(String),
}

impl DataType {
    pub(crate) fn from_duckdb(type_name: &str) -> Self {
        let upper = type_name.to_ascii_uppercase();
        match upper.as_str() {
            "NULL" | "NULLABLE" => DataType::Null,
            "BOOLEAN" | "BOOL" => DataType::Boolean,
            "TINYINT" | "SMALLINT" | "INTEGER" | "INT" | "BIGINT" | "HUGEINT" | "UTINYINT"
            | "USMALLINT" | "UINTEGER" | "UBIGINT" | "INT8" | "INT16" | "INT32" | "INT64"
            | "UINT8" | "UINT16" | "UINT32" | "UINT64" => DataType::Int64,
            "FLOAT" | "DOUBLE" | "DECIMAL" | "NUMERIC" | "REAL" | "FLOAT32" | "FLOAT64" => {
                DataType::Float64
            }
            "VARCHAR"
            | "TEXT"
            | "STRING"
            | "BLOB"
            | "UUID"
            | "DATE"
            | "TIME"
            | "TIMESTAMP"
            | "TIMESTAMP WITH TIME ZONE"
            | "TIMESTAMPTZ"
            | "INTERVAL"
            | "UTF8"
            | "LARGEUTF8"
            | "BINARY"
            | "LARGEBINARY" => DataType::Utf8,
            other => DataType::Other(other.to_string()),
        }
    }
}

/// Single nullable scalar, ready to copy into an Arrow array builder.
#[derive(Debug, Clone, PartialEq)]
pub enum ScalarValue {
    Null,
    Boolean(bool),
    Int64(i64),
    Float64(f64),
    Utf8(String),
}

/// Named field in a batch schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

/// Columnar schema for a [`RecordBatch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub fields: Vec<Field>,
}

impl Schema {
    pub fn empty() -> Self {
        Self { fields: Vec::new() }
    }
}

/// One columnar query result (Arrow RecordBatch-shaped).
#[derive(Debug, Clone, PartialEq)]
pub struct RecordBatch {
    pub schema: Schema,
    /// Column-major values; `columns[i].len() == num_rows` for every column.
    pub columns: Vec<Vec<ScalarValue>>,
    pub num_rows: usize,
}

impl RecordBatch {
    pub fn empty() -> Self {
        Self {
            schema: Schema::empty(),
            columns: Vec::new(),
            num_rows: 0,
        }
    }

    /// Row-major view for CLI / tests (copies references into owned rows).
    pub fn rows(&self) -> Vec<Vec<ScalarValue>> {
        let mut rows = Vec::with_capacity(self.num_rows);
        for row_idx in 0..self.num_rows {
            let mut row = Vec::with_capacity(self.columns.len());
            for column in &self.columns {
                row.push(column[row_idx].clone());
            }
            rows.push(row);
        }
        rows
    }
}
