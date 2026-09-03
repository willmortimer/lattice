//! Phase A JSONL agent protocol (mirrors `@lattice/agent-protocol`).
//!
//! Wire shapes use camelCase field names and snake_case `type` discriminators so
//! Rust and the TypeScript package stay interchangeable over stdio.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Shared protocol version with `@lattice/agent-protocol`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Provider kind for `start_run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Pioneer,
    Openai,
    Local,
    Fake,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pioneer => "pioneer",
            Self::Openai => "openai",
            Self::Local => "local",
            Self::Fake => "fake",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pioneer" => Some(Self::Pioneer),
            "openai" => Some(Self::Openai),
            "local" => Some(Self::Local),
            "fake" => Some(Self::Fake),
            _ => None,
        }
    }
}

/// Opaque run identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentRunId(pub String);

impl AgentRunId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentRunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Commands written to agentd stdin (one JSON object per line).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AgentCommand {
    #[serde(rename = "hello")]
    Hello {
        #[serde(rename = "protocolVersion")]
        protocol_version: u32,
    },
    #[serde(rename = "start_run")]
    StartRun {
        #[serde(rename = "threadId")]
        thread_id: String,
        #[serde(rename = "runId")]
        run_id: String,
        provider: ProviderKind,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        messages: Option<Vec<Value>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        /// Open daemon session id for Lattice HTTP tools.
        #[serde(
            default,
            rename = "workspaceId",
            skip_serializing_if = "Option::is_none"
        )]
        workspace_id: Option<String>,
        /// Absolute workspace root when no session id is available.
        #[serde(
            default,
            rename = "workspaceRoot",
            skip_serializing_if = "Option::is_none"
        )]
        workspace_root: Option<String>,
    },
    #[serde(rename = "cancel_run")]
    CancelRun {
        #[serde(rename = "runId")]
        run_id: String,
    },
    #[serde(rename = "health")]
    Health,
    #[serde(rename = "shutdown")]
    Shutdown,
}

impl AgentCommand {
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }

    pub fn from_line(line: &str) -> Result<Self, ProtocolParseError> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Err(ProtocolParseError::Empty);
        }
        serde_json::from_str(trimmed).map_err(ProtocolParseError::Json)
    }
}

/// Events read from agentd stdout (one JSON object per line).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AgentEvent {
    #[serde(rename = "hello_ack")]
    HelloAck {
        #[serde(rename = "protocolVersion")]
        protocol_version: u32,
    },
    #[serde(rename = "run_started")]
    RunStarted {
        #[serde(rename = "runId")]
        run_id: String,
        #[serde(rename = "threadId")]
        thread_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider: Option<ProviderKind>,
    },
    #[serde(rename = "message_chunk")]
    MessageChunk {
        #[serde(rename = "runId")]
        run_id: String,
        chunk: Value,
    },
    #[serde(rename = "run_completed")]
    RunCompleted {
        #[serde(rename = "runId")]
        run_id: String,
    },
    #[serde(rename = "run_failed")]
    RunFailed {
        #[serde(rename = "runId")]
        run_id: String,
        message: String,
        retryable: bool,
    },
    #[serde(rename = "health")]
    Health { ok: bool },
    #[serde(rename = "step_started")]
    StepStarted {
        #[serde(rename = "runId")]
        run_id: String,
        #[serde(rename = "stepId")]
        step_id: String,
        kind: String,
        label: String,
    },
    #[serde(rename = "step_completed")]
    StepCompleted {
        #[serde(rename = "runId")]
        run_id: String,
        #[serde(rename = "stepId")]
        step_id: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    #[serde(rename = "evidence_added")]
    EvidenceAdded {
        #[serde(rename = "runId")]
        run_id: String,
        #[serde(rename = "evidenceId")]
        evidence_id: String,
        #[serde(rename = "resourceId")]
        resource_id: String,
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision: Option<String>,
        excerpt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        anchor: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        score: Option<f64>,
    },
    #[serde(rename = "overlay_show")]
    OverlayShow {
        #[serde(rename = "runId")]
        run_id: String,
        #[serde(rename = "overlayId")]
        overlay_id: String,
        anchors: Vec<Value>,
        purpose: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commentary: Option<String>,
    },
    #[serde(rename = "overlay_clear")]
    OverlayClear {
        #[serde(rename = "runId")]
        run_id: String,
        #[serde(rename = "overlayId", default, skip_serializing_if = "Option::is_none")]
        overlay_id: Option<String>,
    },
}

impl AgentEvent {
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }

    pub fn from_line(line: &str) -> Result<Self, ProtocolParseError> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Err(ProtocolParseError::Empty);
        }
        serde_json::from_str(trimmed).map_err(ProtocolParseError::Json)
    }

    pub fn run_id(&self) -> Option<&str> {
        match self {
            Self::HelloAck { .. } | Self::Health { .. } => None,
            Self::RunStarted { run_id, .. }
            | Self::MessageChunk { run_id, .. }
            | Self::RunCompleted { run_id }
            | Self::RunFailed { run_id, .. }
            | Self::StepStarted { run_id, .. }
            | Self::StepCompleted { run_id, .. }
            | Self::EvidenceAdded { run_id, .. }
            | Self::OverlayShow { run_id, .. }
            | Self::OverlayClear { run_id, .. } => Some(run_id.as_str()),
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            Self::HelloAck { .. } => "hello_ack",
            Self::RunStarted { .. } => "run_started",
            Self::MessageChunk { .. } => "message_chunk",
            Self::RunCompleted { .. } => "run_completed",
            Self::RunFailed { .. } => "run_failed",
            Self::Health { .. } => "health",
            Self::StepStarted { .. } => "step_started",
            Self::StepCompleted { .. } => "step_completed",
            Self::EvidenceAdded { .. } => "evidence_added",
            Self::OverlayShow { .. } => "overlay_show",
            Self::OverlayClear { .. } => "overlay_clear",
        }
    }
}

