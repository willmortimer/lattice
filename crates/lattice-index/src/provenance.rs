use serde::{Deserialize, Serialize};

/// Resource sensitivity attached to indexed chunks and search hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Sensitivity {
    #[default]
    Workspace,
    Private,
    Secret,
}

impl Sensitivity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Private => "private",
            Self::Secret => "secret",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "private" => Self::Private,
            "secret" => Self::Secret,
            _ => Self::Workspace,
        }
    }
}

/// External-export policy attached to a search hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExportPolicy {
    #[default]
    Ask,
    Allow,
    Deny,
}

impl ExportPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "allow" => Self::Allow,
            "deny" => Self::Deny,
            _ => Self::Ask,
        }
    }
}

/// Traceable derived-state metadata for one hybrid hit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProvenance {
    pub content_hash: String,
    pub chunker_version: String,
    pub namespace_key: Option<String>,
    pub model_id: Option<String>,
    pub model_revision: Option<String>,
    pub instruction_version: Option<String>,
}
