//! OpenAI Responses API streaming client (ADR 0051 / 0066).
//!
//! Posts to `{base}/responses` with `stream: true`, parses SSE events, and maps
//! text deltas into AI SDK–style UI chunks (`text-start` / `text-delta` /
//! `text-end`) carried by agent-protocol `message_chunk` events.
//!
//! When a Lattice HTTP client is configured, runs a thin tool loop (default
//! [`crate::loop_runtime::max_tool_rounds`] rounds, override via
//! `LATTICE_AGENT_MAX_TOOL_ROUNDS`) using Responses function tools; otherwise
//! streams text-only.
//!
//! Tool continuations use `previous_response_id` and send only
//! `function_call_output` items. Re-sending bare `function_call` items without
//! their paired `reasoning` items fails on gpt-5 / o-series models with HTTP 400.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::lattice_client::LatticeToolClient;
use crate::loop_runtime::max_tool_rounds;
use crate::protocol::{AgentEvent, ProviderKind};
use crate::tools::{
    dispatch_tool, openai_tool_definitions, ToolEventSink, ToolRunContext,
    WORKSPACE_AGENT_INSTRUCTIONS,
};

/// Default OpenAI API root (includes `/v1`).
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// Default model when `start_run.model` is empty (matches Node agentd openai path).
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5-nano";

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
    #[error("Hit {0} tool rounds — try a narrower ask or raise LATTICE_AGENT_MAX_TOOL_ROUNDS (default 32, max 128)")]
    ToolLoopExhausted(usize),
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
            (Self::ToolLoopExhausted(a), Self::ToolLoopExhausted(b)) => a == b,
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
    /// When set, enable Responses tool loop against latticed HTTP.
    pub lattice: Option<LatticeToolClient>,
    pub workspace_id: Option<String>,
    pub workspace_root: Option<String>,
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

