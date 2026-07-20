use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::tabular::{build_tabular_table, enforce_row_limit, tabular_error};
use crate::TabularTable;
use crate::Result;

/// Parse a JSON file containing an array of objects into a tabular table.
pub fn parse_json_file(path: &Path) -> Result<TabularTable> {
    let contents = fs::read_to_string(path).map_err(|source| crate::Error::io(path, source))?;
    let value: Value = serde_json::from_str(&contents)
        .map_err(|source| tabular_error(path, format!("failed to parse JSON: {source}")))?;
    let objects = array_of_objects(path, &value)?;
    objects_to_table(path, &objects)
}

/// Parse a JSON Lines file (one object per line) into a tabular table.
pub fn parse_jsonl_file(path: &Path) -> Result<TabularTable> {
    let contents = fs::read_to_string(path).map_err(|source| crate::Error::io(path, source))?;
    let mut objects = Vec::new();
    for (line_number, line) in contents.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed).map_err(|source| {
            tabular_error(
                path,
                format!("failed to parse JSONL line {}: {source}", line_number + 1),
            )
        })?;
        let object = value
            .as_object()
            .ok_or_else(|| {
                tabular_error(
                    path,
                    format!("JSONL line {} must be a JSON object", line_number + 1),
                )
            })?
            .clone();
        objects.push(object);
        if objects.len() > crate::tabular::TABULAR_IMPORT_MAX_ROWS {
            break;
        }
    }
    enforce_row_limit(path, objects.len())?;
    objects_to_table(path, &objects)
}

fn array_of_objects(path: &Path, value: &Value) -> Result<Vec<serde_json::Map<String, Value>>> {
    let array = value.as_array().ok_or_else(|| {
        tabular_error(path, "JSON import must be an array of objects at the top level")
    })?;
    if array.is_empty() {
        return Err(tabular_error(path, "JSON import array is empty"));
    }
    let mut objects = Vec::with_capacity(array.len());
    for (index, item) in array.iter().enumerate() {
        let object = item.as_object().ok_or_else(|| {
            tabular_error(path, format!("JSON array item {index} must be an object"))
        })?;
        objects.push(object.clone());
        if objects.len() > crate::tabular::TABULAR_IMPORT_MAX_ROWS {
            break;
        }
    }
    enforce_row_limit(path, objects.len())?;
    Ok(objects)
}

fn objects_to_table(path: &Path, objects: &[serde_json::Map<String, Value>]) -> Result<TabularTable> {
    let raw_headers = collect_headers(objects);
    let raw_rows = objects
        .iter()
        .map(|object| {
            raw_headers
                .iter()
                .map(|header| json_value_to_string(object.get(header)))
                .collect()
        })
        .collect();
    build_tabular_table(path, &raw_headers, raw_rows)
}

fn collect_headers(objects: &[serde_json::Map<String, Value>]) -> Vec<String> {
    let mut headers = Vec::new();
    let mut seen = BTreeSet::new();
    for object in objects {
        for key in object.keys() {
            if seen.insert(key.clone()) {
                headers.push(key.clone());
            }
        }
    }
    headers
}

fn json_value_to_string(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::String(value)) => value.trim().to_string(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FieldType;

    #[test]
    fn parses_json_array_of_objects() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("people.json");
        fs::write(
            &path,
            r#"[
  {"name":"Ada","active":true,"count":1},
  {"name":"Grace","active":false,"count":2}
]"#,
        )
        .unwrap();

        let parsed = parse_json_file(&path).unwrap();
        assert_eq!(parsed.headers, vec!["active", "count", "name"]);
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.field_types[0], FieldType::Boolean);
        assert_eq!(parsed.field_types[1], FieldType::Integer);
    }

    #[test]
    fn parses_jsonl_objects() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("people.jsonl");
        fs::write(
            &path,
            "{\"label\":\"Widget\",\"amount\":10}\n{\"label\":\"Gadget\",\"amount\":20}\n",
        )
        .unwrap();

        let parsed = parse_jsonl_file(&path).unwrap();
        assert_eq!(parsed.headers, vec!["amount", "label"]);
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.field_types[0], FieldType::Integer);
    }

    #[test]
    fn rejects_non_object_json_array_items() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "[1,2,3]").unwrap();
        let err = parse_json_file(&path).unwrap_err().to_string();
        assert!(err.contains("must be an object"), "{err}");
    }
}
