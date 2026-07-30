//! Phase A JSONL agent protocol (mirrors `apps/daemon/src/agent/protocol.rs`
//! and `@lattice/agent-protocol`).
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
        #[serde(default, rename = "workspaceId", skip_serializing_if = "Option::is_none")]
        workspace_id: Option<String>,
        #[serde(default, rename = "workspaceRoot", skip_serializing_if = "Option::is_none")]
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
    fn golden_hello_command_line() {
        let hello = AgentCommand::Hello {
            protocol_version: PROTOCOL_VERSION,
        };
        let line = hello.to_line().expect("encode");
        assert_eq!(line, "{\"type\":\"hello\",\"protocolVersion\":1}\n");
        assert_eq!(AgentCommand::from_line(&line).expect("decode"), hello);
    }

    #[test]
    fn golden_hello_ack_event_line() {
        let ack = AgentEvent::HelloAck {
            protocol_version: PROTOCOL_VERSION,
        };
        let line = ack.to_line().expect("encode");
        assert_eq!(line, "{\"type\":\"hello_ack\",\"protocolVersion\":1}\n");
        assert_eq!(AgentEvent::from_line(&line).expect("decode"), ack);
    }

    #[test]
    fn golden_start_run_and_message_chunk() {
        let start = AgentCommand::StartRun {
            thread_id: "t1".into(),
            run_id: "r1".into(),
            provider: ProviderKind::Fake,
            model: "fake-model".into(),
            messages: None,
            prompt: Some("hi".into()),
            workspace_id: Some("ws-1".into()),
            workspace_root: None,
        };
        let line = start.to_line().expect("encode");
        assert!(line.contains("\"type\":\"start_run\""));
        assert!(line.contains("\"threadId\":\"t1\""));
        assert!(line.contains("\"runId\":\"r1\""));
        assert!(line.contains("\"workspaceId\":\"ws-1\""));
        assert_eq!(AgentCommand::from_line(&line).expect("decode"), start);

        let event = AgentEvent::MessageChunk {
            run_id: "r1".into(),
            chunk: serde_json::json!({"type":"text-delta","id":"m1","delta":"hi"}),
        };
        let parsed = AgentEvent::from_line(&event.to_line().unwrap()).unwrap();
        assert_eq!(parsed, event);
        assert_eq!(parsed.event_type(), "message_chunk");
    }

    #[test]
    fn health_and_shutdown_round_trip() {
        assert_eq!(
            AgentCommand::from_line(r#"{"type":"health"}"#).unwrap(),
            AgentCommand::Health
        );
        assert_eq!(
            AgentCommand::from_line(r#"{"type":"shutdown"}"#).unwrap(),
            AgentCommand::Shutdown
        );
        assert_eq!(
            AgentEvent::from_line(r#"{"type":"health","ok":true}"#)
                .unwrap()
                .event_type(),
            "health"
        );
    }
}
