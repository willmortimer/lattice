//! Connect JSON request/response DTOs for celld + guest invoke payloads.

use crate::hydrate::{NetworkAttachment, VolumeAttachment};

/// `cell.v1.CellService/ApplyCell` request (proto JSON camelCase).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCellRequest {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub idempotency_key: String,
    pub spec: CellSpec,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expected_spec_digest: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_recreate: bool,
}

/// ApplyCell response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCellResponse {
    #[serde(default)]
    pub operation: Option<Operation>,
    #[serde(default)]
    pub cell: Option<Cell>,
}

/// Minimal CellSpec for Apply.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CellSpec {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<VolumeAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub networks: Vec<NetworkAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advertise_services: Vec<String>,
    /// Proto enum name, e.g. `EXECUTION_MODE_OCI`. Empty = unspecified.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub execution_mode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub oci_bundle_path: String,
}

/// Profile reference.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub digest: String,
}

/// Resource limits.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSpec {
    pub vcpu: u32,
    /// Proto JSON encodes uint64 as a decimal string.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub memory_bytes: String,
}

impl ResourceSpec {
    /// Convenience constructor with memory as u64.
    pub fn new(vcpu: u32, memory_bytes: u64) -> Self {
        Self {
            vcpu,
            memory_bytes: memory_bytes.to_string(),
        }
    }
}

/// StartCell request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartCellRequest {
    pub cell_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub idempotency_key: String,
}

/// StartCell response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartCellResponse {
    #[serde(default)]
    pub operation: Option<Operation>,
}

/// Observed cell summary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub observed_state: String,
    #[serde(default)]
    pub desired_state: String,
}

/// Operation summary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    #[serde(default)]
    pub operation_id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub target_id: String,
    #[serde(default)]
    pub detail: String,
}

/// Guest `HydrateProjection` file entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HydrateFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
}

impl HydrateFile {
    /// UTF-8 text file (mirror `content` field).
    pub fn text(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: Some(content.into()),
            content_base64: None,
        }
    }

    /// Arbitrary bytes via base64 (mirror `content_base64`).
    pub fn from_bytes(path: impl Into<String>, bytes: impl AsRef<[u8]>) -> Self {
        use base64::Engine as _;
        Self {
            path: path.into(),
            content: None,
            content_base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes.as_ref())),
        }
    }
}

/// Guest hydrate request (`lattice.runtime.v1` / `cell.mirror.v1`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HydrateProjectionRequest {
    pub projection_id: String,
    pub files: Vec<HydrateFile>,
}

/// Guest hydrate response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct HydrateProjectionResponse {
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub projection_id: String,
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub file_count: u64,
    #[serde(default)]
    pub bytes: u64,
}

/// Guest `RunTask` request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunTaskRequest {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task_id: String,
    pub projection_id: String,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_sec: Option<u64>,
    /// When false, defer artifact retrieval to CollectOutput.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collect: Option<bool>,
}

/// Guest `RunTask` response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct RunTaskResponse {
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub exit_code: i32,
    #[serde(default)]
    pub projection_id: String,
    #[serde(default)]
    pub output_files: Vec<CollectedFile>,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    #[serde(default)]
    pub detail: String,
}

/// Guest `CollectOutput` request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CollectOutputRequest {
    pub projection_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_content: Option<bool>,
}

impl Default for CollectOutputRequest {
    fn default() -> Self {
        Self {
            projection_id: String::new(),
            prefix: "output".to_string(),
            include_content: Some(true),
        }
    }
}

/// Guest collect response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct CollectOutputResponse {
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub projection_id: String,
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub files: Vec<CollectedFile>,
    #[serde(default)]
    pub file_count: u64,
    #[serde(default)]
    pub bytes: u64,
}

/// One collected file from mirror / RunTask.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct CollectedFile {
    pub path: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub content_base64: String,
}

impl CollectedFile {
    /// Decode `content_base64` when present.
    pub fn content_bytes(&self) -> crate::Result<Vec<u8>> {
        use base64::Engine as _;
        if self.content_base64.is_empty() {
            return Ok(Vec::new());
        }
        base64::engine::general_purpose::STANDARD
            .decode(&self.content_base64)
            .map_err(|err| crate::CellClientError::Connect(format!("base64 decode: {err}")))
    }
}
