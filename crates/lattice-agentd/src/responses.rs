//! OpenAI Responses API streaming client (ADR 0051 / 0066).
//!
//! Posts to `{base}/responses` with `stream: true`, parses SSE events, and maps
//! text deltas into AI SDK–style UI chunks (`text-start` / `text-delta` /
//! `text-end`) carried by agent-protocol `message_chunk` events.
//!
//! No Wasmtime / tools in this slice — text streaming only.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::protocol::{AgentEvent, ProviderKind};

/// Default OpenAI API root (includes `/v1`).
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// Default model when `start_run.model` is empty (matches Node agentd openai path).
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4.1-mini";

/// Errors from the OpenAI Responses client.
#[derive(Debug, Error)]
pub enum ResponsesError {
    #[error("OPENAI_API_KEY is required for provider=openai")]
    MissingApiKey,
    #[error("Run cancelled")]
    Cancelled,
    #[error("openai HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("openai API error: {0}")]
    Api(String),
    #[error("openai transport error: {0}")]
    Transport(String),
    #[error("openai stream ended without completion")]
    Incomplete,
}

impl PartialEq for ResponsesError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MissingApiKey, Self::MissingApiKey)
            | (Self::Cancelled, Self::Cancelled)
            | (Self::Incomplete, Self::Incomplete) => true,
            (Self::Http { status: a, body: ab }, Self::Http { status: b, body: bb }) => {
                a == b && ab == bb
            }
            (Self::Api(a), Self::Api(b)) | (Self::Transport(a), Self::Transport(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ResponsesError {}

/// Options for a live (or fixture-backed) OpenAI Responses run.
#[derive(Debug, Clone)]
pub struct OpenaiRunOptions {
    pub run_id: String,
    pub thread_id: String,
    pub model: String,
    pub prompt: String,
    pub api_key: String,
    /// API root including `/v1` (overridable for wiremock / proxies).
    pub base_url: String,
    pub cancel: Arc<AtomicBool>,
}

/// Resolve `OPENAI_API_KEY` from the process environment.
pub fn api_key_from_env() -> Option<String> {
    std::env::var("OPENAI_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// Resolve Responses API base URL: `OPENAI_BASE_URL` or the public default.
pub fn base_url_from_env() -> String {
    std::env::var("OPENAI_BASE_URL")
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string())
}

pub fn normalize_model(model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        DEFAULT_OPENAI_MODEL.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Emit `run_started` → streamed `message_chunk`(s) → `run_completed` / `run_failed`.
pub async fn emit_openai_run(options: OpenaiRunOptions, events: mpsc::Sender<AgentEvent>) {
    let OpenaiRunOptions {
        run_id,
        thread_id,
        model,
        prompt,
        api_key,
        base_url,
        cancel,
    } = options;

    let send = |event: AgentEvent| {
        let tx = events.clone();
        async move {
            let _ = tx.send(event).await;
        }
    };

    send(AgentEvent::RunStarted {
        run_id: run_id.clone(),
        thread_id,
        provider: Some(ProviderKind::Openai),
    })
    .await;

    if api_key.trim().is_empty() {
        send(AgentEvent::RunFailed {
            run_id,
            message: ResponsesError::MissingApiKey.to_string(),
            retryable: false,
        })
        .await;
        return;
    }

    if cancel.load(Ordering::SeqCst) {
        send(AgentEvent::RunFailed {
            run_id,
            message: ResponsesError::Cancelled.to_string(),
            retryable: false,
        })
        .await;
        return;
    }

    match stream_responses_to_events(
        &api_key,
        &base_url,
        &normalize_model(&model),
        &prompt,
        &run_id,
        &cancel,
        &events,
    )
    .await
    {
        Ok(()) => {
            if cancel.load(Ordering::SeqCst) {
                send(AgentEvent::RunFailed {
                    run_id,
                    message: ResponsesError::Cancelled.to_string(),
                    retryable: false,
                })
                .await;
            } else {
                send(AgentEvent::RunCompleted { run_id }).await;
            }
        }
        Err(ResponsesError::Cancelled) => {
            send(AgentEvent::RunFailed {
                run_id,
                message: ResponsesError::Cancelled.to_string(),
                retryable: false,
            })
            .await;
        }
        Err(err) => {
            let retryable = matches!(err, ResponsesError::Transport(_) | ResponsesError::Http { .. });
            send(AgentEvent::RunFailed {
                run_id,
                message: err.to_string(),
                retryable,
            })
            .await;
        }
    }
}

async fn stream_responses_to_events(
    api_key: &str,
    base_url: &str,
    model: &str,
    prompt: &str,
    run_id: &str,
    cancel: &AtomicBool,
    events: &mpsc::Sender<AgentEvent>,
) -> Result<(), ResponsesError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| ResponsesError::Transport(err.to_string()))?;

    let url = format!("{}/responses", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "input": prompt,
        "stream": true,
    });

    let request = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {api_key}"))
        .header(CONTENT_TYPE, "application/json")
        .json(&body);

    let response = tokio::select! {
        biased;
        _ = wait_cancelled(cancel) => {
            return Err(ResponsesError::Cancelled);
        }
        result = request.send() => {
            result.map_err(|err| {
                if cancel.load(Ordering::SeqCst) {
                    ResponsesError::Cancelled
                } else {
                    ResponsesError::Transport(err.to_string())
                }
            })?
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| String::new());
        let truncated = truncate_for_event(&body, 512);
        return Err(ResponsesError::Http {
            status: status.as_u16(),
            body: truncated,
        });
    }

    let mut byte_stream = response.bytes_stream();
    let mut line_buf = String::new();
    let mut mapper = UiChunkMapper::new(run_id.to_string());
    let mut saw_completed = false;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(ResponsesError::Cancelled);
        }

        let next = tokio::select! {
            biased;
            _ = wait_cancelled(cancel) => {
                return Err(ResponsesError::Cancelled);
            }
            chunk = byte_stream.next() => chunk,
        };

        match next {
            None => break,
            Some(Err(err)) => {
                if cancel.load(Ordering::SeqCst) {
                    return Err(ResponsesError::Cancelled);
                }
                return Err(ResponsesError::Transport(err.to_string()));
            }
            Some(Ok(bytes)) => {
                let text = String::from_utf8_lossy(&bytes);
                line_buf.push_str(&text);
                while let Some(idx) = line_buf.find('\n') {
                    let mut line = line_buf[..idx].to_string();
                    line_buf.drain(..=idx);
                    if line.ends_with('\r') {
                        line.pop();
                    }
                    if let Some(outcome) = handle_sse_line(&line, &mut mapper, events).await? {
                        match outcome {
                            StreamOutcome::Completed => saw_completed = true,
                            StreamOutcome::Failed(msg) => {
                                return Err(ResponsesError::Api(msg));
                            }
                        }
                    }
                }
            }
        }
    }

    // Flush a trailing line without newline (some servers omit the final \n).
    if !line_buf.trim().is_empty() {
        if let Some(outcome) = handle_sse_line(&line_buf, &mut mapper, events).await? {
            match outcome {
                StreamOutcome::Completed => saw_completed = true,
                StreamOutcome::Failed(msg) => return Err(ResponsesError::Api(msg)),
            }
        }
    }

    mapper.finish(events).await?;

    if saw_completed {
        Ok(())
    } else if cancel.load(Ordering::SeqCst) {
        Err(ResponsesError::Cancelled)
    } else {
        Err(ResponsesError::Incomplete)
    }
}

