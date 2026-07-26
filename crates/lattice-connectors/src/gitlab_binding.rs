//! GitLab project binding YAML schema.

use serde::{Deserialize, Serialize};

use crate::binding::{BindingCredentials, BindingMode, ExtractConfig, ExtractStrategy};

pub const GITLAB_BINDING_KIND: &str = "gitlab.repo";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitLabRepoBinding {
    pub kind: String,
    pub id: String,
    /// Full GitLab path (`group/subgroup/project`).
    pub path_with_namespace: String,
    pub owner: String,
    pub repo: String,
    pub project_id: u64,
    pub default_branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_sha: Option<String>,
    pub mode: BindingMode,
    pub credentials: BindingCredentials,
    pub extract: ExtractConfig,
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refreshed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl GitLabRepoBinding {
    pub fn new_read_only(
        id: String,
        path_with_namespace: String,
        project_id: u64,
        default_branch: String,
        credential_key: String,
        extract_rel_path: String,
    ) -> Self {
        let (owner, repo) = split_namespace(&path_with_namespace);
        Self {
            kind: GITLAB_BINDING_KIND.to_string(),
            id,
            path_with_namespace,
            owner,
            repo,
            project_id,
            default_branch,
            head_sha: None,
            mode: BindingMode::Read,
            credentials: BindingCredentials {
                provider: "keychain".into(),
                key: credential_key,
            },
            extract: ExtractConfig {
                strategy: ExtractStrategy::ShallowClone,
                depth: 1,
                path: extract_rel_path,
            },
            capabilities: vec!["list".into(), "read".into(), "snapshot".into()],
            last_refreshed_at: None,
            stale: None,
            last_error: None,
        }
    }

    pub fn full_name(&self) -> String {
        self.path_with_namespace.clone()
    }

    pub fn allows_mutate(&self) -> bool {
        self.capabilities.iter().any(|c| c == "mutate")
            || !matches!(self.mode, BindingMode::Read)
    }
}

fn split_namespace(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((owner, repo)) => (owner.to_string(), repo.to_string()),
        None => (String::new(), path.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_yaml() {
        let binding = GitLabRepoBinding::new_read_only(
            "abc".into(),
            "acme/widget".into(),
            42,
            "main".into(),
            "lattice.gitlab.abc".into(),
            ".lattice/connectors/gitlab/abc/checkout".into(),
        );
        assert_eq!(binding.owner, "acme");
        assert_eq!(binding.repo, "widget");
        let yaml = serde_yaml::to_string(&binding).unwrap();
        let parsed: GitLabRepoBinding = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.kind, GITLAB_BINDING_KIND);
    }
}
