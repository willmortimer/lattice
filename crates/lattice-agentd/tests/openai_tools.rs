//! OpenAI Responses tool loop: mock SSE (function_call → final) + latticed HTTP.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use lattice_agentd::lattice_client::LatticeToolClient;
use lattice_agentd::protocol::{AgentEvent, ProviderKind};
use lattice_agentd::responses::{emit_openai_run, OpenaiRunOptions};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

struct ResponsesSequence {
    calls: AtomicUsize,
    first: String,
    second: String,
}

impl Respond for ResponsesSequence {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let body = if n == 0 {
            self.first.as_str()
        } else {
            self.second.as_str()
        };
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(body)
    }
}

fn sse_function_call_round() -> String {
    [
        r#"data: {"type":"response.created","response":{"id":"resp_tools_1"}}"#,
        r#"data: {"type":"response.output_item.added","response_id":"resp_tools_1","output_index":0,"item":{"type":"function_call","id":"fc_search_1","call_id":"call_search_1","name":"search","arguments":""}}"#,
        r#"data: {"type":"response.function_call_arguments.delta","response_id":"resp_tools_1","item_id":"fc_search_1","output_index":0,"delta":"{\"query\":\"Events\"}"}"#,
        r#"data: {"type":"response.function_call_arguments.done","response_id":"resp_tools_1","item_id":"fc_search_1","output_index":0,"arguments":"{\"query\":\"Events\"}"}"#,
        r#"data: {"type":"response.output_item.done","response_id":"resp_tools_1","output_index":0,"item":{"type":"function_call","id":"fc_search_1","call_id":"call_search_1","name":"search","arguments":"{\"query\":\"Events\"}"}}"#,
        r#"data: {"type":"response.completed","response":{"id":"resp_tools_1","status":"completed"}}"#,
        "data: [DONE]",
        "",
    ]
    .join("\n")
}

fn sse_final_answer_round() -> String {
    [
        r#"data: {"type":"response.created","response":{"id":"resp_tools_2"}}"#,
        r#"data: {"type":"response.output_text.delta","item_id":"msg_final","output_index":0,"content_index":0,"delta":"Found "}"#,
        r#"data: {"type":"response.output_text.delta","item_id":"msg_final","output_index":0,"content_index":0,"delta":"Events in the workspace."}"#,
        r#"data: {"type":"response.output_text.done","item_id":"msg_final","output_index":0,"content_index":0,"text":"Found Events in the workspace."}"#,
        r#"data: {"type":"response.completed","response":{"id":"resp_tools_2","status":"completed"}}"#,
        "data: [DONE]",
        "",
    ]
    .join("\n")
}

#[tokio::test]
async fn openai_tool_loop_hits_search_then_completes() {
    let openai = MockServer::start().await;
    let latticed = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponsesSequence {
            calls: AtomicUsize::new(0),
            first: sse_function_call_round(),
            second: sse_final_answer_round(),
        })
        .expect(2)
        .mount(&openai)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/search"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(json!({
                    "hits": [{ "path": "Data/Events.dataset", "score": 1.0 }]
                })),
        )
        .expect(1)
        .mount(&latticed)
        .await;

    let lattice = LatticeToolClient::new(latticed.uri(), "test-token").expect("client");

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    emit_openai_run(
        OpenaiRunOptions {
            run_id: "r-oai-tools".into(),
            thread_id: "t-oai-tools".into(),
            model: "gpt-test".into(),
            prompt: "Search for Events".into(),
            api_key: "sk-test".into(),
            base_url: format!("{}/v1", openai.uri()),
            cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            lattice: Some(lattice),
            workspace_id: Some("ws-1".into()),
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
            run_id,
            ..
        }) if run_id == "r-oai-tools"
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::StepStarted { kind, .. } if kind == "tool")),
        "expected tool step_started while tools run"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::StepStarted { kind, .. } if kind == "model")),
        "expected model step_started while waiting on OpenAI"
    );
    let delta_count = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                AgentEvent::MessageChunk { chunk, .. }
                    if chunk.get("type").and_then(|t| t.as_str()) == Some("text-delta")
            )
        })
        .count();
    assert!(
        delta_count >= 2,
        "expected live streamed text-delta chunks, got {delta_count}"
    );
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::MessageChunk { chunk, .. } => {
                if chunk.get("type").and_then(|t| t.as_str()) == Some("text-delta") {
                    chunk
                        .get("delta")
                        .and_then(|d| d.as_str())
                        .map(str::to_string)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();
    assert!(
        text.contains("Found Events"),
        "expected final answer text, got {text:?}"
    );
    assert!(matches!(
        events.last(),
        Some(AgentEvent::RunCompleted { run_id }) if run_id == "r-oai-tools"
    ));
}
