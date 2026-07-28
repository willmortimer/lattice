//! Execution manifest (YAML/JSON) describing a scoped KernelFS run.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Top-level execution manifest for a KernelFS run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionManifest {
    pub run_id: String,
    pub base_snapshot: String,
    #[serde(default)]
    pub mounts: Mounts,
    #[serde(default)]
    pub capabilities: Capabilities,
}

/// Mount configuration for the standard KernelFS projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mounts {
    /// Read-only host files copied under `input/` in the run directory.
    #[serde(default)]
    pub input: Vec<InputMount>,
    /// Workspace-relative prefix where `/output` artifacts should be proposed.
    #[serde(default)]
    pub output_proposal_target: Option<String>,
    /// Guest-relative paths under `/work` to promote into the output commit plan.
    #[serde(default)]
    pub work_promote_paths: Vec<String>,
}

impl Default for Mounts {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            output_proposal_target: None,
            work_promote_paths: Vec::new(),
        }
    }
}

/// One authorized input file exposed at `/input/<guest_path>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMount {
    /// Absolute or workspace-relative host path to copy from.
    pub host_path: PathBuf,
    /// Guest-relative path under `/input` (must not escape via `..`).
    pub guest_path: String,
}

/// Deny-by-default capability projection for the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub network: NetworkPolicy,
    #[serde(default)]
    pub secrets: Vec<SecretHandle>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            network: NetworkPolicy::default(),
            secrets: Vec::new(),
        }
    }
}

/// Network access policy (deny-by-default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPolicy {
    #[serde(default)]
    pub allow: Vec<String>,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self { allow: Vec::new() }
    }
}

/// Opaque secret capability handle (not copied into the run directory).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretHandle {
    pub id: String,
}

impl ExecutionManifest {
    /// Parse a manifest from YAML text.
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text)
    }

    /// Parse a manifest from JSON text.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Serialize to YAML.
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
