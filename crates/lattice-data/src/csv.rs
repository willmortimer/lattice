use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::error::Error;
use crate::types::{CellValue, FieldType};
use crate::Result;

/// Parsed CSV contents with sanitized SQL column names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub field_types: Vec<FieldType>,
}

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

    let headers = sanitize_headers(&raw_headers)?;
    let mut column_samples = vec![Vec::new(); headers.len()];
    let mut rows = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|source| csv_error(path, source))?;
        let mut row = Vec::with_capacity(headers.len());
        for (index, _header) in headers.iter().enumerate() {
            let cell = record.get(index).unwrap_or("").trim().to_string();
            if !cell.is_empty() {
                column_samples[index].push(cell.clone());
            }
            row.push(cell);
        }
        if row.iter().any(|cell| !cell.is_empty()) {
            rows.push(row);
        }
    }

    let field_types = column_samples
        .iter()
        .map(|samples| infer_field_type(samples))
        .collect();

    if headers.is_empty() {
        return Err(Error::invalid_package(
            path,
            format!("CSV has no usable columns after sanitizing headers: {raw_headers:?}"),
        ));
    }

    Ok(CsvTable {
        headers,
        rows,
        field_types,
    })
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
        other => Err(Error::table(
            "csv",
            format!("unsupported field type {other:?}; expected text, long_text, integer, decimal, boolean, date, or relation"),
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

/// Infer a Lattice field type from non-empty cell samples.
pub fn infer_field_type(samples: &[String]) -> FieldType {
    if samples.is_empty() {
        return FieldType::Text;
    }

    let mut all_bool = true;
    let mut all_integer = true;
    let mut all_decimal = true;

    for sample in samples {
        let lower = sample.to_ascii_lowercase();
        let is_bool = matches!(lower.as_str(), "true" | "false" | "yes" | "no" | "0" | "1");
        if !is_bool {
            all_bool = false;
        }
        if sample.parse::<i64>().is_err() {
            all_integer = false;
        }
        if sample.parse::<f64>().is_err() {
            all_decimal = false;
        }
    }

    if all_bool {
        FieldType::Boolean
    } else if all_integer {
        FieldType::Integer
    } else if all_decimal {
        FieldType::Decimal
    } else {
        FieldType::Text
    }
}

/// Convert one CSV cell into a typed value for insert.
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
        FieldType::Text | FieldType::LongText => Ok(CellValue::Text(trimmed.to_string())),
        FieldType::Relation => {
            let record_ids: Vec<String> = serde_json::from_str(trimmed).map_err(|_| {
                Error::table(
                    "csv",
                    format!("invalid relation JSON array {trimmed:?}"),
                )
            })?;
            Ok(CellValue::Relation { record_ids })
        }
    }
}

fn sanitize_headers(raw_headers: &[String]) -> Result<Vec<String>> {
    let mut used = BTreeSet::new();
    let mut headers = Vec::with_capacity(raw_headers.len());

    for raw in raw_headers {
        let mut name = sanitize_column_name(raw);
        if name.is_empty() {
            continue;
        }
        let base = name.clone();
        let mut suffix = 2;
        while used.contains(&name) {
            name = format!("{base}_{suffix}");
            suffix += 1;
        }
        used.insert(name.clone());
        headers.push(name);
    }

    Ok(headers)
}

/// Map a CSV header label to a valid SQL identifier.
pub fn sanitize_column_name(header: &str) -> String {
    let mut name = header
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    name = name.trim_matches('_').to_string();
    if name.is_empty() {
        return String::new();
    }
    if name.as_bytes()[0].is_ascii_digit() {
        name = format!("c_{name}");
    }
    name
}

fn csv_error(path: &Path, source: csv::Error) -> Error {
    Error::invalid_package(path, format!("failed to parse CSV: {source}"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
