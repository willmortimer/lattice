use std::collections::BTreeMap;
use std::fmt;

use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};

/// Semantic field types for presentation metadata in `app.yaml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    LongText,
    Integer,
    Decimal,
    Boolean,
    Date,
    /// Multi-record link to another table in the same `.data` package.
    Relation,
    /// Read-only projection of a field through a relation on the same table.
    Lookup,
}

impl FieldType {
    pub fn sqlite_type(self) -> &'static str {
        match self {
            FieldType::Text
            | FieldType::LongText
            | FieldType::Date
            | FieldType::Relation
            | FieldType::Lookup => "TEXT",
            FieldType::Integer | FieldType::Boolean => "INTEGER",
            FieldType::Decimal => "REAL",
        }
    }

    pub fn infer_from_sqlite(declared_type: &str) -> Self {
        let upper = declared_type.to_ascii_uppercase();
        if upper.contains("INT") || upper.contains("BOOL") {
            FieldType::Integer
        } else if upper.contains("REAL") || upper.contains("FLOA") || upper.contains("DOUB") {
            FieldType::Decimal
        } else {
            FieldType::Text
        }
    }

    /// Whether cells of this type are computed at read time and must not be written.
    pub fn is_read_only(self) -> bool {
        matches!(self, FieldType::Lookup)
    }
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldType::Text => write!(f, "text"),
            FieldType::LongText => write!(f, "long_text"),
            FieldType::Integer => write!(f, "integer"),
            FieldType::Decimal => write!(f, "decimal"),
            FieldType::Boolean => write!(f, "boolean"),
            FieldType::Date => write!(f, "date"),
            FieldType::Relation => write!(f, "relation"),
            FieldType::Lookup => write!(f, "lookup"),
        }
    }
}

/// One typed cell value for row CRUD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    Null,
    Text(String),
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
    Date(String),
    /// Linked record ids; SQLite stores a JSON array of strings as TEXT.
    Relation {
        record_ids: Vec<String>,
    },
    /// Resolved lookup display values (never persisted to SQLite).
    Lookup {
        values: Vec<String>,
    },
}

impl CellValue {
    pub fn as_sqlite_value(&self) -> rusqlite::types::Value {
        match self {
            CellValue::Null | CellValue::Lookup { .. } => rusqlite::types::Value::Null,
            CellValue::Text(text) | CellValue::Date(text) => {
                rusqlite::types::Value::Text(text.clone())
            }
            CellValue::Integer(value) => rusqlite::types::Value::Integer(*value),
            CellValue::Decimal(value) => rusqlite::types::Value::Real(*value),
            CellValue::Boolean(value) => rusqlite::types::Value::Integer(i64::from(*value)),
            CellValue::Relation { record_ids } => {
                let encoded = serde_json::to_string(record_ids)
                    .expect("relation record_ids serialize to JSON");
                rusqlite::types::Value::Text(encoded)
            }
        }
    }

