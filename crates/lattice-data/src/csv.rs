use std::collections::BTreeMap;
use std::path::Path;

use crate::error::Error;
use crate::tabular::build_tabular_table;
use crate::types::{CellValue, FieldType};
use crate::{Result, TabularTable};

/// Parsed CSV contents with sanitized SQL column names.
pub type CsvTable = TabularTable;

/// Read a CSV file and infer column types from cell samples.
pub fn parse_csv_file(path: &Path) -> Result<CsvTable> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .map_err(|source| Error::io(path, std::io::Error::other(source)))?;

    let raw_headers = reader
        .headers()
        .map_err(|source| csv_error(path, source))?
        .iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    if raw_headers.is_empty() {
        return Err(Error::invalid_package(path, "CSV file has no header row"));
    }

    let mut raw_rows = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|source| csv_error(path, source))?;
        raw_rows.push(
            record
                .iter()
                .map(|cell| cell.trim().to_string())
                .collect::<Vec<_>>(),
        );
    }

    build_tabular_table(path, &raw_headers, raw_rows)
}

/// Parse a snake_case field type name (`text`, `integer`, …).
pub fn parse_field_type_name(value: &str) -> Result<FieldType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "text" => Ok(FieldType::Text),
        "long_text" => Ok(FieldType::LongText),
        "integer" => Ok(FieldType::Integer),
        "decimal" => Ok(FieldType::Decimal),
        "boolean" => Ok(FieldType::Boolean),
        "date" => Ok(FieldType::Date),
        "relation" => Ok(FieldType::Relation),
        "lookup" => Ok(FieldType::Lookup),
        "rollup" => Ok(FieldType::Rollup),
        "formula" => Ok(FieldType::Formula),
        "enum" => Ok(FieldType::Enum),
        "multi_enum" => Ok(FieldType::MultiEnum),
        other => Err(Error::table(
            "csv",
            format!("unsupported field type {other:?}; expected text, long_text, integer, decimal, boolean, date, relation, lookup, rollup, formula, enum, or multi_enum"),
        )),
    }
}

/// Apply per-column type overrides, keeping inferred types for unspecified columns.
pub fn resolve_field_types(
    headers: &[String],
    inferred: &[FieldType],
    overrides: &BTreeMap<String, FieldType>,
) -> Vec<FieldType> {
    headers
        .iter()
        .zip(inferred)
        .map(|(header, default)| overrides.get(header).copied().unwrap_or(*default))
        .collect()
}

/// Convert one imported cell string into a typed value for insert.
pub fn cell_from_csv(text: &str, field_type: FieldType) -> Result<CellValue> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(CellValue::Null);
    }

    match field_type {
        FieldType::Boolean => {
            let lower = trimmed.to_ascii_lowercase();
            Ok(CellValue::Boolean(matches!(
                lower.as_str(),
                "1" | "true" | "yes"
            )))
        }
        FieldType::Integer => trimmed
            .parse::<i64>()
            .map(CellValue::Integer)
            .map_err(|_| Error::table("csv", format!("invalid integer value {trimmed:?}"))),
        FieldType::Decimal => trimmed
            .parse::<f64>()
            .map(CellValue::Decimal)
            .map_err(|_| Error::table("csv", format!("invalid decimal value {trimmed:?}"))),
        FieldType::Date => Ok(CellValue::Date(trimmed.to_string())),
        FieldType::Text | FieldType::LongText | FieldType::Enum => {
            Ok(CellValue::Text(trimmed.to_string()))
        }
        FieldType::Relation => {
            let record_ids: Vec<String> = serde_json::from_str(trimmed).map_err(|_| {
                Error::table(
                    "csv",
                    format!("invalid relation JSON array {trimmed:?}"),
                )
            })?;
            Ok(CellValue::Relation { record_ids })
        }
        FieldType::MultiEnum => {
            let values: Vec<String> = if trimmed.starts_with('[') {
                serde_json::from_str(trimmed).map_err(|_| {
                    Error::table(
                        "csv",
                        format!("invalid multi_enum JSON array {trimmed:?}"),
                    )
                })?
            } else {
                trimmed
                    .split(',')
                    .map(|part| part.trim().to_string())
                    .filter(|part| !part.is_empty())
                    .collect()
            };
            Ok(CellValue::MultiEnum { values })
        }
        FieldType::Lookup => Err(Error::table(
            "csv",
            "lookup columns are read-only and cannot be imported from CSV",
        )),
        FieldType::Rollup => Err(Error::table(
            "csv",
            "rollup columns are read-only and cannot be imported from CSV",
        )),
        FieldType::Formula => Err(Error::table(
            "csv",
            "formula columns are read-only and cannot be imported from CSV",
        )),
    }
}

fn csv_error(path: &Path, source: csv::Error) -> Error {
    Error::invalid_package(path, format!("failed to parse CSV: {source}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular::{infer_field_type, sanitize_column_name, sanitize_headers};

    #[test]
    fn infers_boolean_integer_decimal_and_text() {
        assert_eq!(
            infer_field_type(&["true".into(), "false".into(), "yes".into()]),
            FieldType::Boolean
        );
        assert_eq!(
            infer_field_type(&["1".into(), "2".into(), "0".into()]),
            FieldType::Integer
        );
        assert_eq!(
            infer_field_type(&["1.5".into(), "2.0".into()]),
            FieldType::Decimal
        );
        assert_eq!(
            infer_field_type(&["Ada".into(), "Grace".into()]),
            FieldType::Text
        );
    }

    #[test]
    fn sanitizes_headers_and_avoids_duplicates() {
        let headers =
            sanitize_headers(&["Name".into(), "name".into(), "2024".into(), "".into()]).unwrap();
        assert_eq!(headers, vec!["name", "name_2", "c_2024"]);
    }

    #[test]
    fn sanitize_column_name_maps_digits() {
        assert_eq!(sanitize_column_name("2024"), "c_2024");
    }

    #[test]
    fn parse_field_type_name_accepts_snake_case_labels() {
        assert_eq!(parse_field_type_name("text").unwrap(), FieldType::Text);
        assert_eq!(parse_field_type_name("INTEGER").unwrap(), FieldType::Integer);
        assert!(parse_field_type_name("unknown").is_err());
    }

    #[test]
    fn resolve_field_types_applies_overrides() {
        let headers = vec!["name".into(), "count".into()];
        let inferred = vec![FieldType::Text, FieldType::Integer];
        let overrides = BTreeMap::from([("count".into(), FieldType::Decimal)]);
        assert_eq!(
            resolve_field_types(&headers, &inferred, &overrides),
            vec![FieldType::Text, FieldType::Decimal]
        );
    }
}
