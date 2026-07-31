//! On-device local LLM via an OpenAI-compatible Chat Completions endpoint.
//!
//! Configure with `LATTICE_LOCAL_LLM_BASE_URL` (e.g. `http://127.0.0.1:8080/v1`).
//! Optional: `LATTICE_LOCAL_LLM_MODEL`, `LATTICE_LOCAL_LLM_API_KEY`.

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
use crate::loop_runtime::max_tool_rounds;
use crate::protocol::{AgentEvent, ProviderKind};
use crate::tools::{
    dispatch_tool, openai_tool_definitions, ToolEventSink, ToolRunContext,
    WORKSPACE_AGENT_INSTRUCTIONS,
};

pub const DEFAULT_LOCAL_MODEL: &str = "local";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocalError {
    #[error("LATTICE_LOCAL_LLM_BASE_URL is required for provider=local")]
    MissingBaseUrl,
    #[error("Run cancelled")]
    Cancelled,
    #[error("local LLM HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("local LLM API error: {0}")]
    Api(String),
    #[error("local LLM transport error: {0}")]
    Transport(String),
    #[error("local LLM stream ended without completion")]
    Incomplete,
    #[error("Hit {0} tool rounds — try a narrower ask or raise LATTICE_AGENT_MAX_TOOL_ROUNDS (default 32, max 128)")]
    ToolLoopExhausted(usize),
}

#[derive(Debug, Clone)]
pub struct LocalRunOptions {
    pub run_id: String,
    pub thread_id: String,
    pub model: String,
    pub prompt: String,
    pub messages: Vec<Value>,
    pub base_url: String,
    pub api_key: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub lattice: Option<LatticeToolClient>,
    pub workspace_id: Option<String>,
    pub workspace_root: Option<String>,
}

pub fn base_url_from_env() -> Option<String> {
    std::env::var("LATTICE_LOCAL_LLM_BASE_URL")
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
}

pub fn api_key_from_env() -> Option<String> {
    std::env::var("LATTICE_LOCAL_LLM_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

pub fn model_from_env() -> Option<String> {
    std::env::var("LATTICE_LOCAL_LLM_MODEL")
        .ok()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
}

pub fn is_configured() -> bool {
    base_url_from_env().is_some()
}

pub fn normalize_model(model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        model_from_env().unwrap_or_else(|| DEFAULT_LOCAL_MODEL.to_string())
    } else {
        trimmed.to_string()
    }
}

pub async fn emit_local_run(options: LocalRunOptions, events: mpsc::Sender<AgentEvent>) {
    let LocalRunOptions {
        run_id,
        thread_id,
        model,
        prompt,
        messages,
        base_url,
        api_key,
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
        provider: Some(ProviderKind::Local),
    })
    .await;

    if base_url.trim().is_empty() {
        send(AgentEvent::RunFailed {
            run_id,
            message: LocalError::MissingBaseUrl.to_string(),
            retryable: false,
        })
        .await;
        return;
    }

    if cancel.load(Ordering::SeqCst) {
        send(AgentEvent::RunFailed {
            run_id,
            message: LocalError::Cancelled.to_string(),
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
            api_key.as_deref(),
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
            api_key.as_deref(),
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
                    message: LocalError::Cancelled.to_string(),
                    retryable: false,
                })
                .await;
            } else {
                send(AgentEvent::RunCompleted { run_id }).await;
            }
        }
        Err(LocalError::Cancelled) => {
            send(AgentEvent::RunFailed {
                run_id,
                message: LocalError::Cancelled.to_string(),
                retryable: false,
            })
            .await;
        }
        Err(err) => {
            let retryable = matches!(err, LocalError::Transport(_) | LocalError::Http { .. });
            send(AgentEvent::RunFailed {
                run_id,
                message: err.to_string(),
                retryable,
            })
            .await;
        }
    }
}

fn request_headers(api_key: Option<&str>) -> Result<HeaderMap, LocalError> {
    let mut headers = HeaderMap::new();
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {key}"))
                .map_err(|err| LocalError::Transport(err.to_string()))?,
        );
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(headers)
}