enum StreamOutcome {
    Completed,
    Failed(String),
}

async fn handle_sse_line(
    line: &str,
    mapper: &mut UiChunkMapper,
    events: &mpsc::Sender<AgentEvent>,
) -> Result<Option<StreamOutcome>, ResponsesError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(':') {
        return Ok(None);
    }
    // OpenAI may emit `event:` lines; we key off JSON `type` in `data:`.
    if trimmed.starts_with("event:") {
        return Ok(None);
    }
    let data = if let Some(rest) = trimmed.strip_prefix("data:") {
        rest.trim()
    } else {
        // Tolerate bare JSON lines in fixtures.
        trimmed
    };
    if data.is_empty() || data == "[DONE]" {
        return Ok(None);
    }

    let value: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(err) => {
            warn!(error = %err, line = %data, "skipping unparseable SSE data");
            return Ok(None);
        }
    };

    mapper.apply(&value, events).await
}

/// Maps Responses SSE JSON objects into UI message chunks.
#[derive(Debug)]
struct UiChunkMapper {
    run_id: String,
    message_id: Option<String>,
    text_started: bool,
    text_ended: bool,
}

impl UiChunkMapper {
    fn new(run_id: String) -> Self {
        Self {
            run_id,
            message_id: None,
            text_started: false,
            text_ended: false,
        }
    }

