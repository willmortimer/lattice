//! Recorded OpenAI Responses SSE fixture + wiremock streaming tests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use lattice_agentd::protocol::{AgentEvent, ProviderKind};
use lattice_agentd::responses::{emit_openai_run, map_sse_fixture_to_chunks, OpenaiRunOptions};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIXTURE_SSE: &str = include_str!("fixtures/responses_text_stream.sse");

#[tokio::test]
async fn recorded_sse_fixture_maps_without_network() {
    let chunks = map_sse_fixture_to_chunks(FIXTURE_SSE, "r-fix")
        .await
        .expect("map fixture");
    assert_eq!(chunks[0], json!({"type":"text-start","id":"msg_fix"}));
    assert!(chunks.iter().any(|c| {
        c.get("type").and_then(|t| t.as_str()) == Some("text-delta")
            && c.get("delta").and_then(|d| d.as_str()) == Some("Lattice")
    }));
    assert_eq!(
        chunks.last().unwrap(),
        &json!({"type":"text-end","id":"msg_fix"})
    );
}

#[tokio::test]
async fn wiremock_openai_run_streams_message_chunks() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(FIXTURE_SSE),
        )
        .mount(&server)
        .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    emit_openai_run(
        OpenaiRunOptions {
            run_id: "r-wire".into(),
            thread_id: "t-wire".into(),
            model: "gpt-test".into(),
            prompt: "ping".into(),
            api_key: "sk-test".into(),
            base_url: format!("{}/v1", server.uri()),
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
            run_id,
            ..
        }) if run_id == "r-wire"
    ));
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::MessageChunk { .. })));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::RunCompleted { run_id }) if run_id == "r-wire"
    ));
}

#[tokio::test]
async fn wiremock_cancel_aborts_stream() {
    let server = MockServer::start().await;
    // Slow-ish body so cancel can win mid-stream.
    let slow_sse = format!(
        "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"resp_slow\"}}}}\n\n{}",
        (0..40)
            .map(|i| {
                format!(
                    "data: {{\"type\":\"response.output_text.delta\",\"item_id\":\"msg_slow\",\"delta\":\"x{i}\",\"sequence_number\":{}}}\n\n",
                    i + 1
                )
            })
            .collect::<String>()
    );
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                // Delay before headers so cancel races the request.
                .set_delay(Duration::from_millis(80))
                .set_body_string(slow_sse),
        )
        .mount(&server)
        .await;

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_flag = Arc::clone(&cancel);
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);

    let join = tokio::spawn(async move {
        emit_openai_run(
            OpenaiRunOptions {
                run_id: "r-cancel".into(),
                thread_id: "t1".into(),
                model: "gpt-test".into(),
                prompt: "slow".into(),
                api_key: "sk-test".into(),
                base_url: format!("{}/v1", server.uri()),
                cancel: cancel_flag,
                lattice: None,
                workspace_id: None,
                workspace_root: None,
            },
            tx,
        )
        .await;
    });

    let first = rx.recv().await.expect("run_started");
    assert!(matches!(first, AgentEvent::RunStarted { .. }));
    cancel.store(true, Ordering::SeqCst);

    let mut saw_cancelled = false;
    while let Some(event) = rx.recv().await {
        if matches!(
            event,
            AgentEvent::RunFailed { message, .. } if message == "Run cancelled"
        ) {
            saw_cancelled = true;
            break;
        }
    }
    join.await.expect("join");
    assert!(saw_cancelled, "expected run_failed Run cancelled");
}

#[tokio::test]
async fn wiremock_http_error_surfaces_run_failed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":{"message":"bad key"}}"#))
        .mount(&server)
        .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    emit_openai_run(
        OpenaiRunOptions {
            run_id: "r-401".into(),
            thread_id: "t1".into(),
            model: "gpt-test".into(),
            prompt: "hi".into(),
            api_key: "sk-bad".into(),
            base_url: format!("{}/v1", server.uri()),
            cancel: Arc::new(AtomicBool::new(false)),
            lattice: None,
            workspace_id: None,
            workspace_root: None,
        },
        tx,
    )
    .await;

    let mut failed = None;
    while let Some(event) = rx.recv().await {
        if let AgentEvent::RunFailed { message, .. } = event {
            failed = Some(message);
            break;
        }
    }
    let message = failed.expect("run_failed");
    assert!(message.contains("401"), "{message}");
}
