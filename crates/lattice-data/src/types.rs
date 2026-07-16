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
}

impl FieldType {
    pub fn sqlite_type(self) -> &'static str {
        match self {
            FieldType::Text | FieldType::LongText | FieldType::Date => "TEXT",
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
        }
    }
}

/// One typed cell value for row CRUD.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Null,
    Text(String),
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
    Date(String),
}

impl CellValue {
    pub fn as_sqlite_value(&self) -> rusqlite::types::Value {
        match self {
            CellValue::Null => rusqlite::types::Value::Null,
            CellValue::Text(text) | CellValue::Date(text) => {
                rusqlite::types::Value::Text(text.clone())
            }
            CellValue::Integer(value) => rusqlite::types::Value::Integer(*value),
            CellValue::Decimal(value) => rusqlite::types::Value::Real(*value),
            CellValue::Boolean(value) => rusqlite::types::Value::Integer(i64::from(*value)),
        }
    }

    pub fn from_sqlite(value_ref: ValueRef<'_>, field_type: FieldType) -> rusqlite::Result<Self> {
        match value_ref {
            ValueRef::Null => Ok(CellValue::Null),
            ValueRef::Integer(value) => match field_type {
                FieldType::Boolean => Ok(CellValue::Boolean(value != 0)),
                FieldType::Date => Ok(CellValue::Date(value.to_string())),
                _ => Ok(CellValue::Integer(value)),
            },
            ValueRef::Real(value) => Ok(CellValue::Decimal(value)),
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
}

/// Column metadata merged from SQLite schema and `app.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub field_type: FieldType,
    pub sqlite_type: String,
}

/// One row from a data-app table.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: String,
    pub values: BTreeMap<String, CellValue>,
}
