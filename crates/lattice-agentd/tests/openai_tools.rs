//! OpenAI Responses tool loop: mock SSE (function_call → final) + latticed HTTP.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use lattice_agentd::lattice_client::LatticeToolClient;
use lattice_agentd::protocol::{AgentEvent, ProviderKind};
use lattice_agentd::responses::{emit_openai_run, OpenaiRunOptions};
use serde_json::{json, Value};
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
        r#"data: {"type":"response.output_item.added","response_id":"resp_tools_1","output_index":0,"item":{"type":"reasoning","id":"rs_tools_1","summary":[]}}"#,
        r#"data: {"type":"response.output_item.done","response_id":"resp_tools_1","output_index":0,"item":{"type":"reasoning","id":"rs_tools_1","summary":[]}}"#,
        r#"data: {"type":"response.output_item.added","response_id":"resp_tools_1","output_index":1,"item":{"type":"function_call","id":"fc_search_1","call_id":"call_search_1","name":"search","arguments":""}}"#,
        r#"data: {"type":"response.function_call_arguments.delta","response_id":"resp_tools_1","item_id":"fc_search_1","output_index":1,"delta":"{\"query\":\"Events\"}"}"#,
        r#"data: {"type":"response.function_call_arguments.done","response_id":"resp_tools_1","item_id":"fc_search_1","output_index":1,"arguments":"{\"query\":\"Events\"}"}"#,
        r#"data: {"type":"response.output_item.done","response_id":"resp_tools_1","output_index":1,"item":{"type":"function_call","id":"fc_search_1","call_id":"call_search_1","name":"search","arguments":"{\"query\":\"Events\"}"}}"#,
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

async fn run_tool_loop_against(
    openai: &MockServer,
    latticed: &MockServer,
    run_id: &str,
) -> Vec<AgentEvent> {
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponsesSequence {
            calls: AtomicUsize::new(0),
            first: sse_function_call_round(),
            second: sse_final_answer_round(),
        })
        .expect(2)
        .mount(openai)
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
        .mount(latticed)
        .await;

    let lattice = LatticeToolClient::new(latticed.uri(), "test-token").expect("client");
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    emit_openai_run(
        OpenaiRunOptions {
            run_id: run_id.into(),
            thread_id: format!("t-{run_id}"),
            model: "gpt-5-nano".into(),
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
    events
}

#[tokio::test]
async fn openai_tool_loop_hits_search_then_completes() {
    let openai = MockServer::start().await;
    let latticed = MockServer::start().await;
    let events = run_tool_loop_against(&openai, &latticed, "r-oai-tools").await;

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

#[tokio::test]
async fn openai_tool_continuation_uses_previous_response_id() {
    let openai = MockServer::start().await;
    let latticed = MockServer::start().await;
    let events = run_tool_loop_against(&openai, &latticed, "r-oai-prev").await;
    assert!(matches!(
        events.last(),
        Some(AgentEvent::RunCompleted { run_id }) if run_id == "r-oai-prev"
    ));

    let requests = openai
        .received_requests()
        .await
        .expect("mock received requests");
    assert_eq!(requests.len(), 2, "expected two Responses rounds");

    let first: Value = serde_json::from_slice(&requests[0].body).expect("first body");
    assert!(first.get("previous_response_id").is_none());
    assert_eq!(
        first.pointer("/input/0/role").and_then(|v| v.as_str()),
        Some("user")
    );

    let second: Value = serde_json::from_slice(&requests[1].body).expect("second body");
    assert_eq!(
        second
            .get("previous_response_id")
            .and_then(|v| v.as_str()),
        Some("resp_tools_1"),
        "tool continuation must reference the prior response so reasoning items stay paired"
    );
    let input = second
        .get("input")
        .and_then(|v| v.as_array())
        .expect("continuation input array");
    assert!(
        !input.is_empty()
            && input.iter().all(|item| {
                item.get("type").and_then(|t| t.as_str()) == Some("function_call_output")
            }),
        "continuation input must be function_call_output only, got {input:?}"
    );
    assert_eq!(
        input[0].get("call_id").and_then(|v| v.as_str()),
        Some("call_search_1")
    );
}
