use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::DATABASE_FILENAME;
use crate::error::Error;
use crate::types::{CellValue, ColumnMeta, FieldType, RollupAggregate, Row};
use crate::Result;

pub const VIEW_FORMAT: &str = "lattice-view";
pub const VIEW_VERSION: u32 = 1;
pub const LAYOUT_GRID: &str = "grid";
pub const LAYOUT_LIST: &str = "list";
pub const LAYOUT_BOARD: &str = "board";
pub const LAYOUT_GALLERY: &str = "gallery";
pub const LAYOUT_CALENDAR: &str = "calendar";
pub const LAYOUT_FORM: &str = "form";

pub const SUPPORTED_LAYOUT_TYPES: &[&str] = &[
    LAYOUT_GRID,
    LAYOUT_LIST,
    LAYOUT_BOARD,
    LAYOUT_GALLERY,
    LAYOUT_CALENDAR,
    LAYOUT_FORM,
];

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
    /// View-scoped cell conditional formatting (grid). First match per field wins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditional_format: Vec<ConditionalFormatRule>,
}

/// Style applied when a conditional format rule matches.
///
/// Token names are Lattice semantic roles without the `--lt-` prefix
/// (for example `accent-wash`, `danger`, `text`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConditionalFormatStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// One cell conditional-format rule: match `field` via `operator`/`value`, then apply `style`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalFormatRule {
    pub field: String,
    pub operator: FilterOperator,
    pub value: String,
    pub style: ConditionalFormatStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewSource {
    pub database: String,
    pub table: String,
}

