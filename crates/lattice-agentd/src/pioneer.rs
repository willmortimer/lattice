//! Pioneer OpenAI-compatible Chat Completions streaming.
//!
//! Matches Node `apps/agentd` pioneer path: `https://api.pioneer.ai/v1` +
//! `PIONEER_API_KEY`, chat completions SSE (not Responses). Maps deltas into
//! the same AI SDK UI chunks as the OpenAI Responses client.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::warn;

use crate::protocol::{AgentEvent, ProviderKind};

pub const DEFAULT_PIONEER_BASE_URL: &str = "https://api.pioneer.ai/v1";
/// Cheap default for local testing (Pioneer catalog).
pub const DEFAULT_PIONEER_MODEL: &str = "gpt-5.6-luna";

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
}

#[derive(Debug, Clone)]
pub struct PioneerRunOptions {
    pub run_id: String,
    pub thread_id: String,
    pub model: String,
    pub prompt: String,
    pub api_key: String,
    pub base_url: String,
    pub cancel: Arc<AtomicBool>,
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

    match stream_chat_completions(
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
}
