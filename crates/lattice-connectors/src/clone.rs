//! Shallow clone / refresh / disconnect for GitHub Extract checkouts.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{path_display, remove_dir_if_exists, Error, Result};
use crate::paths::checkout_dir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneOutcome {
    pub checkout: PathBuf,
    pub head_sha: Option<String>,
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

/// Authenticated HTTPS clone URL using a GitHub access token.
pub fn authenticated_clone_url(owner: &str, repo: &str, token: &str) -> String {
    format!("https://x-access-token:{token}@github.com/{owner}/{repo}.git")
}

pub fn shallow_clone_repo(
    workspace_root: &Path,
    binding_id: &str,
    owner: &str,
    repo: &str,
    token: &str,
    depth: u32,
) -> Result<CloneOutcome> {
    let checkout = checkout_dir(workspace_root, binding_id);
    if let Some(parent) = checkout.parent() {
        std::fs::create_dir_all(parent)?;
    }
    remove_dir_if_exists(&checkout)?;
    let url = authenticated_clone_url(owner, repo, token);
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
    // Drop stored credentials from the remote URL after clone.
    let _ = run_git(
        Some(&checkout),
        &[
            "remote",
            "set-url",
            "origin",
            &format!("https://github.com/{owner}/{repo}.git"),
        ],
    );
    let head_sha = run_git(Some(&checkout), &["rev-parse", "HEAD"]).ok();
    Ok(CloneOutcome { checkout, head_sha })
}

/// Fetch and hard-reset to `origin/<default_branch>` using a fresh token via
/// a temporary remote URL rewrite.
pub fn refresh_shallow_clone(
    workspace_root: &Path,
    binding_id: &str,
    owner: &str,
    repo: &str,
    default_branch: &str,
    token: &str,
) -> Result<CloneOutcome> {
    let checkout = checkout_dir(workspace_root, binding_id);
    if !checkout.is_dir() {
        return Err(Error::NotFound(format!(
            "checkout missing for binding {binding_id}"
        )));
    }
    let auth_url = authenticated_clone_url(owner, repo, token);
    let public_url = format!("https://github.com/{owner}/{repo}.git");
    run_git(Some(&checkout), &["remote", "set-url", "origin", &auth_url])?;
    let fetch_result = run_git(
        Some(&checkout),
        &["fetch", "--depth", "1", "origin", default_branch],
    );
    let reset_ref = format!("origin/{default_branch}");
    let reset_result = fetch_result.and_then(|_| {
        run_git(Some(&checkout), &["reset", "--hard", &reset_ref])
    });
    // Always restore the non-credential remote URL.
    let _ = run_git(
        Some(&checkout),
        &["remote", "set-url", "origin", &public_url],
    );
    reset_result?;
    let head_sha = run_git(Some(&checkout), &["rev-parse", "HEAD"]).ok();
    Ok(CloneOutcome { checkout, head_sha })
}

pub fn disconnect_binding(workspace_root: &Path, binding_id: &str) -> Result<()> {
    let checkout = checkout_dir(workspace_root, binding_id);
    remove_dir_if_exists(&checkout)?;
    // Remove binding directory if empty parent leftovers exist.
    if let Some(parent) = checkout.parent() {
        remove_dir_if_exists(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_url_embeds_token() {
        let url = authenticated_clone_url("acme", "widget", "ghu_secret");
        assert!(url.contains("x-access-token:ghu_secret@github.com/acme/widget.git"));
    }
}
