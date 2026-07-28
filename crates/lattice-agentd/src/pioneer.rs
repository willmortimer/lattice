//! Pioneer OpenAI-compatible Chat Completions streaming + host tool loop.
//!
//! Matches Node `apps/agentd` pioneer path: `https://api.pioneer.ai/v1` +
//! `PIONEER_API_KEY`, chat completions (not Responses). When a Lattice HTTP
//! client is configured, runs a thin tool loop (max 8 rounds); otherwise
//! streams chat-only text into AI SDK UI chunks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::warn;

use crate::lattice_client::LatticeToolClient;
use crate::protocol::{AgentEvent, ProviderKind};
use crate::tools::{
    dispatch_tool, openai_tool_definitions, ToolRunContext, WORKSPACE_AGENT_INSTRUCTIONS,
};

pub const DEFAULT_PIONEER_BASE_URL: &str = "https://api.pioneer.ai/v1";
/// Cheap default for local testing (Pioneer catalog).
pub const DEFAULT_PIONEER_MODEL: &str = "gpt-5.6-luna";

/// Max assistant→tool→assistant rounds when Lattice tools are enabled.
pub const MAX_TOOL_ROUNDS: usize = 8;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PioneerError {
    #[error("PIONEER_API_KEY is required for provider=pioneer")]
    MissingApiKey,
    #[error("Run cancelled")]
    Cancelled,
    #[error("pioneer HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("pioneer API error: {0}")]
    Api(String),
    #[error("pioneer transport error: {0}")]
    Transport(String),
    #[error("pioneer stream ended without completion")]
    Incomplete,
    #[error("pioneer tool loop exceeded {0} rounds")]
    ToolLoopExhausted(usize),
}

#[derive(Debug, Clone)]
pub struct PioneerRunOptions {
    pub run_id: String,
    pub thread_id: String,
    pub model: String,
    /// Flattened fallback prompt (used when `messages` is empty).
    pub prompt: String,
    /// OpenAI-style chat turns (user/assistant). When non-empty, preferred over `prompt`.
    pub messages: Vec<serde_json::Value>,
    pub api_key: String,
    pub base_url: String,
    pub cancel: Arc<AtomicBool>,
    /// When set, enable Chat Completions tool loop against latticed HTTP.
    pub lattice: Option<LatticeToolClient>,
    pub workspace_id: Option<String>,
    pub workspace_root: Option<String>,
}

pub fn api_key_from_env() -> Option<String> {
    std::env::var("PIONEER_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

pub fn base_url_from_env() -> String {
    std::env::var("PIONEER_BASE_URL")
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_PIONEER_BASE_URL.to_string())
}

pub fn normalize_model(model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.is_empty() || trimmed == "default" {
        DEFAULT_PIONEER_MODEL.to_string()
    } else {
        trimmed.to_string()
    }
}

pub async fn emit_pioneer_run(options: PioneerRunOptions, events: mpsc::Sender<AgentEvent>) {
    let PioneerRunOptions {
        run_id,
        thread_id,
        model,
        prompt,
        messages,
        api_key,
        base_url,
        cancel,
        lattice,
        workspace_id,
        workspace_root,
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
        provider: Some(ProviderKind::Pioneer),
    })
    .await;

    if api_key.trim().is_empty() {
        send(AgentEvent::RunFailed {
            run_id,
            message: PioneerError::MissingApiKey.to_string(),
            retryable: false,
        })
        .await;
        return;
    }

    if cancel.load(Ordering::SeqCst) {
        send(AgentEvent::RunFailed {
            run_id,
            message: PioneerError::Cancelled.to_string(),
            retryable: false,
        })
        .await;
        return;
    }

    let model = normalize_model(&model);
    let chat_messages = if messages.is_empty() {
        vec![json!({ "role": "user", "content": prompt })]
    } else {
        messages
    };
    let result = if lattice.is_some() {
        run_tool_loop(
            &api_key,
            &base_url,
            &model,
            &chat_messages,
            &run_id,
            &cancel,
            &events,
            lattice.as_ref(),
            &ToolRunContext {
                workspace_id,
                workspace_root,
            },
        )
        .await
    } else {
        let prompt_text = chat_messages
            .iter()
            .rev()
            .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
            .and_then(|m| m.get("content").and_then(|c| c.as_str()))
            .unwrap_or(prompt.as_str());
        stream_chat_completions(
            &api_key,
            &base_url,
            &model,
            prompt_text,
            &run_id,
            &cancel,
            &events,
        )
        .await
    };

    match result {
        Ok(()) => {
            if cancel.load(Ordering::SeqCst) {
                send(AgentEvent::RunFailed {
                    run_id,
                    message: PioneerError::Cancelled.to_string(),
                    retryable: false,
                })
                .await;
            } else {
                send(AgentEvent::RunCompleted { run_id }).await;
            }
        }
        Err(PioneerError::Cancelled) => {
            send(AgentEvent::RunFailed {
                run_id,
                message: PioneerError::Cancelled.to_string(),
                retryable: false,
            })
            .await;
        }
        Err(err) => {
            let retryable = matches!(err, PioneerError::Transport(_) | PioneerError::Http { .. });
            send(AgentEvent::RunFailed {
                run_id,
                message: err.to_string(),
                retryable,
            })
            .await;
        }
    }
}

