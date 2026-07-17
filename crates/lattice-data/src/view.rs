use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::DATABASE_FILENAME;
use crate::error::Error;
use crate::types::{CellValue, ColumnMeta, Row};
use crate::Result;

pub const VIEW_FORMAT: &str = "lattice-view";
pub const VIEW_VERSION: u32 = 1;
pub const LAYOUT_GRID: &str = "grid";
pub const LAYOUT_LIST: &str = "list";
pub const LAYOUT_BOARD: &str = "board";

const SUPPORTED_LAYOUT_TYPES: &[&str] = &[LAYOUT_GRID, LAYOUT_LIST, LAYOUT_BOARD];

/// Parsed `views/{name}.yaml` grid view definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewDef {
    pub format: String,
    pub version: u32,
    pub source: ViewSource,
    pub layout: ViewLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<ViewSort>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter: Vec<ViewFilter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewSource {
    pub database: String,
    pub table: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewLayout {
    #[serde(rename = "type")]
    pub layout_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    /// Board layout only: column used to group cards into lanes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewSort {
    pub field: String,
    #[serde(default = "default_sort_direction")]
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewFilter {
    pub field: String,
    pub operator: FilterOperator,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterOperator {
    Equals,
    Contains,
}

fn default_sort_direction() -> SortDirection {
    SortDirection::Asc
}

impl ViewDef {
    pub fn new_grid(table: impl Into<String>) -> Self {
        ViewDef {
            format: VIEW_FORMAT.to_string(),
            version: VIEW_VERSION,
            source: ViewSource {
                database: format!("../{DATABASE_FILENAME}"),
                table: table.into(),
            },
            layout: ViewLayout {
                layout_type: LAYOUT_GRID.to_string(),
                columns: Vec::new(),
                group_by: None,
            },
            sort: None,
            filter: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let view: ViewDef = serde_yaml::from_str(&text).map_err(|source| Error::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
        view.check(path)?;
        Ok(view)
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).map_err(|source| Error::Yaml {
            path: PathBuf::from("<view>"),
            source,
        })
    }

    fn check(&self, path: &Path) -> Result<()> {
        let invalid = |message: String| Error::InvalidPackage {
            path: path.to_path_buf(),
            message,
        };
        if self.format != VIEW_FORMAT {
            return Err(invalid(format!(
                "expected view format {VIEW_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version > VIEW_VERSION {
            return Err(invalid(format!(
                "view version {} is newer than supported version {VIEW_VERSION}",
                self.version
            )));
        }
        if !SUPPORTED_LAYOUT_TYPES.contains(&self.layout.layout_type.as_str()) {
            return Err(invalid(format!(
                "unsupported view layout type {:?}; expected one of {:?}",
                self.layout.layout_type, SUPPORTED_LAYOUT_TYPES
            )));
        }
        if let Some(group_by) = &self.layout.group_by {
            validate_identifier(group_by)?;
            if self.layout.layout_type != LAYOUT_BOARD {
                return Err(invalid(
                    "layout.group_by is only supported for board views".to_string(),
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn view_path(package_path: &Path, name: &str) -> PathBuf {
    package_path.join("views").join(format!("{name}.yaml"))
}

pub(crate) fn validate_identifier(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !name.as_bytes()[0].is_ascii_digit();
    if valid {
        Ok(())
    } else {
        Err(Error::table(
            name,
            "invalid SQL identifier; use letters, digits, and underscores",
        ))
    }
}

pub(crate) fn visible_columns<'a>(
    all_columns: &'a [ColumnMeta],
    view: &ViewDef,
) -> Result<Vec<&'a ColumnMeta>> {
    if view.layout.columns.is_empty() {
        return Ok(all_columns.iter().collect());
    }

    let by_name: std::collections::BTreeMap<_, _> = all_columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect();

    let mut visible = Vec::with_capacity(view.layout.columns.len());
    for name in &view.layout.columns {
        validate_identifier(name)?;
        let column = by_name.get(name.as_str()).ok_or_else(|| {
            Error::table(
                view.source.table.clone(),
                format!("view references unknown column {name:?}"),
            )
        })?;
        visible.push(*column);
    }
    Ok(visible)
}

pub(crate) struct ViewQuery {
    pub sql: String,
    pub params: Vec<rusqlite::types::Value>,
}

pub(crate) fn build_view_query(
    table: &str,
    visible: &[&ColumnMeta],
    view: &ViewDef,
    limit: usize,
    offset: usize,
) -> Result<ViewQuery> {
    validate_identifier(table)?;

    let select_list = visible
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let mut clauses = Vec::new();
    let mut params = Vec::new();
    for filter in &view.filter {
        validate_identifier(&filter.field)?;
        match filter.operator {
            FilterOperator::Equals => {
                clauses.push(format!("{} = ?", filter.field));
                params.push(filter_value_as_sqlite(filter)?);
            }
            FilterOperator::Contains => {
                clauses.push(format!("CAST({} AS TEXT) LIKE ?", filter.field));
                params.push(rusqlite::types::Value::Text(format!("%{}%", filter.value)));
            }
        }
    }

    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };

    let sort_field = view
        .sort
        .as_ref()
        .map(|sort| sort.field.as_str())
        .unwrap_or("id");
    validate_identifier(sort_field)?;
    let direction = match view.sort.as_ref().map(|sort| sort.direction) {
        Some(SortDirection::Desc) => "DESC",
        _ => "ASC",
    };

    let sql = format!(
        "SELECT {select_list} FROM {table}{where_sql} ORDER BY {sort_field} {direction} LIMIT ? OFFSET ?"
    );
    params.push(rusqlite::types::Value::Integer(limit as i64));
    params.push(rusqlite::types::Value::Integer(offset as i64));

    Ok(ViewQuery { sql, params })
}

fn filter_value_as_sqlite(filter: &ViewFilter) -> Result<rusqlite::types::Value> {
    if let Ok(integer) = filter.value.parse::<i64>() {
        return Ok(rusqlite::types::Value::Integer(integer));
    }
    Ok(rusqlite::types::Value::Text(filter.value.clone()))
}

pub(crate) fn row_from_view_sql(
    row: &rusqlite::Row<'_>,
    visible: &[&ColumnMeta],
) -> rusqlite::Result<Row> {
    let mut values = std::collections::BTreeMap::new();
    let mut id = String::new();

    for (index, meta) in visible.iter().enumerate() {
        let value = CellValue::from_sqlite(row.get_ref(index)?, meta.field_type)?;
        if meta.name == "id" {
            if let CellValue::Text(text) = &value {
                id = text.clone();
            }
        }
        values.insert(meta.name.clone(), value);
    }

    Ok(Row { id, values })
}