    pub fn from_sqlite(value_ref: ValueRef<'_>, field_type: FieldType) -> rusqlite::Result<Self> {
        // Lookup columns are placeholders; resolved values are filled after the SQL read.
        if field_type == FieldType::Lookup {
            return Ok(CellValue::Lookup { values: Vec::new() });
        }
        match value_ref {
            ValueRef::Null => Ok(CellValue::Null),
            ValueRef::Integer(value) => match field_type {
                FieldType::Boolean => Ok(CellValue::Boolean(value != 0)),
                FieldType::Date => Ok(CellValue::Date(value.to_string())),
                FieldType::Relation | FieldType::Lookup => Err(rusqlite::Error::InvalidColumnType(
                    0,
                    field_type.to_string(),
                    value_ref.data_type(),
                )),
                _ => Ok(CellValue::Integer(value)),
            },
            ValueRef::Real(value) => match field_type {
                FieldType::Relation | FieldType::Lookup => Err(rusqlite::Error::InvalidColumnType(
                    0,
                    field_type.to_string(),
                    value_ref.data_type(),
                )),
                _ => Ok(CellValue::Decimal(value)),
            },
            ValueRef::Text(bytes) => {
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| {
                        rusqlite::Error::InvalidColumnType(0, "".into(), value_ref.data_type())
                    })?
                    .to_string();
                match field_type {
                    FieldType::Date => Ok(CellValue::Date(text)),
                    FieldType::Boolean => Ok(CellValue::Boolean(matches!(
                        text.to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes"
                    ))),
                    FieldType::Decimal => {
                        text.parse::<f64>().map(CellValue::Decimal).map_err(|_| {
                            rusqlite::Error::InvalidColumnType(0, text, rusqlite::types::Type::Real)
                        })
                    }
                    FieldType::Relation => {
                        let record_ids: Vec<String> =
                            serde_json::from_str(&text).map_err(|_| {
                                rusqlite::Error::InvalidColumnType(
                                    0,
                                    text,
                                    rusqlite::types::Type::Text,
                                )
                            })?;
                        Ok(CellValue::Relation { record_ids })
                    }
                    FieldType::Lookup => Ok(CellValue::Lookup { values: Vec::new() }),
                    _ => Ok(CellValue::Text(text)),
                }
            }
            ValueRef::Blob(_) => Err(rusqlite::Error::InvalidColumnType(
                0,
                "blob".into(),
                rusqlite::types::Type::Blob,
            )),
        }
    }

    /// Human-readable display for a cell (used when resolving lookups).
    pub fn display_text(&self) -> String {
        match self {
            CellValue::Null => String::new(),
            CellValue::Text(text) | CellValue::Date(text) => text.clone(),
            CellValue::Integer(value) => value.to_string(),
            CellValue::Decimal(value) => value.to_string(),
            CellValue::Boolean(value) => {
                if *value {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            CellValue::Relation { record_ids } => record_ids.join(", "),
            CellValue::Lookup { values } => values.join(", "),
        }
    }
}

/// Column metadata merged from SQLite schema and `app.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub field_type: FieldType,
    pub sqlite_type: String,
    /// Target table name for [`FieldType::Relation`] within the same package.
    pub relation_table: Option<String>,
    /// Source relation column on this table for [`FieldType::Lookup`].
    pub lookup_relation: Option<String>,
    /// Field on the related table projected by [`FieldType::Lookup`].
    pub lookup_field: Option<String>,
}

/// Prior relation cell state stripped when a target row is deleted.
///
/// Captured so command undo can restore inbound links after
/// [`crate::DataApp::restore_row`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationStrip {
    pub table: String,
    pub row_id: String,
    pub column: String,
    pub prior_record_ids: Vec<String>,
}

/// Bytes stored as `prior_content` for [`RecordDelete`] undo: the deleted row
/// plus any inbound relation cells that dropped its id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletedRowSnapshot {
    pub row: Row,
    #[serde(default)]
    pub relation_strips: Vec<RelationStrip>,
}

/// Prior `schema.sql` + `app.yaml` bytes for undoing schema mutations
/// (`ColumnsAdd` / `TableAdd`) through the command engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaFilesSnapshot {
    pub schema_sql: String,
    pub app_yaml: String,
    /// Columns actually added by [`Command::ColumnsAdd`] (for undo guards).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_columns: Vec<String>,
    /// Table actually added by [`Command::TableAdd`] (for undo guards).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_table: Option<String>,
}

/// Spec for adding a column via [`crate::DataApp::add_columns`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewColumn<'a> {
    pub name: &'a str,
    pub field_type: FieldType,
    pub relation_table: Option<&'a str>,
    pub lookup_relation: Option<&'a str>,
    pub lookup_field: Option<&'a str>,
}

impl<'a> NewColumn<'a> {
    pub fn new(name: &'a str, field_type: FieldType) -> Self {
        Self {
            name,
            field_type,
            relation_table: None,
            lookup_relation: None,
            lookup_field: None,
        }
    }

    pub fn relation(name: &'a str, relation_table: &'a str) -> Self {
        Self {
            name,
            field_type: FieldType::Relation,
            relation_table: Some(relation_table),
            lookup_relation: None,
            lookup_field: None,
        }
    }

    pub fn lookup(name: &'a str, lookup_relation: &'a str, lookup_field: &'a str) -> Self {
        Self {
            name,
            field_type: FieldType::Lookup,
            relation_table: None,
            lookup_relation: Some(lookup_relation),
            lookup_field: Some(lookup_field),
        }
    }
}

/// One row from a data-app table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub id: String,
    pub values: BTreeMap<String, CellValue>,
}
