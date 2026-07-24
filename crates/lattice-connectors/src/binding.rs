//! GitHub repo binding YAML schema.

use serde::{Deserialize, Serialize};

pub const GITHUB_BINDING_KIND: &str = "github.repo";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BindingMode {
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractStrategy {
    ShallowClone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingCredentials {
    pub provider: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractConfig {
    pub strategy: ExtractStrategy,
    pub depth: u32,
    /// Workspace-relative path to the checkout directory.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRepoBinding {
    pub kind: String,
    pub id: String,
    pub owner: String,
    pub repo: String,
    pub repo_id: u64,
    pub default_branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<u64>,
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

impl GitHubRepoBinding {
    pub fn new_read_only(
        id: String,
        owner: String,
        repo: String,
        repo_id: u64,
        default_branch: String,
        installation_id: Option<u64>,
        credential_key: String,
        extract_rel_path: String,
    ) -> Self {
        Self {
            kind: GITHUB_BINDING_KIND.to_string(),
            id,
            owner,
            repo,
            repo_id,
            default_branch,
            head_sha: None,
            installation_id,
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
        format!("{}/{}", self.owner, self.repo)
    }

    pub fn allows_mutate(&self) -> bool {
        self.capabilities.iter().any(|c| c == "mutate")
            || !matches!(self.mode, BindingMode::Read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_yaml() {
        let binding = GitHubRepoBinding::new_read_only(
            "abc".into(),
            "acme".into(),
            "widget".into(),
            42,
            "main".into(),
            Some(9),
            "lattice.github.abc".into(),
            ".lattice/connectors/github/abc/checkout".into(),
        );
        assert!(!binding.allows_mutate());
        let yaml = serde_yaml::to_string(&binding).unwrap();
        let parsed: GitHubRepoBinding = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.kind, GITHUB_BINDING_KIND);
        assert_eq!(parsed.extract.depth, 1);
        assert_eq!(parsed.capabilities, vec!["list", "read", "snapshot"]);
    }
}
