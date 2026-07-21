//! Stdio JSON-lines request/response shapes for the ipykernel bridge.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request written to the bridge stdin (one JSON object per line).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeRequest {
    Execute { id: String, code: String },
    Interrupt { id: String },
    Shutdown { id: String },
}

impl BridgeRequest {
    pub fn id(&self) -> &str {
        match self {
            Self::Execute { id, .. }
            | Self::Interrupt { id }
            | Self::Shutdown { id } => id,
        }
    }

    /// Encode as a single newline-terminated JSON-lines frame.
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }
}

/// Response read from the bridge stdout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeResponse {
    Ready,
    Stream {
        id: String,
        name: String,
        text: String,
    },
    ExecuteResult {
        id: String,
        data: HashMap<String, String>,
    },
    Error {
        id: String,
        ename: String,
        evalue: String,
        traceback: Vec<String>,
    },
    Done {
        id: String,
        status: String,
    },
    BridgeError {
        #[serde(default)]
        id: Option<String>,
        message: String,
    },
}

impl BridgeResponse {
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Ready => None,
            Self::Stream { id, .. }
            | Self::ExecuteResult { id, .. }
            | Self::Error { id, .. }
            | Self::Done { id, .. } => Some(id.as_str()),
            Self::BridgeError { id, .. } => id.as_deref(),
        }
    }

    /// Parse one JSON object (without the trailing newline).
    pub fn from_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim())
    }
}

/// Jupyter-shaped outputs collected from one `execute` until `done`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KernelOutput {
    Stream {
        name: String,
        text: String,
    },
    ExecuteResult {
        data: HashMap<String, String>,
    },
    Error {
        ename: String,
        evalue: String,
        traceback: Vec<String>,
    },
}

/// Result of [`crate::KernelSessionMap::execute`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteResult {
    pub request_id: String,
    /// Final status from the bridge `done` message (`ok`, `error`, …).
    pub status: String,
    pub outputs: Vec<KernelOutput>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_frames_are_newline_delimited_json() {
        let req = BridgeRequest::Execute {
            id: "r1".into(),
            code: "print(1)".into(),
        };
        let line = req.to_line().expect("encode");
        assert!(line.ends_with('\n'));
        assert!(!line[..line.len() - 1].contains('\n'));
        let parsed: BridgeRequest = serde_json::from_str(line.trim()).expect("decode");
        assert_eq!(parsed, req);
    }

    #[test]
    fn response_round_trips_stream_and_error() {
        let stream = r#"{"type":"stream","id":"r1","name":"stdout","text":"hi\n"}"#;
        let parsed = BridgeResponse::from_line(stream).expect("stream");
        assert!(matches!(
            parsed,
            BridgeResponse::Stream {
                name,
                text,
                ..
            } if name == "stdout" && text == "hi\n"
        ));

        let err = r#"{"type":"error","id":"r1","ename":"ValueError","evalue":"x","traceback":["t"]}"#;
        let parsed = BridgeResponse::from_line(err).expect("error");
        match parsed {
            BridgeResponse::Error {
                ename,
                evalue,
                traceback,
                ..
            } => {
                assert_eq!(ename, "ValueError");
                assert_eq!(evalue, "x");
                assert_eq!(traceback, vec!["t"]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn execute_result_serializes_camel_case() {
        let result = ExecuteResult {
            request_id: "r1".into(),
            status: "ok".into(),
            outputs: vec![KernelOutput::Stream {
                name: "stdout".into(),
                text: "1\n".into(),
            }],
        };
        let json = serde_json::to_value(&result).expect("ser");
        assert_eq!(json["requestId"], "r1");
        assert!(json.get("request_id").is_none());
    }
}
