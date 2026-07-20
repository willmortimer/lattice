use std::path::Path;

use calamine::{open_workbook, Data, Reader, Xlsx};

use crate::tabular::{build_tabular_table, enforce_row_limit, tabular_error};
use crate::TabularTable;
use crate::Result;

/// Parse the first worksheet of an `.xlsx` workbook into a tabular table.
pub fn parse_xlsx_file(path: &Path) -> Result<TabularTable> {
    let mut workbook: Xlsx<_> = open_workbook(path).map_err(|source| {
        tabular_error(path, format!("failed to open Excel workbook: {source}"))
    })?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| tabular_error(path, "Excel workbook has no worksheets"))?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|source| tabular_error(path, format!("failed to read worksheet: {source}")))?;

    let mut rows_iter = range.rows();
    let header_row = rows_iter
        .next()
        .ok_or_else(|| tabular_error(path, "Excel worksheet is empty"))?;
    let raw_headers = header_row.iter().map(cell_to_string).collect::<Vec<_>>();

    let mut raw_rows = Vec::new();
    for row in rows_iter {
        if raw_rows.len() >= crate::tabular::TABULAR_IMPORT_MAX_ROWS {
            break;
        }
        raw_rows.push(row.iter().map(cell_to_string).collect());
    }
    enforce_row_limit(path, raw_rows.len())?;

    build_tabular_table(path, &raw_headers, raw_rows)
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        Data::Float(value) => {
            if value.fract().abs() < f64::EPSILON {
                format!("{}", *value as i64)
            } else {
                value.to_string()
            }
        }
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.trim().to_string(),
        Data::DurationIso(value) => value.trim().to_string(),
        Data::Error(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn parses_first_worksheet_with_inferred_types() {
        let path = fixture_path("people.xlsx");
        let parsed = parse_xlsx_file(&path).unwrap();
        assert_eq!(parsed.headers, vec!["name", "active", "count"]);
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.field_types[0], crate::FieldType::Text);
        assert_eq!(parsed.field_types[1], crate::FieldType::Boolean);
        assert_eq!(parsed.field_types[2], crate::FieldType::Integer);
    }
}
