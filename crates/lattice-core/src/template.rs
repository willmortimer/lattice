//! Validated, declarative workspace templates and safe provisioning.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lattice_data::{CellValue, DataApp, FieldType};
use lattice_storage::atomic_write_file;
use serde::{Deserialize, Serialize};

use crate::{
    parse_resource_links, Capabilities, DirectoryMeta, Error, ResourceCatalog, ResourceKind,
    ResourceLinkResolution, Result, Workspace, WorkspaceDefaults, WorkspaceManifest,
    WORKSPACE_MANIFEST_FILENAME,
};

#[derive(Debug)]
pub(crate) struct SeedFile {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

#[derive(Debug)]
pub(crate) struct SeedDirectory {
    pub path: &'static str,
    pub purpose: Option<&'static str>,
    pub default_kind: Option<&'static str>,
    pub icon: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct SeedDataColumn {
    pub name: &'static str,
    pub field_type: &'static str,
}

#[derive(Debug)]
pub(crate) struct SeedDataPackage {
    pub path: &'static str,
    pub title: &'static str,
    pub table: &'static str,
    pub columns: &'static [SeedDataColumn],
    pub rows_json: &'static [&'static str],
}

#[derive(Debug)]
pub(crate) struct GeneratedTemplate {
    pub id: &'static str,
    pub order: u32,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub visibility: &'static str,
    pub recommended: bool,
    pub recommended_title: &'static str,
    pub directories: &'static [SeedDirectory],
    pub preview: &'static [&'static str],
    pub capabilities: &'static [&'static str],
    pub quick_note_directory: &'static str,
    pub daily_note_directory: Option<&'static str>,
    pub attachments_directory: Option<&'static str>,
    pub template_directory: Option<&'static str>,
    pub archive_directory: Option<&'static str>,
    pub open_on_create: Option<&'static str>,
    pub files: &'static [SeedFile],
    pub data_packages: &'static [SeedDataPackage],
}