async fn run_tool_loop(
    api_key: Option<&str>,
    base_url: &str,
    model: &str,
    chat_messages: &[Value],
    run_id: &str,
    cancel: &AtomicBool,
    events: &mpsc::Sender<AgentEvent>,
    lattice: Option<&LatticeToolClient>,
    tool_ctx: &ToolRunContext,
) -> Result<(), LocalError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| LocalError::Transport(err.to_string()))?;

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let tools = openai_tool_definitions();
    let system = format!(
        "{}{}",
        WORKSPACE_AGENT_INSTRUCTIONS,
        tool_ctx.binding_instructions()
    );
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(chat_messages.iter().cloned());
    let sink = ToolEventSink {
        run_id: run_id.to_string(),
        events: events.clone(),
    };
    let max_rounds = max_tool_rounds();

    for round in 0..max_rounds {
        if cancel.load(Ordering::SeqCst) {
            return Err(LocalError::Cancelled);
        }

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
            "stream": true,
        });

        let outcome =
            stream_tool_round(&client, &url, api_key, &body, run_id, cancel, events).await?;
        emit_step_completed(
            run_id,
            &think_id,
            think_started.elapsed().as_millis() as u64,
            None,
            events,
        )
        .await;

        match outcome {
            StreamRoundOutcome::FinalAnswer => return Ok(()),
            StreamRoundOutcome::ToolCalls { message, calls } => {
                messages.push(message);
                for (tool_idx, call) in calls.iter().enumerate() {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(LocalError::Cancelled);
                    }
                    let step_id = format!("tool-{round}-{tool_idx}");
                    let label = if call.name.is_empty() {
                        "Running tool…".to_string()
                    } else {
                        format!("Running `{}`…", call.name)
                    };
                    let tool_started = std::time::Instant::now();
                    emit_step_started(run_id, &step_id, "tool", &label, events).await;
                    let content = dispatch_tool(
                        lattice,
                        tool_ctx,
                        Some(&sink),
                        &call.name,
                        &call.arguments,
                    )
                    .await;
                    emit_step_completed(
                        run_id,
                        &step_id,
                        tool_started.elapsed().as_millis() as u64,
                        Some(call.name.as_str()),
                        events,
                    )
                    .await;
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "content": content,
                    }));
                }
            }
        }
    }

    Err(LocalError::ToolLoopExhausted(max_rounds))
}

#[derive(Debug, Clone)]
struct AccumulatedToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug)]
enum StreamRoundOutcome {
    FinalAnswer,
    ToolCalls {
        message: Value,
        calls: Vec<AccumulatedToolCall>,
    },
}

#[derive(Default)]
struct ToolCallBuilder {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl ToolCallBuilder {
    fn finish(self) -> Option<AccumulatedToolCall> {
        let id = self.id.filter(|s| !s.is_empty())?;
        let name = self.name.unwrap_or_default();
        Some(AccumulatedToolCall {
            id,
            name,
            arguments: if self.arguments.is_empty() {
                "{}".into()
            } else {
                self.arguments
            },
        })
    }
}

fn apply_tool_call_delta(builders: &mut Vec<ToolCallBuilder>, delta_calls: &[Value]) {
    for call in delta_calls {
        let index = call.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        while builders.len() <= index {
            builders.push(ToolCallBuilder::default());
        }
        let slot = &mut builders[index];
        if let Some(id) = call.get("id").and_then(|v| v.as_str()) {
            if !id.is_empty() {
                slot.id = Some(id.to_string());
            }
        }
        if let Some(name) = call.pointer("/function/name").and_then(|v| v.as_str()) {
            if !name.is_empty() {
                slot.name = Some(match slot.name.take() {
                    Some(existing) => existing + name,
                    None => name.to_string(),
                });
            }
        }
        if let Some(args) = call.pointer("/function/arguments").and_then(|v| v.as_str()) {
            slot.arguments.push_str(args);
        }
    }
}

async fn stream_tool_round(
    client: &reqwest::Client,
    url: &str,
    api_key: Option<&str>,
    body: &Value,
    run_id: &str,
    cancel: &AtomicBool,
    events: &mpsc::Sender<AgentEvent>,
) -> Result<StreamRoundOutcome, LocalError> {
    let headers = request_headers(api_key)?;
    let response = tokio::select! {
        biased;
        _ = wait_cancelled(cancel) => return Err(LocalError::Cancelled),
        result = client.post(url).headers(headers).json(body).send() => {
            result.map_err(|err| LocalError::Transport(err.to_string()))?
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(LocalError::Http {
            status: status.as_u16(),
            body: truncate(&body, 512),
        });
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut saw_done = false;
    let mut finish_reason: Option<String> = None;
    let mut tool_builders: Vec<ToolCallBuilder> = Vec::new();
    let mut text_id: Option<String> = None;
    let mut assistant_content = String::new();

    while let Some(chunk) = tokio::select! {
        biased;
        _ = wait_cancelled(cancel) => return Err(LocalError::Cancelled),
        item = stream.next() => item,
    } {
        let chunk = chunk.map_err(|err| LocalError::Transport(err.to_string()))?;
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
                    warn!(error = %err, "local LLM tool SSE JSON parse failed");
                    continue;
                }
            };
            if let Some(err) = payload.get("error") {
                let message = err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("local LLM API error");
                return Err(LocalError::Api(message.to_string()));
            }

            if let Some(reason) = payload
                .pointer("/choices/0/finish_reason")
                .and_then(|v| v.as_str())
            {
                if !reason.is_empty() && reason != "null" {
                    finish_reason = Some(reason.to_string());
                }
            }

            let delta = payload.pointer("/choices/0/delta").cloned().unwrap_or(Value::Null);
            if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                apply_tool_call_delta(&mut tool_builders, calls);
            }

            let content_delta = delta.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if !content_delta.is_empty() && tool_builders.is_empty() {
                assistant_content.push_str(content_delta);
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
                                "delta": content_delta,
                            }),
                        })
                        .await;
                }
            }
        }
        if saw_done {
            break;
        }
    }

    if cancel.load(Ordering::SeqCst) {
        return Err(LocalError::Cancelled);
    }
    if !saw_done
        && finish_reason.is_none()
        && tool_builders.is_empty()
        && assistant_content.is_empty()
    {
        return Err(LocalError::Incomplete);
    }

    let calls: Vec<AccumulatedToolCall> = tool_builders
        .into_iter()
        .filter_map(ToolCallBuilder::finish)
        .collect();

    let wants_tools = finish_reason.as_deref() == Some("tool_calls") || !calls.is_empty();
    if wants_tools && !calls.is_empty() {
        let message = json!({
            "role": "assistant",
            "content": if assistant_content.is_empty() {
                Value::Null
            } else {
                Value::String(assistant_content)
            },
            "tool_calls": calls.iter().map(|c| json!({
                "id": c.id,
                "type": "function",
                "function": {
                    "name": c.name,
                    "arguments": c.arguments,
                }
            })).collect::<Vec<_>>(),
        });
        return Ok(StreamRoundOutcome::ToolCalls { message, calls });
    }

    if let Some(id) = text_id {
        let _ = events
            .send(AgentEvent::MessageChunk {
                run_id: run_id.to_string(),
                chunk: json!({ "type": "text-end", "id": id }),
            })
            .await;
    } else if !assistant_content.is_empty() {
        emit_text_content_streamed(run_id, &assistant_content, events).await;
    }

    Ok(StreamRoundOutcome::FinalAnswer)
}

