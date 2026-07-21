use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::types::{FieldType, RollupAggregate};
use crate::Result;

pub const APP_MANIFEST_FILENAME: &str = "app.yaml";
pub const SCHEMA_FILENAME: &str = "schema.sql";
pub const DATABASE_FILENAME: &str = "database.sqlite";
pub const DEFAULT_VIEW_NAME: &str = "All";

pub const DATA_APP_FORMAT: &str = "lattice-data-app";
pub const SUPPORTED_VERSION: u32 = 1;

/// Parsed `app.yaml` for a `.data` package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppManifest {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub default_table: String,
    pub default_view: String,
    pub database: String,
    pub schema: String,
    #[serde(default)]
    pub tables: BTreeMap<String, TableMeta>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TableMeta {
    #[serde(default)]
    pub columns: BTreeMap<String, ColumnMetaYaml>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnMetaYaml {
    #[serde(rename = "type")]
    pub field_type: FieldType,
    /// Target table for [`FieldType::Relation`] (same `.data` package).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_table: Option<String>,
    /// Optional SQLite junction table for M2M relation storage (demo opt-in).
    ///
    /// When set, linked ids live in `{junction}.(source_id, target_id)` instead of
    /// JSON TEXT on the relation column. The TEXT column remains as a NULL placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub junction_table: Option<String>,
    /// Source relation column on this table for [`FieldType::Lookup`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookup_relation: Option<String>,
    /// Field on the related table projected by [`FieldType::Lookup`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookup_field: Option<String>,
    /// Source relation column on this table for [`FieldType::Rollup`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollup_relation: Option<String>,
    /// Aggregate applied by [`FieldType::Rollup`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollup_aggregate: Option<RollupAggregate>,
    /// Related-table field aggregated by [`FieldType::Rollup`] (optional for count).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollup_field: Option<String>,
    /// Expression for [`FieldType::Formula`] (e.g. `{price} * {quantity}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

impl AppManifest {
    pub fn new(title: impl Into<String>, default_table: impl Into<String>) -> Self {
        AppManifest {
            format: DATA_APP_FORMAT.to_string(),
            version: SUPPORTED_VERSION,
            id: uuid::Uuid::now_v7().to_string(),
            title: title.into(),
            default_table: default_table.into(),
            default_view: DEFAULT_VIEW_NAME.to_string(),
            database: format!("./{DATABASE_FILENAME}"),
            schema: format!("./{SCHEMA_FILENAME}"),
            tables: BTreeMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let manifest: AppManifest = serde_yaml::from_str(&text).map_err(|source| Error::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
        manifest.check(path)?;
        Ok(manifest)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_yaml::to_string(self).expect("app manifest serializes");
        std::fs::write(path, text).map_err(|source| Error::io(path, source))
    }

    fn check(&self, path: &Path) -> Result<()> {
        let invalid = |message: String| Error::InvalidPackage {
            path: path.to_path_buf(),
            message,
        };
        if self.format != DATA_APP_FORMAT {
            return Err(invalid(format!(
                "expected format {DATA_APP_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version > SUPPORTED_VERSION {
            return Err(invalid(format!(
                "manifest version {} is newer than supported version {SUPPORTED_VERSION}",
                self.version
            )));
        }
        Ok(())
    }

    pub fn ensure_default_table(&mut self, table_name: &str) {
        self.tables
            .entry(table_name.to_string())
            .or_default()
            .columns
            .entry("id".to_string())
            .or_insert(ColumnMetaYaml {
                field_type: FieldType::Text,
                relation_table: None,
                junction_table: None,
                lookup_relation: None,
                lookup_field: None,
                rollup_relation: None,
                rollup_aggregate: None,
                rollup_field: None,
                formula: None,
            });
    }

    pub fn column_yaml(&self, table: &str, column: &str) -> Option<&ColumnMetaYaml> {
        self.tables
            .get(table)
            .and_then(|table_meta| table_meta.columns.get(column))
    }
}

pub(crate) fn app_manifest_path(package_path: &Path) -> PathBuf {
    package_path.join(APP_MANIFEST_FILENAME)
}

pub(crate) fn schema_path(package_path: &Path) -> PathBuf {
    package_path.join(SCHEMA_FILENAME)
}

pub(crate) fn database_path(package_path: &Path) -> PathBuf {
    package_path.join(DATABASE_FILENAME)
}

pub(crate) fn default_view_path(package_path: &Path) -> PathBuf {
    package_path
        .join("views")
        .join(format!("{DEFAULT_VIEW_NAME}.yaml"))
}

pub(crate) fn write_default_view(package_path: &Path, table_name: &str) -> Result<()> {
    let views_dir = package_path.join("views");
    std::fs::create_dir_all(&views_dir).map_err(|source| Error::io(&views_dir, source))?;
    let view_path = default_view_path(package_path);
    let contents = format!(
        "format: lattice-view\nversion: 1\nsource:\n  database: ../{DATABASE_FILENAME}\n  table: {table_name}\nlayout:\n  type: grid\n"
    );
    std::fs::write(&view_path, contents).map_err(|source| Error::io(&view_path, source))
}