include!("template_catalog.generated.rs");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateVisibility {
    Gallery,
    Legacy,
    Sample,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDirectory {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDescriptor {
    pub id: String,
    pub order: u32,
    pub name: String,
    pub category: String,
    pub description: String,
    pub visibility: TemplateVisibility,
    pub recommended: bool,
    pub recommended_title: String,
    pub directories: Vec<TemplateDirectory>,
    pub preview: Vec<String>,
    pub capabilities: Vec<String>,
    pub quick_note_directory: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_note_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_on_create: Option<String>,
}

impl TemplateDescriptor {
    fn from_generated(template: &GeneratedTemplate) -> Self {
        Self {
            id: template.id.into(),
            order: template.order,
            name: template.name.into(),
            category: template.category.into(),
            description: template.description.into(),
            visibility: match template.visibility {
                "gallery" => TemplateVisibility::Gallery,
                "sample" => TemplateVisibility::Sample,
                _ => TemplateVisibility::Legacy,
            },
            recommended: template.recommended,
            recommended_title: template.recommended_title.into(),
            directories: template
                .directories
                .iter()
                .map(|directory| TemplateDirectory {
                    path: directory.path.into(),
                    purpose: directory.purpose.map(str::to_string),
                    default_kind: directory.default_kind.map(str::to_string),
                    icon: directory.icon.map(str::to_string),
                })
                .collect(),
            preview: strings(template.preview),
            capabilities: strings(template.capabilities),
            quick_note_directory: template.quick_note_directory.into(),
            daily_note_directory: template.daily_note_directory.map(str::to_string),
            attachments_directory: template.attachments_directory.map(str::to_string),
            template_directory: template.template_directory.map(str::to_string),
            archive_directory: template.archive_directory.map(str::to_string),
            open_on_create: template.open_on_create.map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTemplate {
    Personal,
    Project,
    Research,
    DataLab,
    Team,
    Demo,
    Blank,
}

impl WorkspaceTemplate {
    pub fn id(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Project => "project",
            Self::Research => "research",
            Self::DataLab => "data-lab",
            Self::Team => "team",
            Self::Demo => "demo",
            Self::Blank => "blank",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        let canonical = resolve_template_id(id)?;
        match canonical {
            "personal" => Some(Self::Personal),
            "project" => Some(Self::Project),
            "research" => Some(Self::Research),
            "data-lab" => Some(Self::DataLab),
            "team" => Some(Self::Team),
            "demo" => Some(Self::Demo),
            "blank" => Some(Self::Blank),
            _ => None,
        }
    }

    pub fn descriptor(self) -> TemplateDescriptor {
        TemplateDescriptor::from_generated(generated(self.id()).expect("built-in template exists"))
    }

    pub fn display_name(self) -> &'static str {
        generated(self.id()).expect("built-in template exists").name
    }

    pub fn description(self) -> &'static str {
        generated(self.id())
            .expect("built-in template exists")
            .description
    }

    pub fn gallery() -> Vec<TemplateDescriptor> {
        GENERATED_TEMPLATES
            .iter()
            .filter(|template| template.visibility == "gallery")
            .map(TemplateDescriptor::from_generated)
            .collect()
    }

    pub fn samples() -> Vec<TemplateDescriptor> {
        GENERATED_TEMPLATES
            .iter()
            .filter(|template| template.visibility == "sample")
            .map(TemplateDescriptor::from_generated)
            .collect()
    }

    /// Gallery templates plus First Look samples, in catalog order (excludes legacy).
    pub fn catalog() -> Vec<TemplateDescriptor> {
        GENERATED_TEMPLATES
            .iter()
            .filter(|template| {
                template.visibility == "gallery" || template.visibility == "sample"
            })
            .map(TemplateDescriptor::from_generated)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceCreationMode {
    NewDirectory,
    ExistingDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreationPlan {
    pub target: PathBuf,
    pub title: String,
    pub template_id: String,
    pub mode: WorkspaceCreationMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultWorkspaceStatus {
    NotRequested,
    Updated,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionDiagnostic {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug)]
pub struct WorkspaceProvisionOutcome {
    pub workspace: Workspace,
    pub default_workspace_status: DefaultWorkspaceStatus,
    pub diagnostics: Vec<ProvisionDiagnostic>,
}

pub struct WorkspaceProvisioner;

impl WorkspaceProvisioner {
    pub fn provision(plan: &WorkspaceCreationPlan) -> Result<WorkspaceProvisionOutcome> {
        let template = generated(&plan.template_id).ok_or_else(|| Error::TemplateValidation {
            message: format!("unknown workspace template {:?}", plan.template_id),
        })?;
        validate_target(&plan.target, template, plan.mode)?;
        let workspace = match plan.mode {
            WorkspaceCreationMode::NewDirectory => provision_staged(plan, template)?,
            WorkspaceCreationMode::ExistingDirectory => provision_existing(plan, template)?,
        };
        validate_instantiated_template(&workspace)?;
        Ok(WorkspaceProvisionOutcome {
            workspace,
            default_workspace_status: DefaultWorkspaceStatus::NotRequested,
            diagnostics: Vec::new(),
        })
    }
}

fn provision_staged(
    plan: &WorkspaceCreationPlan,
    template: &GeneratedTemplate,
) -> Result<Workspace> {
    let parent = plan
        .target
        .parent()
        .ok_or_else(|| Error::TemplateValidation {
            message: format!("target {} has no parent directory", plan.target.display()),
        })?;
    std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
    let name = plan
        .target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace");
    let stage = parent.join(format!(".{name}.lattice-stage-{}", uuid::Uuid::now_v7()));
    let result = (|| {
        let workspace = create_workspace_at(&stage, &plan.title, template)?;
        validate_instantiated_template(&workspace)?;
        std::fs::rename(&stage, &plan.target).map_err(|error| Error::io(&plan.target, error))?;
        Workspace::open(&plan.target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&stage);
    }
    result
}

fn provision_existing(
    plan: &WorkspaceCreationPlan,
    template: &GeneratedTemplate,
) -> Result<Workspace> {
    let mut created_files = Vec::new();
    let mut created_directories = Vec::new();
    let result = create_workspace_at_existing(
        &plan.target,
        &plan.title,
        template,
        &mut created_files,
        &mut created_directories,
    )
    .and_then(|workspace| {
        validate_instantiated_template(&workspace)?;
        Ok(workspace)
    });
    if result.is_err() {
        for path in created_files.iter().rev() {
            let _ = std::fs::remove_file(path);
        }
        // Data packages are non-empty directories; remove_dir_all clears nested SQLite files.
        for path in created_directories.iter().rev() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    result
}

fn create_workspace_at(
    root: &Path,
    title: &str,
    template: &GeneratedTemplate,
) -> Result<Workspace> {
    std::fs::create_dir_all(root).map_err(|error| Error::io(root, error))?;
    let manifest = manifest_for(title, template);
    let workspace = Workspace::init_with_manifest(root, manifest)?;
    materialize_template(root, template, None, None)?;
    Ok(workspace)
}

fn create_workspace_at_existing(
    root: &Path,
    title: &str,
    template: &GeneratedTemplate,
    created_files: &mut Vec<PathBuf>,
    created_directories: &mut Vec<PathBuf>,
) -> Result<Workspace> {
    materialize_template(
        root,
        template,
        Some(created_files),
        Some(created_directories),
    )?;
    let manifest_path = root.join(WORKSPACE_MANIFEST_FILENAME);
    manifest_for(title, template).save(&manifest_path)?;
    created_files.push(manifest_path);
    Workspace::open(root)
}

fn materialize_template(
    root: &Path,
    template: &GeneratedTemplate,
    mut created_files: Option<&mut Vec<PathBuf>>,
    mut created_directories: Option<&mut Vec<PathBuf>>,
) -> Result<()> {
    let date = provision_iso_date();
    // Seeds under the template directory must keep `{{title}}` / `{{date}}`
    // verbatim: they are instantiated per page at create time, not at
    // provision time.
    let preserved_prefix = template
        .template_directory
        .map(|directory| format!("{}/", directory.trim_end_matches('/')));
    for directory in template.directories {
        let path = root.join(directory.path);
        create_directory_tree(&path, root, created_directories.as_deref_mut())?;
    }
    for file in template.files {
        let relative = expand_seed_placeholders(file.path, &date);
        let path = root.join(&relative);
        if let Some(parent) = path.parent() {
            create_directory_tree(parent, root, created_directories.as_deref_mut())?;
        }
        let preserved = preserved_prefix
            .as_deref()
            .is_some_and(|prefix| relative.starts_with(prefix));
        let bytes = if preserved {
            file.bytes.to_vec()
        } else {
            expand_seed_file_bytes(file.bytes, &relative, &date)
        };
        atomic_write_file(&path, &bytes)
            .map_err(|error| Error::io(&path, std::io::Error::other(error.to_string())))?;
        if let Some(files) = created_files.as_deref_mut() {
            files.push(path);
        }
    }
    for package in template.data_packages {
        materialize_data_package(root, package, created_directories.as_deref_mut())?;
    }
    Ok(())
}

fn provision_iso_date() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Civil date from Unix day count (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn expand_seed_placeholders(value: &str, date: &str) -> String {
    value.replace("{{date}}", date)
}

fn expand_seed_file_bytes(bytes: &[u8], relative_path: &str, date: &str) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    if !text.contains("{{date}}") && !text.contains("{{title}}") {
        return bytes.to_vec();
    }
    let title = Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Untitled");
    expand_seed_placeholders(text, date)
        .replace("{{title}}", title)
        .into_bytes()
}

fn materialize_data_package(
    root: &Path,
    package: &SeedDataPackage,
    mut created_directories: Option<&mut Vec<PathBuf>>,
) -> Result<()> {
    let package_path = root.join(package.path);
    if let Some(parent) = package_path.parent() {
        create_directory_tree(parent, root, created_directories.as_deref_mut())?;
    }

    let package_existed = package_path.exists();
    let mut app = match DataApp::create(&package_path, package.title, package.table) {
        Ok(app) => app,
        Err(error) => {
            if !package_existed {
                let _ = std::fs::remove_dir_all(&package_path);
            }
            return Err(map_data_error(error));
        }
    };
    if let Some(directories) = created_directories.as_deref_mut() {
        directories.push(package_path.clone());
    }

    let columns: Vec<(&str, FieldType)> = package
        .columns
        .iter()
        .map(|column| {
            Ok((
                column.name,
                parse_field_type(column.field_type).ok_or_else(|| Error::TemplateValidation {
                    message: format!(
                        "data package {} has unknown column type {:?}",
                        package.path, column.field_type
                    ),
                })?,
            ))
        })
        .collect::<Result<_>>()?;
    app.add_columns(package.table, &columns)
        .map_err(map_data_error)?;

    let column_types: BTreeMap<&str, FieldType> = columns.into_iter().collect();
    for row_json in package.rows_json {
        let values = row_values_from_json(row_json, &column_types, package.path)?;
        app.insert_row(package.table, &values)
            .map_err(map_data_error)?;
    }
    Ok(())
}

fn row_values_from_json(
    row_json: &str,
    column_types: &BTreeMap<&str, FieldType>,
    package_path: &str,
) -> Result<BTreeMap<String, CellValue>> {
    let parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(row_json)
        .map_err(|error| Error::TemplateValidation {
            message: format!("data package {package_path} has invalid row JSON: {error}"),
        })?;
    let mut values = BTreeMap::new();
    for (key, value) in parsed {
        let field_type =
            column_types
                .get(key.as_str())
                .copied()
                .ok_or_else(|| Error::TemplateValidation {
                    message: format!(
                        "data package {package_path} row references unknown column {key:?}"
                    ),
                })?;
        values.insert(key, cell_from_json(&value, field_type, package_path)?);
    }
    Ok(values)
}

fn cell_from_json(
    value: &serde_json::Value,
    field_type: FieldType,
    package_path: &str,
) -> Result<CellValue> {
    match value {
        serde_json::Value::Null => Ok(CellValue::Null),
        serde_json::Value::Bool(flag) => match field_type {
            FieldType::Boolean => Ok(CellValue::Boolean(*flag)),
            FieldType::Text | FieldType::LongText => Ok(CellValue::Text(flag.to_string())),
            _ => Err(Error::TemplateValidation {
                message: format!(
                    "data package {package_path}: boolean value incompatible with {field_type}"
                ),
            }),
        },
        serde_json::Value::Number(number) => {
            match field_type {
                FieldType::Integer => number.as_i64().map(CellValue::Integer).ok_or_else(|| {
                    Error::TemplateValidation {
                        message: format!(
                            "data package {package_path}: number {number} is not a valid integer"
                        ),
                    }
                }),
                FieldType::Decimal => number.as_f64().map(CellValue::Decimal).ok_or_else(|| {
                    Error::TemplateValidation {
                        message: format!(
                            "data package {package_path}: number {number} is not a valid decimal"
                        ),
                    }
                }),
                FieldType::Boolean => number
                    .as_i64()
                    .map(|n| CellValue::Boolean(n != 0))
                    .ok_or_else(|| Error::TemplateValidation {
                        message: format!(
                            "data package {package_path}: number {number} is not a valid boolean"
                        ),
                    }),
                FieldType::Text | FieldType::LongText | FieldType::Date => {
                    Ok(CellValue::Text(number.to_string()))
                }
            }
        }
        serde_json::Value::String(text) => {
            match field_type {
                FieldType::Text | FieldType::LongText => Ok(CellValue::Text(text.clone())),
                FieldType::Date => Ok(CellValue::Date(text.clone())),
                FieldType::Boolean => Ok(CellValue::Boolean(matches!(
                    text.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                ))),
                FieldType::Integer => text.parse::<i64>().map(CellValue::Integer).map_err(|_| {
                    Error::TemplateValidation {
                        message: format!(
                            "data package {package_path}: {text:?} is not a valid integer"
                        ),
                    }
                }),
                FieldType::Decimal => text.parse::<f64>().map(CellValue::Decimal).map_err(|_| {
                    Error::TemplateValidation {
                        message: format!(
                            "data package {package_path}: {text:?} is not a valid decimal"
                        ),
                    }
                }),
            }
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(Error::TemplateValidation {
                message: format!(
                    "data package {package_path}: row cells must be JSON primitives or null"
                ),
            })
        }
    }
}

fn parse_field_type(value: &str) -> Option<FieldType> {
    match value {
        "text" => Some(FieldType::Text),
        "long_text" => Some(FieldType::LongText),
        "integer" => Some(FieldType::Integer),
        "decimal" => Some(FieldType::Decimal),
        "boolean" => Some(FieldType::Boolean),
        "date" => Some(FieldType::Date),
        _ => None,
    }
}

fn map_data_error(error: lattice_data::Error) -> Error {
    Error::TemplateValidation {
        message: error.to_string(),
    }
}

fn create_directory_tree(
    path: &Path,
    root: &Path,
    created: Option<&mut Vec<PathBuf>>,
) -> Result<()> {
    let mut missing = Vec::new();
    let mut current = path;
    while current.starts_with(root) && current != root && !current.exists() {
        missing.push(current.to_path_buf());
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent;
    }
    std::fs::create_dir_all(path).map_err(|error| Error::io(path, error))?;
    if let Some(created) = created {
        missing.reverse();
        created.extend(missing);
    }
    Ok(())
}

fn manifest_for(title: &str, template: &GeneratedTemplate) -> WorkspaceManifest {
    let mut manifest = WorkspaceManifest::new(title);
    manifest.source_template = Some(template.id.into());
    manifest.capabilities = Capabilities {
        enabled: strings(template.capabilities),
    };
    manifest.defaults = defaults_for(template);
    manifest.directories = directory_metadata_for(template);
    manifest
}

/// Directory purposes copied into `lattice.yaml` so workspace owners can edit
/// them after provisioning.
fn directory_metadata_for(template: &GeneratedTemplate) -> BTreeMap<String, DirectoryMeta> {
    template
        .directories
        .iter()
        .filter_map(|directory| {
            directory.purpose.map(|purpose| {
                (
                    directory.path.to_string(),
                    DirectoryMeta {
                        purpose: Some(purpose.to_string()),
                    },
                )
            })
        })
        .collect()
}

fn defaults_for(template: &GeneratedTemplate) -> WorkspaceDefaults {
    WorkspaceDefaults {
        quick_note_directory: template.quick_note_directory.into(),
        daily_note_directory: template.daily_note_directory.map(str::to_string),
        attachments_directory: template.attachments_directory.map(str::to_string),
        template_directory: template.template_directory.map(str::to_string),
        archive_directory: template.archive_directory.map(str::to_string),
    }
}

fn validate_target(
    target: &Path,
    template: &GeneratedTemplate,
    mode: WorkspaceCreationMode,
) -> Result<()> {
    match mode {
        WorkspaceCreationMode::NewDirectory if target.exists() => {
            return Err(Error::ProvisioningConflict {
                path: target.to_path_buf(),
            });
        }
        WorkspaceCreationMode::ExistingDirectory if !target.is_dir() => {
            return Err(Error::ProvisioningConflict {
                path: target.to_path_buf(),
            });
        }
        _ => {}
    }
    if mode == WorkspaceCreationMode::ExistingDirectory {
        for path in std::iter::once(WORKSPACE_MANIFEST_FILENAME)
            .chain(template.directories.iter().map(|directory| directory.path))
            .chain(template.files.iter().map(|file| file.path))
            .chain(template.data_packages.iter().map(|package| package.path))
        {
            let candidate = target.join(path);
            if candidate.exists() {
                return Err(Error::ProvisioningConflict { path: candidate });
            }
        }
    }
    Ok(())
}

fn validate_instantiated_template(workspace: &Workspace) -> Result<()> {
    let resources = workspace.scan()?;
    let catalog = ResourceCatalog::new(&resources);
    for resource in resources
        .iter()
        .filter(|resource| resource.kind == ResourceKind::Page)
    {
        let path = workspace.root().join(&resource.path);
        let content = std::fs::read_to_string(&path).map_err(|error| Error::io(&path, error))?;
        for link in parse_resource_links(&content) {
            match catalog.resolve(Some(&resource.path), &link.target) {
                ResourceLinkResolution::Found { .. } => {}
                ResourceLinkResolution::Ambiguous { candidates, .. } => {
                    return Err(Error::TemplateValidation {
                        message: format!(
                            "{} contains ambiguous link [[{}]] ({})",
                            resource.path.display(),
                            link.target,
                            candidates
                                .iter()
                                .map(|target| target.path.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    });
                }
                ResourceLinkResolution::Missing { .. } => {
                    return Err(Error::TemplateValidation {
                        message: format!(
                            "{} contains unresolved link [[{}]]",
                            resource.path.display(),
                            link.target
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

fn generated(id: &str) -> Option<&'static GeneratedTemplate> {
    GENERATED_TEMPLATES
        .iter()
        .find(|template| template.id == id)
}

/// Resolve a user-facing template id or alias to a catalog id.
///
/// Accepts any id present in [`GENERATED_TEMPLATES`], plus a few historical
/// aliases (`default` → `personal`, `first-look` → `demo`, …).
pub fn resolve_template_id(id: &str) -> Option<&'static str> {
    let normalized = id.trim().to_ascii_lowercase();
    let canonical = match normalized.as_str() {
        "default" => "personal",
        "data_lab" | "datalab" | "data" => "data-lab",
        "work" => "team",
        "sample" | "first-look" => "demo",
        "empty" | "none" => "blank",
        other => other,
    };
    generated(canonical).map(|template| template.id)
}

/// Catalog ids in declaration order (for CLI help / errors).
pub fn template_catalog_ids() -> Vec<&'static str> {
    GENERATED_TEMPLATES
        .iter()
        .map(|template| template.id)
        .collect()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

/// Apply a built-in template to a newly initialized workspace without
/// overwriting any user path.
pub fn apply_template(root: &Path, template: WorkspaceTemplate) -> Result<()> {
    let generated = generated(template.id()).expect("built-in template exists");
    for path in generated
        .directories
        .iter()
        .map(|directory| directory.path)
        .chain(generated.files.iter().map(|file| file.path))
        .chain(generated.data_packages.iter().map(|package| package.path))
    {
        let candidate = root.join(path);
        if candidate.exists() {
            return Err(Error::ProvisioningConflict { path: candidate });
        }
    }
    let mut created_files = Vec::new();
    let mut created_directories = Vec::new();
    materialize_template(
        root,
        generated,
        Some(&mut created_files),
        Some(&mut created_directories),
    )?;
    let mut manifest = Workspace::open(root)?.manifest().clone();
    manifest.source_template = Some(generated.id.into());
    manifest.capabilities = Capabilities {
        enabled: strings(generated.capabilities),
    };
    manifest.defaults = defaults_for(generated);
    manifest.directories = directory_metadata_for(generated);
    manifest.save(&root.join(WORKSPACE_MANIFEST_FILENAME))?;
    validate_instantiated_template(&Workspace::open(root)?)
}

pub fn init_with_template(
    root: &Path,
    title: impl Into<String>,
    template_id: &str,
) -> Result<Workspace> {
    let template_id = resolve_template_id(template_id).ok_or_else(|| Error::TemplateValidation {
        message: format!("unknown workspace template {template_id:?}"),
    })?;
    WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
        target: root.to_path_buf(),
        title: title.into(),
        template_id: template_id.into(),
        mode: if root.exists() {
            WorkspaceCreationMode::ExistingDirectory
        } else {
            WorkspaceCreationMode::NewDirectory
        },
    })
    .map(|outcome| outcome.workspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gallery_is_declarative_and_separates_sample_and_legacy_templates() {
        assert_eq!(
            WorkspaceTemplate::gallery()
                .into_iter()
                .map(|template| template.id)
                .collect::<Vec<_>>(),
            [
                "personal",
                "project",
                "research",
                "second-brain",
                "data-lab",
                "dev-notebook",
                "blank",
            ]
        );
        assert_eq!(WorkspaceTemplate::samples()[0].id, "demo");
        assert_eq!(
            WorkspaceTemplate::Team.descriptor().visibility,
            TemplateVisibility::Legacy
        );
    }

    #[test]
    fn catalog_includes_gallery_and_sample_but_not_legacy() {
        assert_eq!(
            WorkspaceTemplate::catalog()
                .into_iter()
                .map(|template| template.id)
                .collect::<Vec<_>>(),
            [
                "personal",
                "project",
                "research",
                "second-brain",
                "data-lab",
                "dev-notebook",
                "blank",
                "demo",
            ]
        );
    }

    #[test]
    fn provisioned_templates_have_no_unresolved_links() {
        let directory = tempfile::tempdir().unwrap();
        for template in WorkspaceTemplate::gallery()
            .into_iter()
            .chain(WorkspaceTemplate::samples())
        {
            let root = directory.path().join(&template.id);
            let outcome = WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
                target: root,
                title: template.recommended_title,
                template_id: template.id,
                mode: WorkspaceCreationMode::NewDirectory,
            })
            .unwrap();
            validate_instantiated_template(&outcome.workspace).unwrap();
        }
    }

    #[test]
    fn existing_folder_collisions_are_blocked_without_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("Home.md"), "user content").unwrap();
        assert!(matches!(
            WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
                target: directory.path().to_path_buf(),
                title: "Personal".into(),
                template_id: "personal".into(),
                mode: WorkspaceCreationMode::ExistingDirectory,
            }),
            Err(Error::ProvisioningConflict { .. })
        ));
        assert_eq!(
            std::fs::read_to_string(directory.path().join("Home.md")).unwrap(),
            "user content"
        );
        assert!(!directory.path().join(WORKSPACE_MANIFEST_FILENAME).exists());
    }

    #[test]
    fn concurrent_new_directory_creation_has_one_atomic_winner() {
        let directory = tempfile::tempdir().unwrap();
        let target = std::sync::Arc::new(directory.path().join("Personal"));
        let threads = (0..2)
            .map(|_| {
                let target = std::sync::Arc::clone(&target);
                std::thread::spawn(move || {
                    WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
                        target: target.as_ref().clone(),
                        title: "Personal".into(),
                        template_id: "personal".into(),
                        mode: WorkspaceCreationMode::NewDirectory,
                    })
                })
            })
            .collect::<Vec<_>>();
        let results = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        validate_instantiated_template(&Workspace::open(target.as_ref()).unwrap()).unwrap();
    }

    #[test]
    fn provisioned_templates_persist_rich_defaults_and_categories() {
        let personal = WorkspaceTemplate::Personal.descriptor();
        assert_eq!(personal.category, "Everyday");
        assert_eq!(personal.open_on_create.as_deref(), Some("Home.md"));
        assert_eq!(personal.daily_note_directory.as_deref(), Some("Journal"));
        assert_eq!(personal.template_directory.as_deref(), Some("Templates"));
        assert_eq!(personal.archive_directory.as_deref(), Some("Archive"));
        assert_eq!(
            personal
                .directories
                .iter()
                .find(|directory| directory.path == "Inbox")
                .and_then(|directory| directory.purpose.as_deref()),
            Some("Drop raw captures here — triage them into Projects or Library.")
        );

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("Personal");
        let outcome = WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
            target: root.clone(),
            title: "Personal".into(),
            template_id: "personal".into(),
            mode: WorkspaceCreationMode::NewDirectory,
        })
        .unwrap();
        assert_eq!(
            outcome.workspace.manifest().source_template.as_deref(),
            Some("personal")
        );
        assert_eq!(
            outcome
                .workspace
                .manifest()
                .defaults
                .daily_note_directory
                .as_deref(),
            Some("Journal")
        );
        assert_eq!(
            outcome
                .workspace
                .manifest()
                .defaults
                .template_directory
                .as_deref(),
            Some("Templates")
        );
        assert_eq!(
            outcome
                .workspace
                .manifest()
                .defaults
                .archive_directory
                .as_deref(),
            Some("Archive")
        );
        let date = provision_iso_date();
        let journal = root.join(format!("Journal/{date}.md"));
        assert!(journal.is_file(), "missing {}", journal.display());
        let body = std::fs::read_to_string(&journal).unwrap();
        assert!(body.contains(&date));
        assert!(!body.contains("{{date}}"));
        assert_eq!(
            WorkspaceTemplate::Blank.descriptor().category,
            "Data & Advanced"
        );
        assert_eq!(WorkspaceTemplate::Project.descriptor().category, "Work");
        assert_eq!(
            WorkspaceTemplate::Research.descriptor().category,
            "Knowledge & Research"
        );
        assert_eq!(
            WorkspaceTemplate::DataLab.descriptor().category,
            "Data & Advanced"
        );
        assert_eq!(WorkspaceTemplate::Demo.descriptor().category, "Sample");
        assert_eq!(WorkspaceTemplate::Team.descriptor().category, "Work");
    }

    #[test]
    fn provisioned_manifest_carries_editable_directory_purposes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("Personal");
        let outcome = WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
            target: root.clone(),
            title: "Personal".into(),
            template_id: "personal".into(),
            mode: WorkspaceCreationMode::NewDirectory,
        })
        .unwrap();
        assert_eq!(
            outcome
                .workspace
                .manifest()
                .directories
                .get("Inbox")
                .and_then(|meta| meta.purpose.as_deref()),
            Some("Drop raw captures here — triage them into Projects or Library.")
        );
        let yaml = std::fs::read_to_string(root.join(WORKSPACE_MANIFEST_FILENAME)).unwrap();
        assert!(yaml.contains("directories:"));
        assert!(yaml.contains("Inbox:"));
    }

    #[test]
    fn template_directory_seeds_keep_placeholders_for_create_time() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("Personal");
        WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
            target: root.clone(),
            title: "Personal".into(),
            template_id: "personal".into(),
            mode: WorkspaceCreationMode::NewDirectory,
        })
        .unwrap();
        let daily = std::fs::read_to_string(root.join("Templates/Daily.md")).unwrap();
        assert!(daily.contains("{{date}}"), "Daily.md lost {{{{date}}}}");
        assert!(daily.contains("{{title}}"), "Daily.md lost {{{{title}}}}");
        let meeting = std::fs::read_to_string(root.join("Templates/Meeting Note.md")).unwrap();
        assert!(meeting.contains("{{title}}"));
        // Seeds outside the template directory are still expanded.
        let home = std::fs::read_to_string(root.join("Home.md")).unwrap();
        assert!(!home.contains("{{date}}"));
    }

    #[test]
    fn resolve_template_id_accepts_catalog_ids_and_aliases() {
        assert_eq!(resolve_template_id("second-brain"), Some("second-brain"));
        assert_eq!(resolve_template_id("dev-notebook"), Some("dev-notebook"));
        assert_eq!(resolve_template_id("first-look"), Some("demo"));
        assert_eq!(resolve_template_id("default"), Some("personal"));
        assert_eq!(resolve_template_id("nope"), None);
        assert!(template_catalog_ids().contains(&"second-brain"));
        assert!(template_catalog_ids().contains(&"dev-notebook"));
    }

    #[test]
    fn existing_folder_rolls_back_when_final_validation_fails() {
        static FILES: &[SeedFile] = &[SeedFile {
            path: "Home.md",
            bytes: b"# Home\n\n[[Missing]]\n",
        }];
        static DIRECTORIES: &[SeedDirectory] = &[SeedDirectory {
            path: "Inbox",
            purpose: None,
            default_kind: None,
            icon: None,
        }];
        let template = GeneratedTemplate {
            id: "invalid",
            order: 1,
            name: "Invalid",
            category: "Test",
            description: "Invalid test template",
            visibility: "gallery",
            recommended: false,
            recommended_title: "Invalid",
            directories: DIRECTORIES,
            preview: &["Home.md"],
            capabilities: &["pages"],
            quick_note_directory: "Inbox",
            daily_note_directory: None,
            attachments_directory: None,
            template_directory: None,
            archive_directory: None,
            open_on_create: None,
            files: FILES,
            data_packages: &[],
        };
        let directory = tempfile::tempdir().unwrap();
        let plan = WorkspaceCreationPlan {
            target: directory.path().to_path_buf(),
            title: "Invalid".into(),
            template_id: "invalid".into(),
            mode: WorkspaceCreationMode::ExistingDirectory,
        };
        assert!(provision_existing(&plan, &template).is_err());
        assert!(!directory.path().join("Home.md").exists());
        assert!(!directory.path().join("Inbox").exists());
        assert!(!directory.path().join(WORKSPACE_MANIFEST_FILENAME).exists());
    }

    #[test]
    fn provisioned_data_packages_are_readable_with_seeded_rows() {
        static COLUMNS: &[SeedDataColumn] = &[
            SeedDataColumn {
                name: "name",
                field_type: "text",
            },
            SeedDataColumn {
                name: "email",
                field_type: "text",
            },
        ];
        static ROWS: &[&str] = &[
            r#"{"name":"Ada Lovelace","email":"ada@example.com"}"#,
            r#"{"name":"Grace Hopper","email":"grace@example.com"}"#,
        ];
        static PACKAGES: &[SeedDataPackage] = &[SeedDataPackage {
            path: "Data/Contacts.data",
            title: "Contacts",
            table: "contacts",
            columns: COLUMNS,
            rows_json: ROWS,
        }];
        static FILES: &[SeedFile] = &[SeedFile {
            path: "Home.md",
            bytes: b"# Home\n",
        }];
        let template = GeneratedTemplate {
            id: "contacts-fixture",
            order: 1,
            name: "Contacts Fixture",
            category: "Test",
            description: "Synthetic data package fixture",
            visibility: "gallery",
            recommended: false,
            recommended_title: "Contacts",
            directories: &[],
            preview: &["Home.md", "Data/Contacts.data"],
            capabilities: &["pages", "sqlite"],
            quick_note_directory: "Inbox",
            daily_note_directory: None,
            attachments_directory: None,
            template_directory: None,
            archive_directory: None,
            open_on_create: None,
            files: FILES,
            data_packages: PACKAGES,
        };

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("ContactsWorkspace");
        create_workspace_at(&root, "Contacts", &template).unwrap();

        let package_path = root.join("Data/Contacts.data");
        let app = DataApp::open(&package_path).unwrap();
        assert_eq!(app.title(), "Contacts");
        assert_eq!(app.default_table(), "contacts");
        let rows = app.list_rows("contacts", 10, 0).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].values.get("name"),
            Some(&CellValue::Text("Ada Lovelace".into()))
        );
        assert_eq!(
            rows[1].values.get("email"),
            Some(&CellValue::Text("grace@example.com".into()))
        );
    }

    #[test]
    fn existing_folder_rolls_back_failed_data_package_materialization() {
        static COLUMNS: &[SeedDataColumn] = &[SeedDataColumn {
            name: "name",
            field_type: "not_a_real_type",
        }];
        static PACKAGES: &[SeedDataPackage] = &[SeedDataPackage {
            path: "Data/Broken.data",
            title: "Broken",
            table: "broken",
            columns: COLUMNS,
            rows_json: &[],
        }];
        static FILES: &[SeedFile] = &[SeedFile {
            path: "Home.md",
            bytes: b"# Home\n",
        }];
        let template = GeneratedTemplate {
            id: "broken-data",
            order: 1,
            name: "Broken Data",
            category: "Test",
            description: "Fails while seeding a data package",
            visibility: "gallery",
            recommended: false,
            recommended_title: "Broken",
            directories: &[],
            preview: &["Home.md"],
            capabilities: &["pages", "sqlite"],
            quick_note_directory: "Inbox",
            daily_note_directory: None,
            attachments_directory: None,
            template_directory: None,
            archive_directory: None,
            open_on_create: None,
            files: FILES,
            data_packages: PACKAGES,
        };
        let directory = tempfile::tempdir().unwrap();
        let plan = WorkspaceCreationPlan {
            target: directory.path().to_path_buf(),
            title: "Broken".into(),
            template_id: "broken-data".into(),
            mode: WorkspaceCreationMode::ExistingDirectory,
        };
        assert!(provision_existing(&plan, &template).is_err());
        assert!(!directory.path().join("Home.md").exists());
        assert!(!directory.path().join("Data").exists());
        assert!(!directory.path().join("Data/Broken.data").exists());
        assert!(!directory.path().join(WORKSPACE_MANIFEST_FILENAME).exists());
    }
}
