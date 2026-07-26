//! Paths under `.lattice/connectors/<provider>/`.

use std::path::{Path, PathBuf};

use lattice_core::OPERATIONAL_DIR;

use crate::error::{path_display, Error, Result};

pub const GITHUB_PROVIDER: &str = "github";
pub const GITLAB_PROVIDER: &str = "gitlab";

/// Relative operational directory for GitHub connector state.
pub const GITHUB_CONNECTOR_DIR: &str = "connectors/github";
/// Relative operational directory for GitLab connector state.
pub const GITLAB_CONNECTOR_DIR: &str = "connectors/gitlab";

pub fn connectors_provider_dir(workspace_root: &Path, provider: &str) -> PathBuf {
    workspace_root
        .join(OPERATIONAL_DIR)
        .join("connectors")
        .join(provider)
}

pub fn connectors_github_dir(workspace_root: &Path) -> PathBuf {
    connectors_provider_dir(workspace_root, GITHUB_PROVIDER)
}

pub fn connectors_gitlab_dir(workspace_root: &Path) -> PathBuf {
    connectors_provider_dir(workspace_root, GITLAB_PROVIDER)
}

pub fn binding_yaml_path_for(workspace_root: &Path, provider: &str, binding_id: &str) -> PathBuf {
    connectors_provider_dir(workspace_root, provider).join(format!("{binding_id}.yaml"))
}

pub fn binding_yaml_path(workspace_root: &Path, binding_id: &str) -> PathBuf {
    binding_yaml_path_for(workspace_root, GITHUB_PROVIDER, binding_id)
}

pub fn checkout_dir_for(workspace_root: &Path, provider: &str, binding_id: &str) -> PathBuf {
    connectors_provider_dir(workspace_root, provider)
        .join(binding_id)
        .join("checkout")
}

pub fn checkout_dir(workspace_root: &Path, binding_id: &str) -> PathBuf {
    checkout_dir_for(workspace_root, GITHUB_PROVIDER, binding_id)
}

/// True when `rel` (workspace-relative) points at any connector extract or
/// binding under `.lattice/connectors/`.
pub fn is_under_operational_connectors(rel: &Path) -> bool {
    let mut components = rel.components();
    match (
        components.next(),
        components.next(),
        components.next(),
    ) {
        (
            Some(std::path::Component::Normal(op)),
            Some(std::path::Component::Normal(connectors)),
            _,
        ) if op == OPERATIONAL_DIR && connectors == "connectors" => true,
        _ => false,
    }
}

/// True when `rel` is inside a GitHub or GitLab checkout extract.
pub fn is_connector_extract_path(rel: &Path) -> bool {
    let parts: Vec<_> = rel
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    parts.len() >= 5
        && parts[0] == OPERATIONAL_DIR
        && parts[1] == "connectors"
        && (parts[2] == GITHUB_PROVIDER || parts[2] == GITLAB_PROVIDER)
        && parts[4] == "checkout"
}

/// Resolve a path inside a checkout, rejecting `..` and escapes.
pub fn resolve_in_checkout_for(
    workspace_root: &Path,
    provider: &str,
    binding_id: &str,
    rel_in_repo: &str,
) -> Result<PathBuf> {
    if binding_id.trim().is_empty()
        || binding_id.contains('/')
        || binding_id.contains('\\')
        || binding_id.contains("..")
    {
        return Err(Error::sandbox(format!(
            "invalid binding id {binding_id:?}"
        )));
    }
    if provider.contains('/') || provider.contains('\\') || provider.contains("..") {
        return Err(Error::sandbox(format!("invalid provider {provider:?}")));
    }
    let checkout = checkout_dir_for(workspace_root, provider, binding_id);
    if !checkout.is_dir() {
        return Err(Error::NotFound(format!(
            "checkout missing for binding {binding_id}"
        )));
    }
    let rel = Path::new(rel_in_repo);
    if rel.is_absolute()
        || rel.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(Error::sandbox(format!(
            "path {rel_in_repo:?} escapes the checkout"
        )));
    }
    let candidate = checkout.join(rel);
    let canonical_checkout = checkout.canonicalize().map_err(|err| {
        Error::sandbox(format!(
            "cannot canonicalize checkout {}: {err}",
            path_display(&checkout)
        ))
    })?;
    let canonical_candidate = if candidate.exists() {
        candidate.canonicalize().map_err(|err| {
            Error::sandbox(format!("cannot resolve {rel_in_repo:?}: {err}"))
        })?
    } else {
        let parent = candidate.parent().unwrap_or(&checkout);
        let canonical_parent = if parent.exists() {
            parent.canonicalize().map_err(|err| {
                Error::sandbox(format!("cannot resolve parent of {rel_in_repo:?}: {err}"))
            })?
        } else {
            return Err(Error::NotFound(rel_in_repo.to_string()));
        };
        canonical_parent.join(candidate.file_name().unwrap_or_default())
    };
    if !canonical_candidate.starts_with(&canonical_checkout) {
        return Err(Error::sandbox(format!(
            "{rel_in_repo:?} escapes the checkout"
        )));
    }
    Ok(canonical_candidate)
}

pub fn resolve_in_checkout(
    workspace_root: &Path,
    binding_id: &str,
    rel_in_repo: &str,
) -> Result<PathBuf> {
    resolve_in_checkout_for(workspace_root, GITHUB_PROVIDER, binding_id, rel_in_repo)
}

pub fn list_binding_ids_for(workspace_root: &Path, provider: &str) -> Result<Vec<String>> {
    let dir = connectors_provider_dir(workspace_root, provider);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(id) = name.strip_suffix(".yaml") {
            if !id.is_empty() {
                ids.push(id.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

pub fn list_binding_ids(workspace_root: &Path) -> Result<Vec<String>> {
    list_binding_ids_for(workspace_root, GITHUB_PROVIDER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use std::path::PathBuf;

    #[test]
    fn extract_path_detection() {
        assert!(is_connector_extract_path(Path::new(
            ".lattice/connectors/github/abc/checkout/README.md"
        )));
        assert!(is_connector_extract_path(Path::new(
            ".lattice/connectors/gitlab/abc/checkout/README.md"
        )));
        assert!(is_under_operational_connectors(Path::new(
            ".lattice/connectors/github/abc.yaml"
        )));
        assert!(!is_connector_extract_path(Path::new("Notes/a.md")));
        assert!(!is_under_operational_connectors(Path::new("Notes/a.md")));
    }

    #[test]
    fn resolve_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        let binding = "bind1";
        let checkout = checkout_dir(dir.path(), binding);
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::write(checkout.join("ok.md"), "x").unwrap();
        assert!(resolve_in_checkout(dir.path(), binding, "../escape").is_err());
        assert!(resolve_in_checkout(dir.path(), binding, "ok.md").is_ok());
    }

    #[test]
    fn nested_lattice_yaml_does_not_change_paths() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        let binding = "bind1";
        let checkout = checkout_dir(dir.path(), binding);
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::write(checkout.join("lattice.yaml"), "name: Nested\n").unwrap();
        let rel = PathBuf::from(format!(
            "{OPERATIONAL_DIR}/connectors/github/{binding}/checkout/lattice.yaml"
        ));
        assert!(is_connector_extract_path(&rel));
        assert_eq!(
            checkout_dir(dir.path(), binding),
            dir.path()
                .join(OPERATIONAL_DIR)
                .join("connectors/github")
                .join(binding)
                .join("checkout")
        );
    }
}
