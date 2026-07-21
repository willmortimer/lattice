//! Shared data-binding contracts for interfaces, embeds, and dashboards.
//!
//! Paths are workspace-relative. Query bindings are read-only by default;
//! mutations go through semantic commands or proposals.

use serde::{Deserialize, Serialize};

/// How an interface (or embed) component loads its data.
///
/// Serialized with kebab-case `type` tags and camelCase field names for IPC
/// JSON; YAML interface files use the same tags (`duckdb-query`, `saved-view`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum BindingSpec {
    /// Open or preview a workspace resource as-is.
    Resource {
        resource: String,
    },
    /// Open a saved view inside a `.data` package.
    SavedView {
        resource: String,
        view: String,
    },
    /// Bounded read-only SQL against a `.data` package SQLite database.
    SqliteQuery {
        resource: String,
        sql: String,
        #[serde(default = "default_query_limit")]
        limit: usize,
    },
    /// Bounded DuckDB SQL over one or more `.dataset` packages.
    DuckdbQuery {
        resources: Vec<String>,
        sql: String,
        #[serde(default = "default_query_limit")]
        limit: usize,
    },
    /// Bind to a notebook cell output.
    NotebookOutput {
        resource: String,
        #[serde(rename = "cellId")]
        cell_id: String,
    },
    /// Bind to a named task output artifact.
    TaskOutput {
        resource: String,
        name: String,
    },
}

fn default_query_limit() -> usize {
    10_000
}

impl BindingSpec {
    /// Workspace-relative resource paths referenced by this binding.
    pub fn resource_paths(&self) -> Vec<&str> {
        match self {
            BindingSpec::Resource { resource }
            | BindingSpec::SavedView { resource, .. }
            | BindingSpec::SqliteQuery { resource, .. }
            | BindingSpec::NotebookOutput { resource, .. }
            | BindingSpec::TaskOutput { resource, .. } => vec![resource.as_str()],
            BindingSpec::DuckdbQuery { resources, .. } => {
                resources.iter().map(String::as_str).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_spec_json_round_trip_all_variants() {
        let cases = [
            BindingSpec::Resource {
                resource: "CRM.data".into(),
            },
            BindingSpec::SavedView {
                resource: "CRM.data".into(),
                view: "Board".into(),
            },
            BindingSpec::SqliteQuery {
                resource: "CRM.data".into(),
                sql: "SELECT COUNT(*) AS value FROM contacts".into(),
                limit: 1,
            },
            BindingSpec::DuckdbQuery {
                resources: vec!["Data/Orders.dataset".into()],
                sql: "SELECT SUM(revenue) AS value FROM read_parquet('Data/Orders.dataset/facts/**/*.parquet', hive_partitioning = true, union_by_name = true)".into(),
                limit: 1,
            },
            BindingSpec::NotebookOutput {
                resource: "Notebooks/Orders analytics.ipynb".into(),
                cell_id: "cell-1".into(),
            },
            BindingSpec::TaskOutput {
                resource: "tasks/hello.task".into(),
                name: "report".into(),
            },
        ];

        for binding in cases {
            let json = serde_json::to_string(&binding).unwrap();
            assert!(json.contains("\"type\":"));
            let parsed: BindingSpec = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, binding);
        }
    }

    #[test]
    fn binding_spec_json_uses_kebab_type_and_camel_cell_id() {
        let binding = BindingSpec::NotebookOutput {
            resource: "n.ipynb".into(),
            cell_id: "abc".into(),
        };
        let json = serde_json::to_string(&binding).unwrap();
        assert!(json.contains("\"type\":\"notebook-output\""));
        assert!(json.contains("\"cellId\":\"abc\""));
        assert!(!json.contains("cell_id"));
    }

    #[test]
    fn binding_spec_yaml_round_trip() {
        let binding = BindingSpec::DuckdbQuery {
            resources: vec!["Data/Orders.dataset".into()],
            sql: "SELECT 1 AS value".into(),
            limit: 1,
        };
        let yaml = serde_yaml::to_string(&binding).unwrap();
        assert!(yaml.contains("type: duckdb-query"));
        let parsed: BindingSpec = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, binding);
    }
}
