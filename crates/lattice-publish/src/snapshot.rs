//! Freeze BindingSpec results into JSON snapshots for offline HTML.

use std::path::{Path, PathBuf};

use lattice_data::{BindingSpec, DataApp, InterfaceComponent, InterfaceDef};
use serde::Serialize;
use serde_json::json;

use crate::error::{Error, Result};
use crate::markdown::escape_html;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSnapshot {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentSnapshot {
    pub id: String,
    pub component_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub span: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<TableSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceSnapshot {
    pub format: &'static str,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub columns: u32,
    pub components: Vec<ComponentSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactBindingSnapshot {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<TableSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSnapshot {
    pub format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub entrypoint: String,
    pub bindings: Vec<ArtifactBindingSnapshot>,
}

pub fn empty_table(note: impl Into<String>) -> TableSnapshot {
    TableSnapshot {
        columns: Vec::new(),
        rows: Vec::new(),
        note: Some(note.into()),
    }
}

fn resolve_package_path(
    workspace_root: &Path,
    resource: &str,
    interface_package: &Path,
) -> PathBuf {
    if resource == "." {
        return interface_package.to_path_buf();
    }
    workspace_root.join(resource)
}

/// Freeze interface component bindings into a read-only snapshot.
pub fn freeze_interface(
    workspace_root: &Path,
    package_path: &Path,
    interface: &InterfaceDef,
) -> Result<InterfaceSnapshot> {
    let mut components = Vec::new();
    for component in &interface.components {
        components.push(freeze_component(workspace_root, package_path, component)?);
    }
    // Legacy view/form-only interfaces still export a readable shell.
    if components.is_empty() {
        for view in &interface.views {
            components.push(ComponentSnapshot {
                id: format!("view_{view}"),
                component_type: "data-view".into(),
                title: Some(view.clone()),
                span: 12,
                metric: None,
                table: Some(snapshot_saved_view(
                    workspace_root,
                    package_path,
                    ".",
                    view,
                )?),
                note: None,
            });
        }
        for form in &interface.forms {
            components.push(ComponentSnapshot {
                id: format!("form_{form}"),
                component_type: "form".into(),
                title: Some(form.clone()),
                span: 12,
                metric: None,
                table: None,
                note: Some("Form surfaces are not interactive in static exports.".into()),
            });
        }
    }

    Ok(InterfaceSnapshot {
        format: "lattice-publish-interface-snapshot",
        name: interface.name.clone(),
        title: interface.title.clone(),
        description: interface.description.clone(),
        columns: interface.layout_columns(),
        components,
    })
}

fn freeze_component(
    workspace_root: &Path,
    package_path: &Path,
    component: &InterfaceComponent,
) -> Result<ComponentSnapshot> {
    let component_type = match component.component_type {
        lattice_data::InterfaceComponentType::Metric => "metric",
        lattice_data::InterfaceComponentType::Chart => "chart",
        lattice_data::InterfaceComponentType::Map => "map",
        lattice_data::InterfaceComponentType::Form => "form",
        lattice_data::InterfaceComponentType::DataView => "data-view",
    };

    let mut metric = None;
    let mut table = None;
    let mut note = None;

    match &component.binding {
        Some(binding) => match binding {
            BindingSpec::SqliteQuery {
                resource,
                sql,
                limit,
            } => match snapshot_sqlite(workspace_root, package_path, resource, sql, *limit) {
                Ok((columns, rows)) => {
                    if component_type == "metric" {
                        metric = rows.first().and_then(|row| row.first()).cloned();
                        if metric.is_none() {
                            note = Some("Query returned no rows.".into());
                        }
                    } else {
                        table = Some(TableSnapshot {
                            columns,
                            rows,
                            note: None,
                        });
                    }
                }
                Err(err) => {
                    table = Some(empty_table(format!("Could not freeze sqlite-query: {err}")));
                    note = Some(format!("Could not freeze sqlite-query: {err}"));
                }
            },
            BindingSpec::SavedView { resource, view } => {
                table = Some(snapshot_saved_view(
                    workspace_root,
                    package_path,
                    resource,
                    view,
                )?);
            }
            BindingSpec::DuckdbQuery { .. } => {
                table = Some(empty_table(
                    "DuckDB query results are not frozen in this export; empty placeholder table.",
                ));
                note = Some("DuckDB snapshot placeholder.".into());
            }
            BindingSpec::Resource { resource } => {
                note = Some(format!(
                    "Resource binding `{resource}` (no tabular freeze)."
                ));
            }
            BindingSpec::NotebookOutput { resource, cell_id } => {
                note = Some(format!(
                    "Notebook output `{resource}` / `{cell_id}` is not frozen in this export."
                ));
            }
            BindingSpec::TaskOutput { resource, name } => {
                note = Some(format!(
                    "Task output `{resource}` / `{name}` is not frozen in this export."
                ));
            }
        },
        None => {
            if component_type == "form" {
                note = Some("Form surfaces are not interactive in static exports.".into());
            } else {
                note = Some("Component has no binding.".into());
            }
        }
    }

    Ok(ComponentSnapshot {
        id: component.id.clone(),
        component_type: component_type.into(),
        title: component.title.clone(),
        span: component.span,
        metric,
        table,
        note,
    })
}

fn snapshot_sqlite(
    workspace_root: &Path,
    package_path: &Path,
    resource: &str,
    sql: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<Vec<serde_json::Value>>)> {
    let path = resolve_package_path(workspace_root, resource, package_path);
    let app = DataApp::open(&path)?;
    Ok(app.query_sql_table(sql, limit)?)
}

fn snapshot_saved_view(
    workspace_root: &Path,
    package_path: &Path,
    resource: &str,
    view: &str,
) -> Result<TableSnapshot> {
    let path = resolve_package_path(workspace_root, resource, package_path);
    match DataApp::open(&path) {
        Ok(app) => match app.load_view(view) {
            Ok(view_def) => {
                let table = view_def.source.table.clone();
                match app.list_rows_with_view(&table, &view_def, 100, 0) {
                    Ok((columns_meta, rows)) => {
                        let columns = columns_meta.into_iter().map(|c| c.name).collect::<Vec<_>>();
                        let mut json_rows = Vec::new();
                        for row in rows {
                            let mut values = Vec::with_capacity(columns.len());
                            for col in &columns {
                                if col == "id" {
                                    values.push(json!(row.id));
                                } else {
                                    values.push(cell_to_json(row.values.get(col)));
                                }
                            }
                            json_rows.push(values);
                        }
                        Ok(TableSnapshot {
                            columns,
                            rows: json_rows,
                            note: None,
                        })
                    }
                    Err(err) => Ok(empty_table(format!(
                        "Could not read saved view `{view}`: {err}"
                    ))),
                }
            }
            Err(err) => Ok(empty_table(format!(
                "Could not load saved view `{view}`: {err}"
            ))),
        },
        Err(err) => Ok(empty_table(format!(
            "Could not open package for saved view `{view}`: {err}"
        ))),
    }
}

fn cell_to_json(value: Option<&lattice_data::CellValue>) -> serde_json::Value {
    match value {
        None | Some(lattice_data::CellValue::Null) => serde_json::Value::Null,
        Some(lattice_data::CellValue::Text(v)) | Some(lattice_data::CellValue::Date(v)) => {
            json!(v)
        }
        Some(lattice_data::CellValue::Integer(v)) => json!(v),
        Some(lattice_data::CellValue::Decimal(v)) => json!(v),
        Some(lattice_data::CellValue::Boolean(v)) => json!(v),
        Some(lattice_data::CellValue::Relation { record_ids }) => json!(record_ids),
        Some(lattice_data::CellValue::Lookup { values }) => json!(values),
        Some(lattice_data::CellValue::Rollup { value }) => json!(value),
        Some(lattice_data::CellValue::Formula { value }) => match value {
            None => serde_json::Value::Null,
            Some(lattice_data::FormulaValue::Text(v)) => json!(v),
            Some(lattice_data::FormulaValue::Number(v)) => json!(v),
        },
    }
}

/// Freeze artifact bindings for offline injection.
pub fn freeze_artifact_bindings(
    workspace_root: &Path,
    bindings: &std::collections::BTreeMap<String, BindingSpec>,
) -> Result<Vec<ArtifactBindingSnapshot>> {
    let mut out = Vec::new();
    for (name, binding) in bindings {
        out.push(freeze_artifact_binding(workspace_root, name, binding)?);
    }
    Ok(out)
}

fn freeze_artifact_binding(
    workspace_root: &Path,
    name: &str,
    binding: &BindingSpec,
) -> Result<ArtifactBindingSnapshot> {
    match binding {
        BindingSpec::SqliteQuery {
            resource,
            sql,
            limit,
        } => match DataApp::open(&workspace_root.join(resource)) {
            Ok(app) => match app.query_sql_table(sql, (*limit).max(1)) {
                Ok((columns, rows)) => {
                    let (column, value) = if columns.len() == 1 && rows.len() <= 1 {
                        (
                            columns.first().cloned(),
                            rows.first().and_then(|r| r.first()).cloned(),
                        )
                    } else {
                        (None, None)
                    };
                    let is_scalar = value.is_some();
                    Ok(ArtifactBindingSnapshot {
                        name: name.into(),
                        kind: if is_scalar {
                            "scalar".into()
                        } else {
                            "table".into()
                        },
                        value,
                        column,
                        path: None,
                        view: None,
                        table: if is_scalar {
                            None
                        } else {
                            Some(TableSnapshot {
                                columns,
                                rows,
                                note: None,
                            })
                        },
                        note: None,
                    })
                }
                Err(err) => Ok(ArtifactBindingSnapshot {
                    name: name.into(),
                    kind: "unsupported".into(),
                    value: None,
                    column: None,
                    path: None,
                    view: None,
                    table: Some(empty_table(format!("Could not freeze sqlite-query: {err}"))),
                    note: Some(err.to_string()),
                }),
            },
            Err(err) => Ok(ArtifactBindingSnapshot {
                name: name.into(),
                kind: "unsupported".into(),
                value: None,
                column: None,
                path: None,
                view: None,
                table: Some(empty_table(format!("Could not open `{resource}`: {err}"))),
                note: Some(err.to_string()),
            }),
        },
        BindingSpec::Resource { resource } => Ok(ArtifactBindingSnapshot {
            name: name.into(),
            kind: "resource".into(),
            value: None,
            column: None,
            path: Some(resource.clone()),
            view: None,
            table: None,
            note: None,
        }),
        BindingSpec::SavedView { resource, view } => Ok(ArtifactBindingSnapshot {
            name: name.into(),
            kind: "saved-view".into(),
            value: None,
            column: None,
            path: Some(resource.clone()),
            view: Some(view.clone()),
            table: Some(snapshot_saved_view(
                workspace_root,
                &workspace_root.join(resource),
                resource,
                view,
            )?),
            note: None,
        }),
        BindingSpec::DuckdbQuery { .. }
        | BindingSpec::NotebookOutput { .. }
        | BindingSpec::TaskOutput { .. } => Ok(ArtifactBindingSnapshot {
            name: name.into(),
            kind: "unsupported".into(),
            value: None,
            column: None,
            path: None,
            view: None,
            table: Some(empty_table(
                "Binding type is declared but not frozen in this export.",
            )),
            note: Some("Binding type not frozen in static export.".into()),
        }),
    }
}

pub fn render_table_html(table: &TableSnapshot) -> String {
    let mut html = String::from("<div class=\"lt-table-wrap\"><table class=\"lt-table\">");
    if !table.columns.is_empty() {
        html.push_str("<thead><tr>");
        for col in &table.columns {
            html.push_str("<th>");
            html.push_str(&escape_html(col));
            html.push_str("</th>");
        }
        html.push_str("</tr></thead>");
    }
    html.push_str("<tbody>");
    if table.rows.is_empty() {
        let colspan = table.columns.len().max(1);
        let note = table
            .note
            .as_deref()
            .unwrap_or("No rows in this frozen snapshot.");
        html.push_str(&format!(
            "<tr><td colspan=\"{colspan}\" class=\"lt-muted\">{}</td></tr>",
            escape_html(note)
        ));
    } else {
        for row in &table.rows {
            html.push_str("<tr>");
            for cell in row {
                html.push_str("<td>");
                html.push_str(&escape_html(&value_display(cell)));
                html.push_str("</td>");
            }
            html.push_str("</tr>");
        }
    }
    html.push_str("</tbody></table></div>");
    html
}

fn value_display(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(path, text).map_err(|source| Error::io(path, source))
}