async fn run_tool_loop(
    api_key: &str,
    base_url: &str,
    model: &str,
    chat_messages: &[Value],
    run_id: &str,
    cancel: &AtomicBool,
    events: &mpsc::Sender<AgentEvent>,
    lattice: Option<&LatticeToolClient>,
    tool_ctx: &ToolRunContext,
) -> Result<(), PioneerError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| PioneerError::Transport(err.to_string()))?;

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let tools = openai_tool_definitions();
    let system = format!(
        "{}{}",
        WORKSPACE_AGENT_INSTRUCTIONS,
        tool_ctx.binding_instructions()
    );
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(chat_messages.iter().cloned());

    for round in 0..MAX_TOOL_ROUNDS {
        if cancel.load(Ordering::SeqCst) {
            return Err(PioneerError::Cancelled);
        }

        // Non-streaming completions leave the UI silent for seconds; trail steps
        // show that the run is alive while waiting on Pioneer / tools.
        let think_id = format!("think-{round}");
        let think_started = std::time::Instant::now();
        emit_step_started(
            run_id,
            &think_id,
            "model",
            if round == 0 {
                "Checking the workspace…"
            } else {
                "Continuing with tool results…"
            },
            events,
        )
        .await;

        let body = json!({
            "model": model,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto",
            "stream": false,
        });

        let payload = post_chat_json(&client, &url, api_key, &body, cancel).await?;
        emit_step_completed(
            run_id,
            &think_id,
            think_started.elapsed().as_millis() as u64,
            None,
            events,
        )
        .await;

        if let Some(err) = payload.get("error") {
            let message = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("pioneer API error");
            return Err(PioneerError::Api(message.to_string()));
        }

        let choice = payload
            .pointer("/choices/0")
            .cloned()
            .ok_or_else(|| PioneerError::Api("missing choices[0]".into()))?;
        let message = choice
            .get("message")
            .cloned()
            .ok_or_else(|| PioneerError::Api("missing message".into()))?;

        let tool_calls = message
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if tool_calls.is_empty() {
            let content = message
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            emit_text_content_streamed(run_id, content, events).await;
            return Ok(());
        }

        // Keep the assistant turn (including tool_calls) in history.
        messages.push(message);

        for (tool_idx, call) in tool_calls.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Err(PioneerError::Cancelled);
            }
            let call_id = call
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("call_unknown");
            let name = call
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = call
                .pointer("/function/arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("{}");
            let step_id = format!("tool-{round}-{tool_idx}");
            let label = if name.is_empty() {
                "Running tool…".to_string()
            } else {
                format!("Running `{name}`…")
            };
            let tool_started = std::time::Instant::now();
            emit_step_started(run_id, &step_id, "tool", &label, events).await;
            let content = dispatch_tool(lattice, tool_ctx, name, arguments).await;
            emit_step_completed(
                run_id,
                &step_id,
                tool_started.elapsed().as_millis() as u64,
                Some(name),
                events,
            )
            .await;
            messages.push(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": content,
            }));
        }
    }

    Err(PioneerError::ToolLoopExhausted(MAX_TOOL_ROUNDS))
}

