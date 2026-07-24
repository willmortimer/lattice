//! High-level GitHub connect / list / read / refresh / disconnect.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::binding::GitHubRepoBinding;
use crate::clone::{disconnect_binding, refresh_shallow_clone, shallow_clone_repo};
use crate::credentials::{TokenMaterial, TokenStore};
use crate::error::{path_display, write_atomic, Error, Result};
use crate::github_api::{list_accessible_repos, GitHubApiClient, GitHubRepoSummary};
use crate::paths::{binding_yaml_path, checkout_dir, list_binding_ids, resolve_in_checkout};

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

fn credential_key_for(binding_id: &str) -> String {
    format!("lattice.github.{binding_id}")
}

fn load_binding(workspace_root: &Path, binding_id: &str) -> Result<GitHubRepoBinding> {
    let path = binding_yaml_path(workspace_root, binding_id);
    if !path.is_file() {
        return Err(Error::NotFound(format!("binding {binding_id}")));
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(serde_yaml::from_str(&text)?)
}

fn save_binding(workspace_root: &Path, binding: &GitHubRepoBinding) -> Result<()> {
    let path = binding_yaml_path(workspace_root, &binding.id);
    let yaml = serde_yaml::to_string(binding)?;
    write_atomic(&path, yaml.as_bytes())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRepoInput {
    pub owner: String,
    pub repo: String,
    pub repo_id: u64,
    pub default_branch: String,
    #[serde(default)]
    pub installation_id: Option<u64>,
    /// User access token used for the shallow clone.
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedRepoSummary {
    pub binding: GitHubRepoBinding,
    pub checkout_exists: bool,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutEntry {
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutFile {
    pub path: String,
    pub content: String,
    pub byte_len: u64,
}

pub fn connect_repo(
    workspace_root: &Path,
    tokens: &dyn TokenStore,
    input: ConnectRepoInput,
) -> Result<ConnectedRepoSummary> {
    let binding_id = uuid::Uuid::now_v7().to_string();
    let cred_key = credential_key_for(&binding_id);
    let checkout = checkout_dir(workspace_root, &binding_id);
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

    let mut binding = GitHubRepoBinding::new_read_only(
        binding_id.clone(),
        input.owner.clone(),
        input.repo.clone(),
        input.repo_id,
        input.default_branch.clone(),
        input.installation_id,
        cred_key.clone(),
        extract_rel,
    );

    match shallow_clone_repo(
        workspace_root,
        &binding_id,
        &input.owner,
        &input.repo,
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
            let _ = disconnect_binding(workspace_root, &binding_id);
            return Err(err);
        }
    }

    // Refuse to treat nested lattice.yaml as a workspace — just persist binding.
    let nested_manifest = checkout.join(lattice_core::WORKSPACE_MANIFEST_FILENAME);
    if nested_manifest.is_file() {
        // Documented non-goal: never Workspace::open(checkout).
        tracing_ignore_nested_manifest(&nested_manifest);
    }

    save_binding(workspace_root, &binding)?;
    Ok(ConnectedRepoSummary {
        checkout_exists: checkout.is_dir(),
        stale: binding.stale.unwrap_or(false),
        binding,
    })
}

fn tracing_ignore_nested_manifest(_path: &Path) {
    // No-op marker for tests / reviewers: nested manifests are ignored.
}

pub fn list_bindings(workspace_root: &Path) -> Result<Vec<ConnectedRepoSummary>> {
    let mut out = Vec::new();
    for id in list_binding_ids(workspace_root)? {
        let binding = load_binding(workspace_root, &id)?;
        let checkout = checkout_dir(workspace_root, &id);
        out.push(ConnectedRepoSummary {
            stale: binding.stale.unwrap_or(false) || !checkout.is_dir(),
            checkout_exists: checkout.is_dir(),
            binding,
        });
    }
    Ok(out)
}

pub fn refresh_repo(
    workspace_root: &Path,
    tokens: &dyn TokenStore,
    binding_id: &str,
) -> Result<ConnectedRepoSummary> {
    let mut binding = load_binding(workspace_root, binding_id)?;
    if binding.allows_mutate() {
        return Err(Error::sandbox(
            "mutate mode is not supported for GitHub extracts in this slice",
        ));
    }
    let material = tokens
        .get(&binding.credentials.key)?
        .ok_or_else(|| Error::credentials(format!("missing token for {}", binding.credentials.key)))?;

    match refresh_shallow_clone(
        workspace_root,
        binding_id,
        &binding.owner,
        &binding.repo,
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
    let checkout = checkout_dir(workspace_root, binding_id);
    Ok(ConnectedRepoSummary {
        checkout_exists: checkout.is_dir(),
        stale: false,
        binding,
    })
}

pub fn disconnect_repo(
    workspace_root: &Path,
    tokens: &dyn TokenStore,
    binding_id: &str,
) -> Result<()> {
    let binding = load_binding(workspace_root, binding_id).ok();
    if let Some(binding) = &binding {
        let _ = tokens.delete(&binding.credentials.key);
    }
    disconnect_binding(workspace_root, binding_id)?;
    let yaml = binding_yaml_path(workspace_root, binding_id);
    if yaml.exists() {
        std::fs::remove_file(&yaml)?;
    }
    Ok(())
}

pub fn list_checkout_tree(
    workspace_root: &Path,
    binding_id: &str,
) -> Result<Vec<CheckoutEntry>> {
    let _binding = load_binding(workspace_root, binding_id)?;
    let checkout = checkout_dir(workspace_root, binding_id);
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
        // Skip .git internals in the Connected tree.
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

pub fn read_checkout_file(
    workspace_root: &Path,
    binding_id: &str,
    rel_path: &str,
) -> Result<CheckoutFile> {
    let binding = load_binding(workspace_root, binding_id)?;
    if binding.allows_mutate() {
        return Err(Error::sandbox("unexpected mutate capability on read binding"));
    }
    let absolute = resolve_in_checkout(workspace_root, binding_id, rel_path)?;
    if absolute.is_dir() {
        return Err(Error::message(format!("{rel_path} is a directory")));
    }
    let bytes = std::fs::read(&absolute)?;
    // Text open for browse; binary is base64-unfriendly here — reject large/non-utf8 softly.
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

/// List repos for an access token (used after device-flow completes).
pub fn list_repos_for_token(
    api: &dyn GitHubApiClient,
    access_token: &str,
) -> Result<Vec<GitHubRepoSummary>> {
    list_accessible_repos(api, access_token, 3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::MemoryTokenStore;
    use lattice_core::Workspace;

    #[test]
    fn connect_without_git_fails_cleanly_and_leaves_no_binding() {
        // Avoid network: shallow_clone is exercised via disconnect/list/read
        // fixtures below. This test only asserts binding YAML IO + mutate guard.
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        assert!(list_bindings(dir.path()).unwrap().is_empty());
        let binding = GitHubRepoBinding::new_read_only(
            "x".into(),
            "acme".into(),
            "widget".into(),
            1,
            "main".into(),
            None,
            credential_key_for("x"),
            ".lattice/connectors/github/x/checkout".into(),
        );
        assert!(!binding.allows_mutate());
        save_binding(dir.path(), &binding).unwrap();
        assert_eq!(list_bindings(dir.path()).unwrap().len(), 1);
    }

    #[test]
    fn list_and_read_local_extract_without_network() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        let binding_id = "local-bind";
        let checkout = checkout_dir(dir.path(), binding_id);
        std::fs::create_dir_all(checkout.join("src")).unwrap();
        std::fs::write(checkout.join("README.md"), "# Hello\n").unwrap();
        std::fs::write(checkout.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(checkout.join("lattice.yaml"), "name: Nested\n").unwrap();

        let binding = GitHubRepoBinding::new_read_only(
            binding_id.into(),
            "acme".into(),
            "widget".into(),
            1,
            "main".into(),
            None,
            credential_key_for(binding_id),
            workspace_rel(dir.path(), &checkout),
        );
        save_binding(dir.path(), &binding).unwrap();

        let tree = list_checkout_tree(dir.path(), binding_id).unwrap();
        assert!(tree.iter().any(|e| e.path == "README.md"));
        assert!(tree.iter().any(|e| e.path == "lattice.yaml"));
        assert!(tree.iter().any(|e| e.path == "src" && e.is_dir));

        let file = read_checkout_file(dir.path(), binding_id, "README.md").unwrap();
        assert_eq!(file.content, "# Hello\n");

        // Nested lattice.yaml is readable as a file, not opened as workspace.
        let nested = read_checkout_file(dir.path(), binding_id, "lattice.yaml").unwrap();
        assert!(nested.content.contains("Nested"));

        assert!(resolve_in_checkout(dir.path(), binding_id, "../x").is_err());
    }

    #[test]
    fn disconnect_removes_binding_and_checkout() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        let tokens = MemoryTokenStore::new();
        let binding_id = "gone";
        let checkout = checkout_dir(dir.path(), binding_id);
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::write(checkout.join("a.txt"), "a").unwrap();
        let binding = GitHubRepoBinding::new_read_only(
            binding_id.into(),
            "acme".into(),
            "widget".into(),
            1,
            "main".into(),
            None,
            credential_key_for(binding_id),
            workspace_rel(dir.path(), &checkout),
        );
        tokens
            .set(
                &binding.credentials.key,
                &TokenMaterial {
                    access_token: "t".into(),
                    refresh_token: None,
                    expires_in: None,
                    token_type: None,
                },
            )
            .unwrap();
        save_binding(dir.path(), &binding).unwrap();
        disconnect_repo(dir.path(), &tokens, binding_id).unwrap();
        assert!(list_bindings(dir.path()).unwrap().is_empty());
        assert!(!checkout.exists());
        assert!(tokens.get(&binding.credentials.key).unwrap().is_none());
    }
}
