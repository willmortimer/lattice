use serde::{Deserialize, Serialize};

/// Model installation and runtime lifecycle for an embedding provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddingInstallState {
    NotInstalled,
    Loading,
    Ready,
    Degraded,
    Failed,
}

/// Per-chunk embedding index lifecycle within one namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChunkEmbeddingStatus {
    Pending,
    Ready,
    Failed,
    Stale,
}

impl ChunkEmbeddingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Stale => "stale",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "ready" => Some(Self::Ready),
            "failed" => Some(Self::Failed),
            "stale" => Some(Self::Stale),
            _ => None,
        }
    }
}