    async fn apply(
        &mut self,
        value: &Value,
        events: &mpsc::Sender<AgentEvent>,
    ) -> Result<Option<StreamOutcome>, ResponsesError> {
        let Some(event_type) = value.get("type").and_then(|v| v.as_str()) else {
            return Ok(None);
        };

        match event_type {
            "response.output_text.delta" => {
                let delta = value
                    .get("delta")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if delta.is_empty() {
                    return Ok(None);
                }
                self.ensure_started(value, events).await?;
                let id = self.message_id.clone().unwrap_or_else(|| "openai-msg".into());
                self.send_chunk(
                    events,
                    json!({ "type": "text-delta", "id": id, "delta": delta }),
                )
                .await?;
                Ok(None)
            }
            "response.output_text.done" => {
                self.ensure_started(value, events).await?;
                self.ensure_ended(events).await?;
                Ok(None)
            }
            "response.completed" => {
                self.ensure_ended(events).await?;
                Ok(Some(StreamOutcome::Completed))
            }
            "response.failed" => {
                let msg = value
                    .pointer("/response/error/message")
                    .and_then(|v| v.as_str())
                    .or_else(|| value.get("message").and_then(|v| v.as_str()))
                    .unwrap_or("response.failed");
                Ok(Some(StreamOutcome::Failed(msg.to_string())))
            }
            "error" => {
                let msg = value
                    .get("message")
                    .and_then(|v| v.as_str())
                    .or_else(|| value.pointer("/error/message").and_then(|v| v.as_str()))
                    .unwrap_or("openai stream error");
                Ok(Some(StreamOutcome::Failed(msg.to_string())))
            }
            other => {
                debug!(event_type = other, "ignoring Responses SSE event");
                Ok(None)
            }
        }
    }

    async fn ensure_started(
        &mut self,
        value: &Value,
        events: &mpsc::Sender<AgentEvent>,
    ) -> Result<(), ResponsesError> {
        if self.text_started {
            return Ok(());
        }
        let id = value
            .get("item_id")
            .and_then(|v| v.as_str())
            .unwrap_or("openai-msg")
            .to_string();
        self.message_id = Some(id.clone());
        self.text_started = true;
        self.send_chunk(events, json!({ "type": "text-start", "id": id }))
            .await
    }

    async fn ensure_ended(
        &mut self,
        events: &mpsc::Sender<AgentEvent>,
    ) -> Result<(), ResponsesError> {
        if !self.text_started || self.text_ended {
            return Ok(());
        }
        let id = self
            .message_id
            .clone()
            .unwrap_or_else(|| "openai-msg".into());
        self.text_ended = true;
        self.send_chunk(events, json!({ "type": "text-end", "id": id }))
            .await
    }

    async fn finish(&mut self, events: &mpsc::Sender<AgentEvent>) -> Result<(), ResponsesError> {
        self.ensure_ended(events).await
    }