/// Convert Chat Completions nested `tools` into Responses flat function tools.
pub fn responses_tool_definitions() -> Vec<Value> {
    openai_tool_definitions()
        .into_iter()
        .filter_map(|tool| {
            let func = tool.get("function")?;
            let name = func.get("name")?.as_str()?.to_string();
            let description = func
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let parameters = func
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
            Some(json!({
                "type": "function",
                "name": name,
                "description": description,
                "parameters": parameters,
            }))
        })
        .collect()
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
        thread_id: thread_id.clone(),
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

    let model = normalize_model(&model);
    let result = if lattice.is_some() {
        run_tool_loop(
            &api_key,
            &base_url,
            &model,
            &prompt,
            &run_id,
            &cancel,
            &events,
            lattice.as_ref(),
            &ToolRunContext {
                workspace_id,
                workspace_root,
                thread_id: Some(thread_id),
            },
        )
        .await
    } else {
        stream_responses_to_events(
            &api_key,
            &base_url,
            &model,
            &prompt,
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

async fn run_tool_loop(
    api_key: &str,
    base_url: &str,
    model: &str,
    prompt: &str,
    run_id: &str,
    cancel: &AtomicBool,
    events: &mpsc::Sender<AgentEvent>,
    lattice: Option<&LatticeToolClient>,
    tool_ctx: &ToolRunContext,
) -> Result<(), ResponsesError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| ResponsesError::Transport(err.to_string()))?;

    let url = format!("{}/responses", base_url.trim_end_matches('/'));
    let tools = responses_tool_definitions();
    let instructions = format!(
        "{}{}",
        WORKSPACE_AGENT_INSTRUCTIONS,
        tool_ctx.binding_instructions()
    );
    // First round: user message. Later rounds: function_call_output only, with
    // previous_response_id so the API keeps reasoning items that pair with
    // function_call (required for gpt-5 / o-series reasoning models).
    let mut input: Vec<Value> = vec![json!({ "role": "user", "content": prompt })];
    let mut previous_response_id: Option<String> = None;
    let sink = ToolEventSink {
        run_id: run_id.to_string(),
        events: events.clone(),
    };
    let max_rounds = max_tool_rounds();

    for round in 0..max_rounds {
        if cancel.load(Ordering::SeqCst) {
            return Err(ResponsesError::Cancelled);
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

        let mut body = json!({
            "model": model,
            "instructions": instructions,
            "input": input,
            "tools": tools,
            "tool_choice": "auto",
            "stream": true,
        });
        if let Some(prev) = previous_response_id.as_ref() {
            body["previous_response_id"] = json!(prev);
        }

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
            StreamRoundOutcome::FinalAnswer { .. } => return Ok(()),
            StreamRoundOutcome::ToolCalls {
                response_id,
                calls,
                echoed_items,
            } => {
                if let Some(id) = response_id {
                    previous_response_id = Some(id);
                }

                // When previous_response_id is available, only send tool outputs
                // on the next turn. Otherwise echo reasoning + function_call
                // items so reasoning models still accept the continuation.
                let mut next_input = Vec::new();
                if previous_response_id.is_none() {
                    next_input.extend(echoed_items);
                    let has_function_call = next_input.iter().any(|item| {
                        item.get("type").and_then(|t| t.as_str()) == Some("function_call")
                    });
                    if !has_function_call {
                        for call in &calls {
                            next_input.push(json!({
                                "type": "function_call",
                                "id": call.id,
                                "call_id": call.call_id,
                                "name": call.name,
                                "arguments": call.arguments,
                            }));
                        }
                    }
                }

                for (tool_idx, call) in calls.iter().enumerate() {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(ResponsesError::Cancelled);
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
                    next_input.push(json!({
                        "type": "function_call_output",
                        "call_id": call.call_id,
                        "output": content,
                    }));
                }
                input = next_input;
            }
        }
    }

    Err(ResponsesError::ToolLoopExhausted(max_rounds))
}

#[derive(Debug, Clone)]
struct AccumulatedFunctionCall {
    id: String,
    call_id: String,
    name: String,
    arguments: String,
}

#[derive(Debug)]
enum StreamRoundOutcome {
    FinalAnswer {
        #[allow(dead_code)]
        response_id: Option<String>,
    },
    ToolCalls {
        response_id: Option<String>,
        calls: Vec<AccumulatedFunctionCall>,
        /// Reasoning + function_call items in output order (fallback when
        /// `previous_response_id` is unavailable).
        echoed_items: Vec<Value>,
    },
}

#[derive(Default)]
struct FunctionCallBuilder {
    id: Option<String>,
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl FunctionCallBuilder {
    fn finish(self) -> Option<AccumulatedFunctionCall> {
        let call_id = self.call_id.filter(|s| !s.is_empty())?;
        let id = self.id.filter(|s| !s.is_empty()).unwrap_or_else(|| call_id.clone());
        let name = self.name.unwrap_or_default();
        Some(AccumulatedFunctionCall {
            id,
            call_id,
            name,
            arguments: if self.arguments.is_empty() {
                "{}".into()
            } else {
                self.arguments
            },
        })
    }
}

fn apply_function_call_item(
    by_index: &mut HashMap<usize, FunctionCallBuilder>,
    by_item_id: &mut HashMap<String, usize>,
    output_index: usize,
    item: &Value,
) {
    let slot = by_index.entry(output_index).or_default();
    if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
        if !id.is_empty() {
            slot.id = Some(id.to_string());
            by_item_id.insert(id.to_string(), output_index);
        }
    }
    if let Some(call_id) = item.get("call_id").and_then(|v| v.as_str()) {
        if !call_id.is_empty() {
            slot.call_id = Some(call_id.to_string());
        }
    }
    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
        if !name.is_empty() {
            slot.name = Some(name.to_string());
        }
    }
    if let Some(args) = item.get("arguments").and_then(|v| v.as_str()) {
        if !args.is_empty() {
            slot.arguments = args.to_string();
        }
    }
}