async fn post_chat_json(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    body: &Value,
    cancel: &AtomicBool,
) -> Result<Value, PioneerError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|err| PioneerError::Transport(err.to_string()))?,
    );
    if let Ok(value) = HeaderValue::from_str(api_key) {
        headers.insert("X-API-Key", value);
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let response = tokio::select! {
        biased;
        _ = wait_cancelled(cancel) => return Err(PioneerError::Cancelled),
        result = client.post(url).headers(headers).json(body).send() => {
            result.map_err(|err| PioneerError::Transport(err.to_string()))?
        }
    };

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| PioneerError::Transport(err.to_string()))?;
    if !status.is_success() {
        return Err(PioneerError::Http {
            status: status.as_u16(),
            body: truncate(&text, 512),
        });
    }
    serde_json::from_str(&text).map_err(|err| PioneerError::Api(err.to_string()))
}

async fn emit_step_started(
    run_id: &str,
    step_id: &str,
    kind: &str,
    label: &str,
    events: &mpsc::Sender<AgentEvent>,
) {
    let _ = events
        .send(AgentEvent::StepStarted {
            run_id: run_id.to_string(),
            step_id: step_id.to_string(),
            kind: kind.to_string(),
            label: label.to_string(),
        })
        .await;
}

async fn emit_step_completed(
    run_id: &str,
    step_id: &str,
    duration_ms: u64,
    summary: Option<&str>,
    events: &mpsc::Sender<AgentEvent>,
) {
    let _ = events
        .send(AgentEvent::StepCompleted {
            run_id: run_id.to_string(),
            step_id: step_id.to_string(),
            duration_ms,
            summary: summary.map(str::to_string),
        })
        .await;
}

/// Prefer ~48–96 byte deltas so assistant-ui paints during the final answer
/// even though Pioneer tool rounds are non-streaming.
const FINAL_TEXT_CHUNK_TARGET: usize = 72;

async fn emit_text_content_streamed(
    run_id: &str,
    content: &str,
    events: &mpsc::Sender<AgentEvent>,
) {
    if content.is_empty() {
        return;
    }
    let id = format!("{run_id}-text");
    let _ = events
        .send(AgentEvent::MessageChunk {
            run_id: run_id.to_string(),
            chunk: json!({ "type": "text-start", "id": id }),
        })
        .await;

    for delta in chunk_text_for_stream(content, FINAL_TEXT_CHUNK_TARGET) {
        let _ = events
            .send(AgentEvent::MessageChunk {
                run_id: run_id.to_string(),
                chunk: json!({
                    "type": "text-delta",
                    "id": id,
                    "delta": delta,
                }),
            })
            .await;
        // Let the JSONL / Tauri channel flush between deltas.
        tokio::task::yield_now().await;
    }

    let _ = events
        .send(AgentEvent::MessageChunk {
            run_id: run_id.to_string(),
            chunk: json!({ "type": "text-end", "id": id }),
        })
        .await;
}

fn chunk_text_for_stream(content: &str, target: usize) -> Vec<&str> {
    if content.len() <= target {
        return vec![content];
    }
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = content.as_bytes();
    while start < bytes.len() {
        let mut end = (start + target).min(bytes.len());
        if end < bytes.len() {
            // Prefer breaking on whitespace so we don't split UTF-8 mid-word often.
            if let Some(rel) = content[start..end].rfind(char::is_whitespace) {
                end = start + rel + 1;
            }
            while end > start && !content.is_char_boundary(end) {
                end -= 1;
            }
        }
        if end <= start {
            end = (start + 1..bytes.len() + 1)
                .find(|&i| content.is_char_boundary(i))
                .unwrap_or(bytes.len());
        }
        out.push(&content[start..end]);
        start = end;
    }
    out
}

