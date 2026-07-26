//! Shallow clone / refresh / disconnect for git Extract checkouts.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{path_display, remove_dir_if_exists, Error, Result};
use crate::paths::{checkout_dir_for, GITHUB_PROVIDER, GITLAB_PROVIDER};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneOutcome {
    pub checkout: PathBuf,
    pub head_sha: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitForge {
    GitHub,
    GitLab,
}

impl GitForge {
    pub fn provider_id(self) -> &'static str {
        match self {
            Self::GitHub => GITHUB_PROVIDER,
            Self::GitLab => GITLAB_PROVIDER,
        }
    }

    pub fn public_https_host(self) -> &'static str {
        match self {
            Self::GitHub => "github.com",
            Self::GitLab => "gitlab.com",
        }
    }
}

fn run_git(cwd: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd
        .output()
        .map_err(|err| Error::git(format!("failed to spawn git: {err}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::git(format!(
            "git {} failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Authenticated HTTPS clone URL (`owner/repo` or GitLab path_with_namespace).
pub fn authenticated_clone_url(forge: GitForge, path_with_namespace: &str, token: &str) -> String {
    match forge {
        GitForge::GitHub => {
            format!(
                "https://x-access-token:{token}@github.com/{path_with_namespace}.git"
            )
        }
        GitForge::GitLab => {
            format!("https://oauth2:{token}@gitlab.com/{path_with_namespace}.git")
        }
    }
}

/// GitHub-specific helper kept for existing call sites.
pub fn authenticated_github_clone_url(owner: &str, repo: &str, token: &str) -> String {
    authenticated_clone_url(GitForge::GitHub, &format!("{owner}/{repo}"), token)
}

/// Back-compat name used by GitHub service tests.
pub fn authenticated_clone_url_github(owner: &str, repo: &str, token: &str) -> String {
    authenticated_github_clone_url(owner, repo, token)
}

pub fn shallow_clone_repo_for(
    workspace_root: &Path,
    forge: GitForge,
    binding_id: &str,
    path_with_namespace: &str,
    token: &str,
    depth: u32,
) -> Result<CloneOutcome> {
    let checkout = checkout_dir_for(workspace_root, forge.provider_id(), binding_id);
    if let Some(parent) = checkout.parent() {
        std::fs::create_dir_all(parent)?;
    }
    remove_dir_if_exists(&checkout)?;
    let url = authenticated_clone_url(forge, path_with_namespace, token);
    let depth_arg = depth.max(1).to_string();
    run_git(
        None,
        &[
            "clone",
            "--depth",
            &depth_arg,
            "--single-branch",
            &url,
            &path_display(&checkout),
        ],
    )?;
    let public_url = format!(
        "https://{}/{path_with_namespace}.git",
        forge.public_https_host()
    );
    let _ = run_git(
        Some(&checkout),
        &["remote", "set-url", "origin", &public_url],
    );
    let head_sha = run_git(Some(&checkout), &["rev-parse", "HEAD"]).ok();
    Ok(CloneOutcome { checkout, head_sha })
}

pub fn shallow_clone_repo(
    workspace_root: &Path,
    binding_id: &str,
    owner: &str,
    repo: &str,
    token: &str,
    depth: u32,
) -> Result<CloneOutcome> {
    shallow_clone_repo_for(
        workspace_root,
        GitForge::GitHub,
        binding_id,
        &format!("{owner}/{repo}"),
        token,
        depth,
    )
}

pub fn refresh_shallow_clone_for(
    workspace_root: &Path,
    forge: GitForge,
    binding_id: &str,
    path_with_namespace: &str,
    default_branch: &str,
    token: &str,
) -> Result<CloneOutcome> {
    let checkout = checkout_dir_for(workspace_root, forge.provider_id(), binding_id);
    if !checkout.is_dir() {
        return Err(Error::NotFound(format!(
            "checkout missing for binding {binding_id}"
        )));
    }
    let auth_url = authenticated_clone_url(forge, path_with_namespace, token);
    let public_url = format!(
        "https://{}/{path_with_namespace}.git",
        forge.public_https_host()
    );
    run_git(Some(&checkout), &["remote", "set-url", "origin", &auth_url])?;
    let fetch_result = run_git(
        Some(&checkout),
        &["fetch", "--depth", "1", "origin", default_branch],
    );
    let reset_ref = format!("origin/{default_branch}");
    let reset_result =
        fetch_result.and_then(|_| run_git(Some(&checkout), &["reset", "--hard", &reset_ref]));
    let _ = run_git(
        Some(&checkout),
        &["remote", "set-url", "origin", &public_url],
    );
    reset_result?;
    let head_sha = run_git(Some(&checkout), &["rev-parse", "HEAD"]).ok();
    Ok(CloneOutcome { checkout, head_sha })
}

pub fn refresh_shallow_clone(
    workspace_root: &Path,
    binding_id: &str,
    owner: &str,
    repo: &str,
    default_branch: &str,
    token: &str,
) -> Result<CloneOutcome> {
    refresh_shallow_clone_for(
        workspace_root,
        GitForge::GitHub,
        binding_id,
        &format!("{owner}/{repo}"),
        default_branch,
        token,
    )
}

pub fn disconnect_binding_for(
    workspace_root: &Path,
    provider: &str,
    binding_id: &str,
) -> Result<()> {
    let checkout = checkout_dir_for(workspace_root, provider, binding_id);
    remove_dir_if_exists(&checkout)?;
    if let Some(parent) = checkout.parent() {
        remove_dir_if_exists(parent)?;
    }
    Ok(())
}

pub fn disconnect_binding(workspace_root: &Path, binding_id: &str) -> Result<()> {
    disconnect_binding_for(workspace_root, GITHUB_PROVIDER, binding_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_url_embeds_token() {
        let url = authenticated_github_clone_url("acme", "widget", "ghu_secret");
        assert!(url.contains("x-access-token:ghu_secret@github.com/acme/widget.git"));
        let gl = authenticated_clone_url(GitForge::GitLab, "acme/widget", "glpat");
        assert!(gl.contains("oauth2:glpat@gitlab.com/acme/widget.git"));
    }
}