async fn stream_chat_completions(
    api_key: Option<&str>,
    base_url: &str,
    model: &str,
    prompt: &str,
    run_id: &str,
    cancel: &AtomicBool,
    events: &mpsc::Sender<AgentEvent>,
) -> Result<(), LocalError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| LocalError::Transport(err.to_string()))?;

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": true,
    });
    let headers = request_headers(api_key)?;

    let response = tokio::select! {
        biased;
        _ = wait_cancelled(cancel) => return Err(LocalError::Cancelled),
        result = client.post(&url).headers(headers).json(&body).send() => {
            result.map_err(|err| LocalError::Transport(err.to_string()))?
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(LocalError::Http {
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
        _ = wait_cancelled(cancel) => return Err(LocalError::Cancelled),
        item = stream.next() => item,
    } {
        let chunk = chunk.map_err(|err| LocalError::Transport(err.to_string()))?;
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
                    warn!(error = %err, "local LLM SSE JSON parse failed");
                    continue;
                }
            };
            if let Some(err) = payload.get("error") {
                let message = err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("local LLM API error");
                return Err(LocalError::Api(message.to_string()));
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
        Ok(())
    } else if cancel.load(Ordering::SeqCst) {
        Err(LocalError::Cancelled)
    } else {
        Err(LocalError::Incomplete)
    }
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
    let _ = events
        .send(AgentEvent::MessageChunk {
            run_id: run_id.to_string(),
            chunk: json!({
                "type": "text-delta",
                "id": id,
                "delta": content,
            }),
        })
        .await;
    let _ = events
        .send(AgentEvent::MessageChunk {
            run_id: run_id.to_string(),
            chunk: json!({ "type": "text-end", "id": id }),
        })
        .await;
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
    fn normalize_model_defaults_local() {
        assert_eq!(normalize_model(""), DEFAULT_LOCAL_MODEL);
        assert_eq!(normalize_model("qwen2.5"), "qwen2.5");
    }

    #[test]
    fn tool_call_deltas_accumulate_by_index() {
        let mut builders = Vec::new();
        apply_tool_call_delta(
            &mut builders,
            &[json!({
                "index": 0,
                "id": "call_1",
                "type": "function",
                "function": { "name": "search", "arguments": "" }
            })],
        );
        apply_tool_call_delta(
            &mut builders,
            &[json!({
                "index": 0,
                "function": { "arguments": "{\"query\":\"x\"}" }
            })],
        );
        let call = builders.pop().unwrap().finish().unwrap();
        assert_eq!(call.id, "call_1");
        assert_eq!(call.name, "search");
        assert_eq!(call.arguments, "{\"query\":\"x\"}");
    }
}