/// Daemon-facing start-run request (maps onto the JSONL `start_run` command).
#[derive(Debug, Clone)]
pub struct StartAgentRunRequest {
    pub thread_id: String,
    pub run_id: AgentRunId,
    pub provider: ProviderKind,
    pub model: String,
    pub messages: Option<Vec<Value>>,
    pub prompt: Option<String>,
    /// Workspace id for event fan-out and Lattice HTTP tool binding.
    pub workspace_id: String,
    /// Absolute workspace root passed through to agentd tools when set.
    pub workspace_root: Option<String>,
}

impl StartAgentRunRequest {
    pub fn validate(&self) -> Result<(), AgentRuntimeError> {
        if self.messages.is_none() && self.prompt.is_none() {
            return Err(AgentRuntimeError::InvalidRequest(
                "start_run requires messages or prompt".into(),
            ));
        }
        if self.thread_id.is_empty() {
            return Err(AgentRuntimeError::InvalidRequest(
                "thread_id is required".into(),
            ));
        }
        if self.run_id.as_str().is_empty() {
            return Err(AgentRuntimeError::InvalidRequest(
                "run_id is required".into(),
            ));
        }
        Ok(())
    }

    pub fn to_command(&self) -> AgentCommand {
        let workspace_id = if self.workspace_id.is_empty() {
            None
        } else {
            Some(self.workspace_id.clone())
        };
        AgentCommand::StartRun {
            thread_id: self.thread_id.clone(),
            run_id: self.run_id.0.clone(),
            provider: self.provider,
            model: self.model.clone(),
            messages: self.messages.clone(),
            prompt: self.prompt.clone(),
            workspace_id,
            workspace_root: self.workspace_root.clone(),
        }
    }
}

/// Handle returned after a run is accepted by the backend.
#[derive(Debug, Clone)]
pub struct AgentRunHandle {
    pub run_id: AgentRunId,
    pub thread_id: String,
}

/// Backend health snapshot.
#[derive(Debug, Clone)]
pub struct AgentRuntimeHealth {
    pub ok: bool,
    pub backend: String,
    pub degraded: bool,
}

/// Errors from the agent runtime backends / controller.
#[derive(Debug, Error)]
pub enum AgentRuntimeError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("agent unavailable: {0}")]
    Unavailable(String),
    #[error("agent protocol error: {0}")]
    Protocol(String),
    #[error("agent spawn failed: {0}")]
    Spawn(String),
    #[error("agent run not found: {0}")]
    RunNotFound(String),
    #[error("agent io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("agent json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

impl AgentRuntimeError {
    pub fn wire_code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "agent_invalid_request",
            Self::Unavailable(_) => "agent_unavailable",
            Self::Protocol(_) => "agent_protocol_error",
            Self::Spawn(_) => "agent_spawn_failed",
            Self::RunNotFound(_) => "agent_run_not_found",
            Self::Io(_) => "agent_io_error",
            Self::Json(_) => "agent_json_error",
            Self::Message(_) => "agent_error",
        }
    }
}

#[derive(Debug, Error)]
pub enum ProtocolParseError {
    #[error("line is empty")]
    Empty,
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_round_trips_match_ts_shapes() {
        let hello = AgentCommand::Hello {
            protocol_version: PROTOCOL_VERSION,
        };
        let line = hello.to_line().expect("encode");
        assert!(line.ends_with('\n'));
        assert_eq!(line.trim(), r#"{"type":"hello","protocolVersion":1}"#);
        assert_eq!(AgentCommand::from_line(&line).expect("decode"), hello);

        let start = AgentCommand::StartRun {
            thread_id: "t1".into(),
            run_id: "r1".into(),
            provider: ProviderKind::Pioneer,
            model: "gpt-test".into(),
            messages: Some(vec![serde_json::json!({
                "id": "m1",
                "role": "user",
                "content": "hello"
            })]),
            prompt: None,
            workspace_id: Some("ws-1".into()),
            workspace_root: None,
        };
        let parsed = AgentCommand::from_line(&start.to_line().unwrap()).unwrap();
        assert_eq!(parsed, start);
    }

    #[test]
    fn event_round_trips_message_chunk() {
        let event = AgentEvent::MessageChunk {
            run_id: "r1".into(),
            chunk: serde_json::json!({"type":"text-delta","id":"m1","delta":"hi"}),
        };
        let parsed = AgentEvent::from_line(&event.to_line().unwrap()).unwrap();
        assert_eq!(parsed, event);
        assert_eq!(parsed.event_type(), "message_chunk");
    }

    #[test]
    fn event_round_trips_spatial_overlay_show() {
        let event = AgentEvent::OverlayShow {
            run_id: "r1".into(),
            overlay_id: "ov-1".into(),
            anchors: vec![serde_json::json!({
                "kind": "markdown-block",
                "resourceId": "page:notes",
                "blockId": "blk-2"
            })],
            purpose: "attention".into(),
            commentary: Some("Focus".into()),
        };
        let parsed = AgentEvent::from_line(&event.to_line().unwrap()).unwrap();
        assert_eq!(parsed, event);
        assert_eq!(parsed.event_type(), "overlay_show");
        assert_eq!(parsed.run_id(), Some("r1"));
    }
}