    async fn send_chunk(
        &self,
        events: &mpsc::Sender<AgentEvent>,
        chunk: Value,
    ) -> Result<(), ResponsesError> {
        events
            .send(AgentEvent::MessageChunk {
                run_id: self.run_id.clone(),
                chunk,
            })
            .await
            .map_err(|_| ResponsesError::Transport("event channel closed".into()))
    }
}

async fn wait_cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn truncate_for_event(body: &str, max: usize) -> String {
    let trimmed = body.trim();
    if trimmed.len() <= max {
        return trimmed.to_string();
    }
    format!("{}…", &trimmed[..max])
}

/// Parse a recorded SSE body into UI chunks (fixture helper / unit tests).
pub async fn map_sse_fixture_to_chunks(sse: &str, run_id: &str) -> Result<Vec<Value>, ResponsesError> {
    let (tx, mut rx) = mpsc::channel(64);
    let mut mapper = UiChunkMapper::new(run_id.to_string());
    let mut saw_completed = false;
    for line in sse.lines() {
        if let Some(outcome) = handle_sse_line(line, &mut mapper, &tx).await? {
            match outcome {
                StreamOutcome::Completed => saw_completed = true,
                StreamOutcome::Failed(msg) => return Err(ResponsesError::Api(msg)),
            }
        }
    }
    mapper.finish(&tx).await?;
    drop(tx);
    let mut chunks = Vec::new();
    while let Some(event) = rx.recv().await {
        if let AgentEvent::MessageChunk { chunk, .. } = event {
            chunks.push(chunk);
        }
    }
    if !saw_completed {
        return Err(ResponsesError::Incomplete);
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"data: {"type":"response.created","response":{"id":"resp_test"}}

data: {"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"content_index":0,"delta":"Hello","sequence_number":1}

data: {"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"content_index":0,"delta":" world","sequence_number":2}

data: {"type":"response.output_text.done","item_id":"msg_1","output_index":0,"content_index":0,"text":"Hello world","sequence_number":3}

data: {"type":"response.completed","response":{"id":"resp_test","status":"completed"}}

data: [DONE]
"#;

    #[tokio::test]
    async fn fixture_maps_to_ui_text_chunks() {
        let chunks = map_sse_fixture_to_chunks(FIXTURE, "r1")
            .await
            .expect("map fixture");
        assert_eq!(
            chunks[0],
            json!({"type":"text-start","id":"msg_1"})
        );
        assert_eq!(
            chunks[1],
            json!({"type":"text-delta","id":"msg_1","delta":"Hello"})
        );
        assert_eq!(
            chunks[2],
            json!({"type":"text-delta","id":"msg_1","delta":" world"})
        );
        assert_eq!(chunks[3], json!({"type":"text-end","id":"msg_1"}));
        assert_eq!(chunks.len(), 4);
    }

    #[tokio::test]
    async fn missing_api_key_fails_fast() {
        let (tx, mut rx) = mpsc::channel(8);
        emit_openai_run(
            OpenaiRunOptions {
                run_id: "r-missing".into(),
                thread_id: "t1".into(),
                model: "gpt-test".into(),
                prompt: "hi".into(),
                api_key: String::new(),
                base_url: "http://127.0.0.1:9".into(),
                cancel: Arc::new(AtomicBool::new(false)),
            },
            tx,
        )
        .await;

        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            let terminal = matches!(
                event,
                AgentEvent::RunCompleted { .. } | AgentEvent::RunFailed { .. }
            );
            events.push(event);
            if terminal {
                break;
            }
        }
        assert!(matches!(
            events.first(),
            Some(AgentEvent::RunStarted {
                provider: Some(ProviderKind::Openai),
                ..
            })
        ));
        assert!(matches!(
            events.last(),
            Some(AgentEvent::RunFailed { message, retryable: false, .. })
                if message.contains("OPENAI_API_KEY")
        ));
    }

    #[test]
    fn normalize_model_defaults() {
        assert_eq!(normalize_model(""), DEFAULT_OPENAI_MODEL);
        assert_eq!(normalize_model("  gpt-4o  "), "gpt-4o");
    }
}
