use std::path::PathBuf;

use lattice_core::{
    ensure_lattice_home, initialize_active_lattice_home, lattice_dev_reset_demo_enabled,
    DefaultWorkspaceStatus, ProvisionDiagnostic, TemplateDescriptor, WorkspaceCreationMode,
    WorkspaceCreationPlan, WorkspaceProvisioner,
};
use lattice_profile::register_workspace;
use serde::Serialize;

use crate::workspace::{snapshot_from_workspace, WorkspaceSnapshot};

/// Snapshot of `~/Lattice` after ensuring the layout exists.
#[derive(Debug, Serialize)]
pub struct LatticeHomeInfo {
    pub root: String,
    pub workspaces: String,
    pub settings: String,
    pub default_workspace: Option<WorkspaceSnapshot>,
    pub diagnostics: Vec<ProvisionDiagnostic>,
    /// True when `LATTICE_DEV_RESET_DEMO` wiped and re-seeded First Look.
    #[serde(rename = "demoReset")]
    pub demo_reset: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProvisionResult {
    pub workspace: WorkspaceSnapshot,
    pub default_workspace_status: DefaultWorkspaceStatus,
    pub diagnostics: Vec<ProvisionDiagnostic>,
}

pub fn ensure_home() -> Result<LatticeHomeInfo, String> {
    let demo_reset = lattice_dev_reset_demo_enabled();
    let (home, outcome) = initialize_active_lattice_home().map_err(|err| err.to_string())?;
    let workspace_id = outcome.workspace.manifest().id.to_string();
    let _ = register_workspace(&workspace_id, outcome.workspace.root());
    let default_workspace = Some(snapshot_from_workspace(&outcome.workspace)?);
    Ok(LatticeHomeInfo {
        root: home.root.to_string_lossy().into_owned(),
        workspaces: home.workspaces.to_string_lossy().into_owned(),
        settings: home.settings.to_string_lossy().into_owned(),
        default_workspace,
        diagnostics: outcome.diagnostics,
        demo_reset,
    })
}

pub fn create_workspace(
    path: String,
    title: Option<String>,
    template: String,
    set_default: bool,
    initialize_existing: bool,
) -> Result<WorkspaceProvisionResult, String> {
    let root = PathBuf::from(&path);
    let title = title.unwrap_or_else(|| {
        root.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string()
    });
    let mut outcome = WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
        target: root,
        title,
        template_id: template,
        mode: if initialize_existing {
            WorkspaceCreationMode::ExistingDirectory
        } else {
            WorkspaceCreationMode::NewDirectory
        },
    })
    .map_err(|err| err.to_string())?;
    if set_default {
        match ensure_lattice_home()
            .map_err(|error| error.to_string())
            .and_then(|home| {
                home.set_default_workspace(outcome.workspace.root())
                    .map_err(|error| error.to_string())
            }) {
            Ok(_) => outcome.default_workspace_status = DefaultWorkspaceStatus::Updated,
            Err(error) => {
                outcome.default_workspace_status = DefaultWorkspaceStatus::Failed;
                outcome.diagnostics.push(ProvisionDiagnostic {
                    code: "default-workspace-save-failed".into(),
                    message: format!(
                        "The workspace was created, but Lattice could not make it the default: {error}"
                    ),
                    retryable: true,
                });
            }
        }
    }
    Ok(WorkspaceProvisionResult {
        workspace: snapshot_from_workspace(&outcome.workspace)?,
        default_workspace_status: outcome.default_workspace_status,
        diagnostics: outcome.diagnostics,
    })
}

/// Built-in workspace templates for the New Workspace gallery and First Look sample.
pub fn list_templates() -> Vec<TemplateDescriptor> {
    lattice_core::WorkspaceTemplate::catalog()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_profile::{
        default_workspace_registry_path, WorkspaceRegistry, LATTICE_HOME_ENV,
        LATTICE_WORKSPACE_REGISTRY_PATH_ENV,
    };
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn ensure_home_registers_personal_workspace_in_registry() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var(LATTICE_HOME_ENV, directory.path());
        std::env::remove_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV);

        let info = ensure_home().unwrap();
        let default = info.default_workspace.expect("default workspace");
        assert!(default.root.contains("Personal"));

        let registry = WorkspaceRegistry::load_default().unwrap();
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.list()[0].workspace_id, default.id);
        assert_eq!(
            registry.list()[0]
                .root
                .canonicalize()
                .unwrap_or_else(|_| registry.list()[0].root.clone()),
            std::path::PathBuf::from(&default.root)
                .canonicalize()
                .unwrap_or_else(|_| std::path::PathBuf::from(&default.root))
        );
        assert_eq!(
            default_workspace_registry_path().parent().unwrap().parent().unwrap(),
            directory.path()
        );

        std::env::remove_var(LATTICE_HOME_ENV);
    }
}