async fn stream_chat_completions(
    api_key: &str,
    base_url: &str,
    model: &str,
    prompt: &str,
    run_id: &str,
    cancel: &AtomicBool,
    events: &mpsc::Sender<AgentEvent>,
) -> Result<(), PioneerError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| PioneerError::Transport(err.to_string()))?;

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": true,
    });

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|err| PioneerError::Transport(err.to_string()))?,
    );
    // Pioneer curl examples use X-API-Key; send both for compatibility.
    if let Ok(value) = HeaderValue::from_str(api_key) {
        headers.insert("X-API-Key", value);
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let response = tokio::select! {
        biased;
        _ = wait_cancelled(cancel) => return Err(PioneerError::Cancelled),
        result = client.post(&url).headers(headers).json(&body).send() => {
            result.map_err(|err| PioneerError::Transport(err.to_string()))?
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(PioneerError::Http {
            status: status.as_u16(),
            body: truncate(&body, 512),
        });
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut text_id: Option<String> = None;
    let mut saw_done = false;

    while let Some(chunk) = tokio::select! {
        biased;
        _ = wait_cancelled(cancel) => return Err(PioneerError::Cancelled),
        item = stream.next() => item,
    } {
        let chunk = chunk.map_err(|err| PioneerError::Transport(err.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(boundary) = buffer.find('\n') {
            let mut line = buffer[..boundary].to_string();
            buffer.drain(..=boundary);
            if line.ends_with('\r') {
                line.pop();
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                saw_done = true;
                break;
            }

            let payload: Value = match serde_json::from_str(data) {
                Ok(value) => value,
                Err(err) => {
                    warn!(error = %err, "pioneer SSE JSON parse failed");
                    continue;
                }
            };
            if let Some(err) = payload.get("error") {
                let message = err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pioneer API error");
                return Err(PioneerError::Api(message.to_string()));
            }

            let delta = payload
                .pointer("/choices/0/delta/content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if delta.is_empty() {
                continue;
            }

            if text_id.is_none() {
                let id = format!("{run_id}-text");
                text_id = Some(id.clone());
                let _ = events
                    .send(AgentEvent::MessageChunk {
                        run_id: run_id.to_string(),
                        chunk: json!({ "type": "text-start", "id": id }),
                    })
                    .await;
            }
            if let Some(id) = text_id.as_ref() {
                let _ = events
                    .send(AgentEvent::MessageChunk {
                        run_id: run_id.to_string(),
                        chunk: json!({
                            "type": "text-delta",
                            "id": id,
                            "delta": delta,
                        }),
                    })
                    .await;
            }
        }
        if saw_done {
            break;
        }
    }

    if let Some(id) = text_id {
        let _ = events
            .send(AgentEvent::MessageChunk {
                run_id: run_id.to_string(),
                chunk: json!({ "type": "text-end", "id": id }),
            })
            .await;
        Ok(())
    } else if saw_done {
        // Empty completion is still a successful run.
        Ok(())
    } else if cancel.load(Ordering::SeqCst) {
        Err(PioneerError::Cancelled)
    } else {
        Err(PioneerError::Incomplete)
    }
}

async fn wait_cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_model_defaults_luna() {
        assert_eq!(normalize_model(""), DEFAULT_PIONEER_MODEL);
        assert_eq!(normalize_model("default"), DEFAULT_PIONEER_MODEL);
        assert_eq!(normalize_model("gpt-5.6-terra"), "gpt-5.6-terra");
    }

    #[test]
    fn chunk_text_splits_long_answers() {
        let text = "alpha beta gamma delta epsilon zeta eta theta";
        let chunks = chunk_text_for_stream(text, 12);
        assert!(chunks.len() > 1);
        assert_eq!(chunks.concat(), text);
    }
}
