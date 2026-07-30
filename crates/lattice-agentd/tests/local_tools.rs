//! Local LLM tool loop: mock SSE chat completions (tool_call → final) + latticed HTTP.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use lattice_agentd::lattice_client::LatticeToolClient;
use lattice_agentd::local::{emit_local_run, LocalRunOptions};
use lattice_agentd::protocol::{AgentEvent, ProviderKind};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

struct ChatSequence {
    calls: AtomicUsize,
    first: String,
    second: String,
}

impl Respond for ChatSequence {
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

fn sse_tool_call_round() -> String {
    [
        r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"role":"assistant","content":null,"tool_calls":[{"index":0,"id":"call_search_1","type":"function","function":{"name":"search","arguments":""}}]},"finish_reason":null}]}"#,
        r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"query\":\"Events\"}"}}]},"finish_reason":null}]}"#,
        r#"data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
        "data: [DONE]",
        "",
    ]
    .join("\n")
}

fn sse_final_answer_round() -> String {
    [
        r#"data: {"id":"chatcmpl-2","choices":[{"index":0,"delta":{"role":"assistant","content":"Found "},"finish_reason":null}]}"#,
        r#"data: {"id":"chatcmpl-2","choices":[{"index":0,"delta":{"content":"Events in the workspace."},"finish_reason":null}]}"#,
        r#"data: {"id":"chatcmpl-2","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
        "data: [DONE]",
        "",
    ]
    .join("\n")
}

#[tokio::test]
async fn local_tool_loop_hits_search_then_completes() {
    let local = MockServer::start().await;
    let latticed = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ChatSequence {
            calls: AtomicUsize::new(0),
            first: sse_tool_call_round(),
            second: sse_final_answer_round(),
        })
        .expect(2)
        .mount(&local)
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
    emit_local_run(
        LocalRunOptions {
            run_id: "run-local-tools".into(),
            thread_id: "t-local".into(),
            model: "qwen".into(),
            prompt: "Search for Events".into(),
            messages: vec![],
            base_url: format!("{}/v1", local.uri()),
            api_key: None,
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
            provider: Some(ProviderKind::Local),
            ..
        })
    ));
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::StepStarted { kind, .. } if kind == "tool")));
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::MessageChunk { .. })));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::RunCompleted { .. })
    ));
}