async fn stream_tool_round(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    body: &Value,
    run_id: &str,
    cancel: &AtomicBool,
    events: &mpsc::Sender<AgentEvent>,
) -> Result<StreamRoundOutcome, ResponsesError> {
    let request = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {api_key}"))
        .header(CONTENT_TYPE, "application/json")
        .json(body);

    let response = tokio::select! {
        biased;
        _ = wait_cancelled(cancel) => return Err(ResponsesError::Cancelled),
        result = request.send() => {
            result.map_err(|err| ResponsesError::Transport(err.to_string()))?
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(ResponsesError::Http {
            status: status.as_u16(),
            body: truncate_for_event(&body, 512),
        });
    }

    let mut byte_stream = response.bytes_stream();
    let mut line_buf = String::new();
    let mut mapper = UiChunkMapper::new(run_id.to_string());
    let mut saw_completed = false;
    let mut by_index: HashMap<usize, FunctionCallBuilder> = HashMap::new();
    let mut by_item_id: HashMap<String, usize> = HashMap::new();
    let mut ordered_indices: Vec<usize> = Vec::new();
    let mut response_id: Option<String> = None;
    let mut echoed_by_index: HashMap<usize, Value> = HashMap::new();
    let mut echoed_order: Vec<usize> = Vec::new();

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(ResponsesError::Cancelled);
        }

        let next = tokio::select! {
            biased;
            _ = wait_cancelled(cancel) => return Err(ResponsesError::Cancelled),
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
                    if let Some(outcome) = handle_tool_sse_line(
                        &line,
                        &mut mapper,
                        events,
                        &mut by_index,
                        &mut by_item_id,
                        &mut ordered_indices,
                        &mut response_id,
                        &mut echoed_by_index,
                        &mut echoed_order,
                    )
                    .await?
                    {
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

    if !line_buf.trim().is_empty() {
        if let Some(outcome) = handle_tool_sse_line(
            &line_buf,
            &mut mapper,
            events,
            &mut by_index,
            &mut by_item_id,
            &mut ordered_indices,
            &mut response_id,
            &mut echoed_by_index,
            &mut echoed_order,
        )
        .await?
        {
            match outcome {
                StreamOutcome::Completed => saw_completed = true,
                StreamOutcome::Failed(msg) => return Err(ResponsesError::Api(msg)),
            }
        }
    }

    let mut calls: Vec<AccumulatedFunctionCall> = ordered_indices
        .into_iter()
        .filter_map(|idx| by_index.remove(&idx))
        .filter_map(FunctionCallBuilder::finish)
        .collect();
    // Include any builders that never got an ordered index (defensive).
    let mut leftovers: Vec<(usize, FunctionCallBuilder)> = by_index.into_iter().collect();
    leftovers.sort_by_key(|(idx, _)| *idx);
    for (_, builder) in leftovers {
        if let Some(call) = builder.finish() {
            calls.push(call);
        }
    }

    let echoed_items: Vec<Value> = echoed_order
        .into_iter()
        .filter_map(|idx| echoed_by_index.remove(&idx))
        .collect();

    if !calls.is_empty() {
        // Do not emit a partial text trail when the model chose tools.
        return Ok(StreamRoundOutcome::ToolCalls {
            response_id,
            calls,
            echoed_items,
        });
    }

    mapper.finish(events).await?;

    if saw_completed {
        Ok(StreamRoundOutcome::FinalAnswer { response_id })
    } else if cancel.load(Ordering::SeqCst) {
        Err(ResponsesError::Cancelled)
    } else {
        Err(ResponsesError::Incomplete)
    }
}

fn capture_response_id(value: &Value, response_id: &mut Option<String>) {
    if response_id.is_some() {
        return;
    }
    if let Some(id) = value
        .pointer("/response/id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        *response_id = Some(id.to_string());
    }
}

fn remember_echoed_item(
    echoed_by_index: &mut HashMap<usize, Value>,
    echoed_order: &mut Vec<usize>,
    output_index: usize,
    item: Value,
) {
    if !echoed_order.contains(&output_index) {
        echoed_order.push(output_index);
    }
    echoed_by_index.insert(output_index, item);
}

async fn handle_tool_sse_line(
    line: &str,
    mapper: &mut UiChunkMapper,
    events: &mpsc::Sender<AgentEvent>,
    by_index: &mut HashMap<usize, FunctionCallBuilder>,
    by_item_id: &mut HashMap<String, usize>,
    ordered_indices: &mut Vec<usize>,
    response_id: &mut Option<String>,
    echoed_by_index: &mut HashMap<usize, Value>,
    echoed_order: &mut Vec<usize>,
) -> Result<Option<StreamOutcome>, ResponsesError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(':') {
        return Ok(None);
    }
    if trimmed.starts_with("event:") {
        return Ok(None);
    }
    let data = if let Some(rest) = trimmed.strip_prefix("data:") {
        rest.trim()
    } else {
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

    let Some(event_type) = value.get("type").and_then(|v| v.as_str()) else {
        return Ok(None);
    };

    match event_type {
        "response.created" | "response.in_progress" => {
            capture_response_id(&value, response_id);
            Ok(None)
        }
        "response.output_item.added" | "response.output_item.done" => {
            capture_response_id(&value, response_id);
            let item = value.get("item").cloned().unwrap_or(Value::Null);
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let output_index = value
                .get("output_index")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            match item_type {
                "function_call" => {
                    if !ordered_indices.contains(&output_index) {
                        ordered_indices.push(output_index);
                    }
                    apply_function_call_item(by_index, by_item_id, output_index, &item);
                    // Prefer the completed item payload when available.
                    if event_type == "response.output_item.done" {
                        remember_echoed_item(echoed_by_index, echoed_order, output_index, item);
                    }
                }
                "reasoning" => {
                    // Reasoning models emit rs_* items that must precede their
                    // paired function_call when previous_response_id is absent.
                    if event_type == "response.output_item.done" || !echoed_by_index.contains_key(&output_index)
                    {
                        remember_echoed_item(echoed_by_index, echoed_order, output_index, item);
                    }
                }
                _ => {}
            }
            Ok(None)
        }
        "response.function_call_arguments.delta" => {
            let output_index = value
                .get("output_index")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .or_else(|| {
                    value
                        .get("item_id")
                        .and_then(|v| v.as_str())
                        .and_then(|id| by_item_id.get(id).copied())
                })
                .unwrap_or(0);
            if !ordered_indices.contains(&output_index) {
                ordered_indices.push(output_index);
            }
            if let Some(item_id) = value.get("item_id").and_then(|v| v.as_str()) {
                by_item_id.entry(item_id.to_string()).or_insert(output_index);
                let slot = by_index.entry(output_index).or_default();
                if slot.id.is_none() {
                    slot.id = Some(item_id.to_string());
                }
            }
            if let Some(delta) = value.get("delta").and_then(|v| v.as_str()) {
                by_index
                    .entry(output_index)
                    .or_default()
                    .arguments
                    .push_str(delta);
            }
            Ok(None)
        }
        "response.function_call_arguments.done" => {
            let output_index = value
                .get("output_index")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .or_else(|| {
                    value
                        .get("item_id")
                        .and_then(|v| v.as_str())
                        .and_then(|id| by_item_id.get(id).copied())
                })
                .unwrap_or(0);
            if let Some(args) = value.get("arguments").and_then(|v| v.as_str()) {
                by_index.entry(output_index).or_default().arguments = args.to_string();
            }
            Ok(None)
        }
        "response.output_text.delta"
        | "response.output_text.done"
        | "response.completed"
        | "response.failed"
        | "error" => {
            if event_type == "response.completed" {
                capture_response_id(&value, response_id);
            }
            // Only stream assistant text when this round is not assembling tools.
            if by_index.is_empty() {
                mapper.apply(&value, events).await
            } else if event_type == "response.completed" {
                Ok(Some(StreamOutcome::Completed))
            } else if event_type == "response.failed" || event_type == "error" {
                mapper.apply(&value, events).await
            } else {
                Ok(None)
            }
        }
        other => {
            debug!(event_type = other, "ignoring Responses SSE event");
            Ok(None)
        }
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
                lattice: None,
                workspace_id: None,
                workspace_root: None,
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

    #[test]
    fn responses_tools_are_flat_function_shape() {
        let tools = responses_tool_definitions();
        assert!(!tools.is_empty());
        let search = tools
            .iter()
            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("search"))
            .expect("search tool");
        assert_eq!(search.get("type").and_then(|t| t.as_str()), Some("function"));
        assert!(search.get("function").is_none());
        assert!(search.get("parameters").is_some());
        assert!(search.get("description").is_some());
    }
}
