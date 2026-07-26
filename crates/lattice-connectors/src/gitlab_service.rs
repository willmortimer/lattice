//! High-level GitLab connect / list / read / refresh / disconnect.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::clone::{
    disconnect_binding_for, refresh_shallow_clone_for, shallow_clone_repo_for, GitForge,
};
use crate::credentials::{binding_token_key_for, TokenMaterial, TokenStore};
use crate::error::{path_display, write_atomic, Error, Result};
use crate::gitlab_api::{list_accessible_projects, GitLabApiClient, GitLabProjectSummary};
use crate::gitlab_binding::GitLabRepoBinding;
use crate::paths::{
    binding_yaml_path_for, checkout_dir_for, list_binding_ids_for, resolve_in_checkout_for,
    GITLAB_PROVIDER,
};
use crate::service::{CheckoutEntry, CheckoutFile};

fn workspace_rel(root: &Path, absolute: &Path) -> String {
    absolute
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path_display(absolute))
}

fn now_rfc3339_approx() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn load_binding(workspace_root: &Path, binding_id: &str) -> Result<GitLabRepoBinding> {
    let path = binding_yaml_path_for(workspace_root, GITLAB_PROVIDER, binding_id);
    if !path.is_file() {
        return Err(Error::NotFound(format!("binding {binding_id}")));
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(serde_yaml::from_str(&text)?)
}

fn save_binding(workspace_root: &Path, binding: &GitLabRepoBinding) -> Result<()> {
    let path = binding_yaml_path_for(workspace_root, GITLAB_PROVIDER, &binding.id);
    let yaml = serde_yaml::to_string(binding)?;
    write_atomic(&path, yaml.as_bytes())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectGitLabRepoInput {
    pub path_with_namespace: String,
    pub project_id: u64,
    pub default_branch: String,
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedGitLabRepoSummary {
    pub binding: GitLabRepoBinding,
    pub checkout_exists: bool,
    pub stale: bool,
}

pub fn connect_gitlab_repo(
    workspace_root: &Path,
    tokens: &dyn TokenStore,
    input: ConnectGitLabRepoInput,
) -> Result<ConnectedGitLabRepoSummary> {
    let binding_id = uuid::Uuid::now_v7().to_string();
    let cred_key = binding_token_key_for(GITLAB_PROVIDER, &binding_id);
    let checkout = checkout_dir_for(workspace_root, GITLAB_PROVIDER, &binding_id);
    let extract_rel = workspace_rel(workspace_root, &checkout);

    tokens.set(
        &cred_key,
        &TokenMaterial {
            access_token: input.access_token.clone(),
            refresh_token: None,
            expires_in: None,
            token_type: Some("bearer".into()),
        },
    )?;

    let mut binding = GitLabRepoBinding::new_read_only(
        binding_id.clone(),
        input.path_with_namespace.clone(),
        input.project_id,
        input.default_branch.clone(),
        cred_key.clone(),
        extract_rel,
    );

    match shallow_clone_repo_for(
        workspace_root,
        GitForge::GitLab,
        &binding_id,
        &input.path_with_namespace,
        &input.access_token,
        1,
    ) {
        Ok(outcome) => {
            binding.head_sha = outcome.head_sha;
            binding.last_refreshed_at = Some(now_rfc3339_approx());
            binding.stale = Some(false);
            binding.last_error = None;
        }
        Err(err) => {
            let _ = tokens.delete(&cred_key);
            let _ = disconnect_binding_for(workspace_root, GITLAB_PROVIDER, &binding_id);
            return Err(err);
        }
    }

    save_binding(workspace_root, &binding)?;
    Ok(ConnectedGitLabRepoSummary {
        checkout_exists: checkout.is_dir(),
        stale: binding.stale.unwrap_or(false),
        binding,
    })
}

pub fn list_gitlab_bindings(workspace_root: &Path) -> Result<Vec<ConnectedGitLabRepoSummary>> {
    let mut out = Vec::new();
    for id in list_binding_ids_for(workspace_root, GITLAB_PROVIDER)? {
        let binding = load_binding(workspace_root, &id)?;
        let checkout = checkout_dir_for(workspace_root, GITLAB_PROVIDER, &id);
        out.push(ConnectedGitLabRepoSummary {
            stale: binding.stale.unwrap_or(false) || !checkout.is_dir(),
            checkout_exists: checkout.is_dir(),
            binding,
        });
    }
    Ok(out)
}

pub fn refresh_gitlab_repo(
    workspace_root: &Path,
    tokens: &dyn TokenStore,
    binding_id: &str,
) -> Result<ConnectedGitLabRepoSummary> {
    let mut binding = load_binding(workspace_root, binding_id)?;
    if binding.allows_mutate() {
        return Err(Error::sandbox(
            "mutate mode is not supported for GitLab extracts in this slice",
        ));
    }
    let material = tokens
        .get(&binding.credentials.key)?
        .ok_or_else(|| Error::credentials(format!("missing token for {}", binding.credentials.key)))?;

    match refresh_shallow_clone_for(
        workspace_root,
        GitForge::GitLab,
        binding_id,
        &binding.path_with_namespace,
        &binding.default_branch,
        &material.access_token,
    ) {
        Ok(outcome) => {
            binding.head_sha = outcome.head_sha;
            binding.last_refreshed_at = Some(now_rfc3339_approx());
            binding.stale = Some(false);
            binding.last_error = None;
        }
        Err(err) => {
            binding.stale = Some(true);
            binding.last_error = Some(err.to_string());
            save_binding(workspace_root, &binding)?;
            return Err(err);
        }
    }
    save_binding(workspace_root, &binding)?;
    let checkout = checkout_dir_for(workspace_root, GITLAB_PROVIDER, binding_id);
    Ok(ConnectedGitLabRepoSummary {
        checkout_exists: checkout.is_dir(),
        stale: false,
        binding,
    })
}

pub fn disconnect_gitlab_repo(
    workspace_root: &Path,
    tokens: &dyn TokenStore,
    binding_id: &str,
) -> Result<()> {
    let binding = load_binding(workspace_root, binding_id).ok();
    if let Some(binding) = &binding {
        let _ = tokens.delete(&binding.credentials.key);
    }
    disconnect_binding_for(workspace_root, GITLAB_PROVIDER, binding_id)?;
    let yaml = binding_yaml_path_for(workspace_root, GITLAB_PROVIDER, binding_id);
    if yaml.exists() {
        std::fs::remove_file(&yaml)?;
    }
    Ok(())
}

pub fn list_gitlab_checkout_tree(
    workspace_root: &Path,
    binding_id: &str,
) -> Result<Vec<CheckoutEntry>> {
    let _binding = load_binding(workspace_root, binding_id)?;
    let checkout = checkout_dir_for(workspace_root, GITLAB_PROVIDER, binding_id);
    if !checkout.is_dir() {
        return Err(Error::NotFound(format!(
            "checkout missing for binding {binding_id}"
        )));
    }
    let mut entries = Vec::new();
    for entry in WalkDir::new(&checkout).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path == checkout {
            continue;
        }
        if path.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        let rel = path
            .strip_prefix(&checkout)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if rel.is_empty() {
            continue;
        }
        let is_dir = entry.file_type().is_dir();
        let size = if is_dir {
            None
        } else {
            entry.metadata().ok().map(|m| m.len())
        };
        entries.push(CheckoutEntry {
            path: rel,
            is_dir,
            size,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

pub fn read_gitlab_checkout_file(
    workspace_root: &Path,
    binding_id: &str,
    rel_path: &str,
) -> Result<CheckoutFile> {
    let binding = load_binding(workspace_root, binding_id)?;
    if binding.allows_mutate() {
        return Err(Error::sandbox("unexpected mutate capability on read binding"));
    }
    let absolute =
        resolve_in_checkout_for(workspace_root, GITLAB_PROVIDER, binding_id, rel_path)?;
    if absolute.is_dir() {
        return Err(Error::message(format!("{rel_path} is a directory")));
    }
    let bytes = std::fs::read(&absolute)?;
    const MAX_TEXT: usize = 2 * 1024 * 1024;
    if bytes.len() > MAX_TEXT {
        return Err(Error::message(format!(
            "{rel_path} exceeds read-only text limit ({MAX_TEXT} bytes)"
        )));
    }
    let content = String::from_utf8(bytes).map_err(|_| {
        Error::message(format!("{rel_path} is not valid UTF-8 text"))
    })?;
    Ok(CheckoutFile {
        byte_len: content.len() as u64,
        path: rel_path.replace('\\', "/"),
        content,
    })
}

pub fn list_gitlab_projects_for_token(
    api: &dyn GitLabApiClient,
    access_token: &str,
) -> Result<Vec<GitLabProjectSummary>> {
    list_accessible_projects(api, access_token, 3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::MemoryTokenStore;
    use lattice_core::Workspace;

    #[test]
    fn list_and_read_local_extract_without_network() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        let binding_id = "local-gl";
        let checkout = checkout_dir_for(dir.path(), GITLAB_PROVIDER, binding_id);
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::write(checkout.join("README.md"), "# Hello\n").unwrap();

        let binding = GitLabRepoBinding::new_read_only(
            binding_id.into(),
            "acme/widget".into(),
            1,
            "main".into(),
            binding_token_key_for(GITLAB_PROVIDER, binding_id),
            workspace_rel(dir.path(), &checkout),
        );
        save_binding(dir.path(), &binding).unwrap();

        let tree = list_gitlab_checkout_tree(dir.path(), binding_id).unwrap();
        assert!(tree.iter().any(|e| e.path == "README.md"));
        let file = read_gitlab_checkout_file(dir.path(), binding_id, "README.md").unwrap();
        assert_eq!(file.content, "# Hello\n");

        let tokens = MemoryTokenStore::new();
        disconnect_gitlab_repo(dir.path(), &tokens, binding_id).unwrap();
        assert!(list_gitlab_bindings(dir.path()).unwrap().is_empty());
    }
}
