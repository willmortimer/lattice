use std::collections::BTreeSet;
use std::path::Path;

use crate::error::Error;
use crate::types::FieldType;
use crate::Result;

/// Maximum rows accepted from spreadsheet imports (first sheet only).
pub const TABULAR_IMPORT_MAX_ROWS: usize = 100_000;

/// Parsed tabular contents with sanitized SQL column names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabularTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub field_types: Vec<FieldType>,
}

/// Infer column types and drop blank rows after header sanitization.
pub(crate) fn build_tabular_table(
    path: &Path,
    raw_headers: &[String],
    raw_rows: Vec<Vec<String>>,
) -> Result<TabularTable> {
    if raw_headers.is_empty() {
        return Err(Error::invalid_package(path, "tabular import has no header row"));
    }

    let headers = sanitize_headers(raw_headers)?;
    if headers.is_empty() {
        return Err(Error::invalid_package(
            path,
            format!("tabular import has no usable columns after sanitizing headers: {raw_headers:?}"),
        ));
    }

    let mut column_samples = vec![Vec::new(); headers.len()];
    let mut rows = Vec::new();

    for raw_row in raw_rows {
        let mut row = Vec::with_capacity(headers.len());
        for index in 0..headers.len() {
            let cell = raw_row.get(index).map(|value| value.trim()).unwrap_or("");
            if !cell.is_empty() {
                column_samples[index].push(cell.to_string());
            }
            row.push(cell.to_string());
        }
        if row.iter().any(|cell| !cell.is_empty()) {
            rows.push(row);
        }
    }

    let field_types = column_samples
        .iter()
        .map(|samples| infer_field_type(samples))
        .collect();

    Ok(TabularTable {
        headers,
        rows,
        field_types,
    })
}

pub(crate) fn sanitize_headers(raw_headers: &[String]) -> Result<Vec<String>> {
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

/// Map a header label to a valid SQL identifier.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabularFormat {
    Csv,
    Xlsx,
    Json,
    Jsonl,
}

pub fn tabular_format(path: &Path) -> TabularFormat {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("xlsx") => TabularFormat::Xlsx,
        Some("json") => TabularFormat::Json,
        Some("jsonl") | Some("ndjson") => TabularFormat::Jsonl,
        _ => TabularFormat::Csv,
    }
}

pub fn tabular_format_label(format: TabularFormat) -> &'static str {
    match format {
        TabularFormat::Csv => "CSV",
        TabularFormat::Xlsx => "Excel",
        TabularFormat::Json => "JSON",
        TabularFormat::Jsonl => "JSONL",
    }
}

pub(crate) fn tabular_error(path: &Path, message: impl Into<String>) -> Error {
    Error::invalid_package(path, message)
}

pub(crate) fn enforce_row_limit(path: &Path, row_count: usize) -> Result<()> {
    if row_count > TABULAR_IMPORT_MAX_ROWS {
        return Err(tabular_error(
            path,
            format!(
                "tabular import exceeds row limit ({row_count} > {TABULAR_IMPORT_MAX_ROWS})"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn parse_tabular_file_dispatches_by_extension() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("people.json");
        fs::write(
            &json_path,
            r#"[{"name":"Ada","count":1},{"name":"Grace","count":2}]"#,
        )
        .unwrap();
        let parsed = crate::parse_tabular_file(&json_path).unwrap();
        assert_eq!(parsed.headers, vec!["count", "name"]);
        assert_eq!(parsed.rows.len(), 2);
    }
}