/// One footer or per-group summary aggregate for grid views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewLayoutSummary {
    pub field: String,
    pub aggregate: RollupAggregate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewLayout {
    #[serde(rename = "type")]
    pub layout_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    /// Board layout: lane column. Grid layout: optional row grouping with per-group footers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    /// Grid layout only: per-field footer / group summary aggregates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summaries: Vec<ViewLayoutSummary>,
    /// Gallery layout only: column rendered as each card's cover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_field: Option<String>,
    /// Calendar layout only: column used to place records on the calendar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_field: Option<String>,
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
                summaries: Vec::new(),
                cover_field: None,
                date_field: None,
            },
            sort: None,
            filter: Vec::new(),
            conditional_format: Vec::new(),
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

    /// Validate format, layout fields, and conditional-format rules.
    pub fn validate(&self) -> Result<()> {
        self.check(Path::new("<view>"))
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
            if self.layout.layout_type != LAYOUT_BOARD && self.layout.layout_type != LAYOUT_GRID {
                return Err(invalid(
                    "layout.group_by is only supported for board and grid views".to_string(),
                ));
            }
        }
        if !self.layout.summaries.is_empty() {
            if self.layout.layout_type != LAYOUT_GRID {
                return Err(invalid(
                    "layout.summaries is only supported for grid views".to_string(),
                ));
            }
            for summary in &self.layout.summaries {
                validate_identifier(&summary.field)?;
            }
        }
        if let Some(cover_field) = &self.layout.cover_field {
            validate_identifier(cover_field)?;
            if self.layout.layout_type != LAYOUT_GALLERY {
                return Err(invalid(
                    "layout.cover_field is only supported for gallery views".to_string(),
                ));
            }
        }
        if let Some(date_field) = &self.layout.date_field {
            validate_identifier(date_field)?;
            if self.layout.layout_type != LAYOUT_CALENDAR {
                return Err(invalid(
                    "layout.date_field is only supported for calendar views".to_string(),
                ));
            }
        }
        for rule in &self.conditional_format {
            validate_identifier(&rule.field)?;
            validate_style_token(path, rule.style.bg.as_deref(), "conditional_format.style.bg")?;
            validate_style_token(path, rule.style.text.as_deref(), "conditional_format.style.text")?;
            if rule.style.bg.is_none() && rule.style.text.is_none() {
                return Err(invalid(
                    "conditional_format rule requires at least one of style.bg or style.text"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl ConditionalFormatRule {
    /// Whether `display` (cell display string) matches this rule.
    pub fn matches_display(&self, display: &str) -> bool {
        match self.operator {
            FilterOperator::Equals => display.eq_ignore_ascii_case(&self.value),
            FilterOperator::Contains => display
                .to_ascii_lowercase()
                .contains(&self.value.to_ascii_lowercase()),
        }
    }
}

fn validate_style_token(path: &Path, token: Option<&str>, label: &str) -> Result<()> {
    let Some(token) = token else {
        return Ok(());
    };
    let valid = !token.is_empty()
        && token
            .chars()
            .enumerate()
            .all(|(index, ch)| match (index, ch) {
                (0, ch) => ch.is_ascii_lowercase(),
                (_, ch) => ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-',
            });
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidPackage {
            path: path.to_path_buf(),
            message: format!(
                "{label} must be a non-empty semantic token name (a-z, digits, hyphens); got {token:?}"
            ),
        })
    }
}

/// Compute one summary aggregate over `rows` for `field` using `field_type` hints.
pub fn compute_layout_summary(
    rows: &[Row],
    field: &str,
    field_type: FieldType,
    aggregate: RollupAggregate,
) -> Option<f64> {
    match aggregate {
        RollupAggregate::Count => Some(
            rows.iter()
                .filter(|row| !cell_value_is_empty(row.values.get(field)))
                .count() as f64,
        ),
        RollupAggregate::Sum | RollupAggregate::Min | RollupAggregate::Max => {
            let numbers: Vec<f64> = rows
                .iter()
                .filter_map(|row| numeric_cell_value(row.values.get(field), field_type))
                .collect();
            if numbers.is_empty() {
                return None;
            }
            match aggregate {
                RollupAggregate::Sum => Some(numbers.iter().sum()),
                RollupAggregate::Min => numbers.into_iter().reduce(f64::min),
                RollupAggregate::Max => numbers.into_iter().reduce(f64::max),
                RollupAggregate::Count => unreachable!(),
            }
        }
    }
}

fn cell_value_is_empty(value: Option<&CellValue>) -> bool {
    match value {
        None => true,
        Some(CellValue::Null) => true,
        Some(CellValue::Text(text)) => text.is_empty(),
        Some(CellValue::Date(text)) => text.is_empty(),
        Some(CellValue::Relation { record_ids }) => record_ids.is_empty(),
        Some(CellValue::MultiEnum { values }) => values.is_empty(),
        Some(CellValue::Lookup { values }) => values.is_empty(),
        Some(CellValue::Rollup { value }) => value.is_none(),
        Some(CellValue::Formula { value }) => value.is_none(),
        Some(CellValue::Integer(_))
        | Some(CellValue::Decimal(_))
        | Some(CellValue::Boolean(_)) => false,
    }
}

fn numeric_cell_value(value: Option<&CellValue>, field_type: FieldType) -> Option<f64> {
    match value {
        Some(CellValue::Integer(value)) => Some(*value as f64),
        Some(CellValue::Decimal(value)) => Some(*value),
        Some(CellValue::Rollup { value }) => *value,
        Some(CellValue::Formula { value }) => match value {
            Some(crate::types::FormulaValue::Number(number)) => Some(*number),
            _ => None,
        },
        _ if matches!(field_type, FieldType::Integer | FieldType::Decimal) => None,
        _ => None,
    }
}

pub(crate) fn view_path(package_path: &Path, name: &str) -> PathBuf {
    package_path.join("views").join(format!("{name}.yaml"))
}

/// Write `views/{name}.yaml` inside a `.data` package.
pub fn write_package_view(package_path: &Path, name: &str, view: &ViewDef) -> Result<()> {
    validate_identifier(name)?;
    let path = view_path(package_path, name);
    view.check(&path)?;
    let views_dir = package_path.join("views");
    std::fs::create_dir_all(&views_dir).map_err(|source| Error::io(&views_dir, source))?;
    let contents = view.to_yaml()?;
    std::fs::write(&path, contents).map_err(|source| Error::io(&path, source))?;
    Ok(())
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

fn view_filter_clause(view: &ViewDef) -> Result<(String, Vec<rusqlite::types::Value>)> {
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
    Ok((where_sql, params))
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

    let (where_sql, mut params) = view_filter_clause(view)?;

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

/// Count rows matching a view's filters (ignores sort/limit/offset).
pub(crate) fn build_view_count_query(table: &str, view: &ViewDef) -> Result<ViewQuery> {
    validate_identifier(table)?;
    let (where_sql, params) = view_filter_clause(view)?;
    let sql = format!("SELECT COUNT(*) FROM {table}{where_sql}");
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
