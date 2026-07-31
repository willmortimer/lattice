//! Deterministic fake provider stream (no network).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::protocol::{AgentEvent, ProviderKind};

/// Options for the synthetic fake run stream.
#[derive(Debug, Clone)]
pub struct FakeRunOptions {
    pub run_id: String,
    pub thread_id: String,
    pub prompt: String,
    /// Delay between message chunks so `cancel_run` can race in.
    pub chunk_delay: Duration,
    pub cancel: Arc<AtomicBool>,
}

/// Emit `run_started` → `message_chunk`(s) → `run_completed` (or `run_failed` on cancel).
pub async fn emit_fake_run(options: FakeRunOptions, events: mpsc::Sender<AgentEvent>) {
    let FakeRunOptions {
        run_id,
        thread_id,
        prompt,
        chunk_delay,
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
        provider: Some(ProviderKind::Fake),
    })
    .await;

    let text = format!("Echo: {prompt}");
    let message_id = "fake-msg";
    let parts = split_into_chunks(&text, 3);

    send(AgentEvent::MessageChunk {
        run_id: run_id.clone(),
        chunk: serde_json::json!({ "type": "text-start", "id": message_id }),
    })
    .await;

    for part in parts {
        if cancel.load(Ordering::SeqCst) {
            send(AgentEvent::RunFailed {
                run_id: run_id.clone(),
                message: "Run cancelled".into(),
                retryable: false,
            })
            .await;
            return;
        }
        if !chunk_delay.is_zero() {
            tokio::time::sleep(chunk_delay).await;
        }
        if cancel.load(Ordering::SeqCst) {
            send(AgentEvent::RunFailed {
                run_id: run_id.clone(),
                message: "Run cancelled".into(),
                retryable: false,
            })
            .await;
            return;
        }
        send(AgentEvent::MessageChunk {
            run_id: run_id.clone(),
            chunk: serde_json::json!({
                "type": "text-delta",
                "id": message_id,
                "delta": part,
            }),
        })
        .await;
    }

    if cancel.load(Ordering::SeqCst) {
        send(AgentEvent::RunFailed {
            run_id: run_id.clone(),
            message: "Run cancelled".into(),
            retryable: false,
        })
        .await;
        return;
    }

    send(AgentEvent::MessageChunk {
        run_id: run_id.clone(),
        chunk: serde_json::json!({ "type": "text-end", "id": message_id }),
    })
    .await;

    if prompt.trim().eq_ignore_ascii_case("spatial-demo") {
        emit_fake_spatial_sequence(&run_id, &events).await;
    }

    send(AgentEvent::RunCompleted { run_id }).await;
}

/// Deterministic `step_started` → `overlay_show` → `step_completed` sequence
/// for the `spatial-demo` fixture prompt (Phase C smoke / CI).
async fn emit_fake_spatial_sequence(run_id: &str, events: &mpsc::Sender<AgentEvent>) {
    let step_id = "fake-spatial-step";
    let overlay_id = "fake-spatial-overlay";
    let started = std::time::Instant::now();

    let _ = events
        .send(AgentEvent::StepStarted {
            run_id: run_id.to_string(),
            step_id: step_id.into(),
            kind: "search".into(),
            label: "Search demo page".into(),
        })
        .await;
    let _ = events
        .send(AgentEvent::OverlayShow {
            run_id: run_id.to_string(),
            overlay_id: overlay_id.into(),
            anchors: vec![serde_json::json!({
                "kind": "markdown-block",
                "resourceId": "fake-demo-page",
                "blockId": "fake-demo-block",
            })],
            purpose: "attention".into(),
            commentary: Some("Fake spatial fixture".into()),
        })
        .await;
    let _ = events
        .send(AgentEvent::StepCompleted {
            run_id: run_id.to_string(),
            step_id: step_id.into(),
            duration_ms: started.elapsed().as_millis() as u64,
            summary: Some("Highlighted demo block".into()),
        })
        .await;
}

/// Build a single text-delta Value (test helper / golden fixtures).
pub fn text_delta_chunk(id: &str, delta: &str) -> Value {
    serde_json::json!({ "type": "text-delta", "id": id, "delta": delta })
}

fn split_into_chunks(text: &str, count: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let n = count.clamp(1, text.len());
    let size = text.len().div_ceil(n);
    text.as_bytes()
        .chunks(size)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn fake_run_emits_started_chunks_completed() {
        let (tx, mut rx) = mpsc::channel(32);
        emit_fake_run(
            FakeRunOptions {
                run_id: "r1".into(),
                thread_id: "t1".into(),
                prompt: "hi".into(),
                chunk_delay: Duration::ZERO,
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
                run_id,
                thread_id,
                provider: Some(ProviderKind::Fake),
            }) if run_id == "r1" && thread_id == "t1"
        ));
        assert!(events
            .iter()
            .any(|e| matches!(e, AgentEvent::MessageChunk { .. })));
        assert!(matches!(
            events.last(),
            Some(AgentEvent::RunCompleted { run_id }) if run_id == "r1"
        ));
    }

    #[tokio::test]
    async fn fake_spatial_prompt_emits_overlay_sequence() {
        let (tx, mut rx) = mpsc::channel(32);
        emit_fake_run(
            FakeRunOptions {
                run_id: "run-spatial".into(),
                thread_id: "t1".into(),
                prompt: "spatial-demo".into(),
                chunk_delay: Duration::ZERO,
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

        assert!(
            events.iter().any(|event| matches!(
                event,
                AgentEvent::StepStarted { kind, label, .. }
                    if kind == "search" && label == "Search demo page"
            )),
            "expected search step_started, got {events:?}"
        );
        assert!(
            events.iter().any(|event| matches!(
                event,
                AgentEvent::OverlayShow { overlay_id, .. } if overlay_id == "fake-spatial-overlay"
            )),
            "expected overlay_show, got {events:?}"
        );
        assert!(
            events.iter().any(|event| matches!(
                event,
                AgentEvent::StepCompleted { step_id, .. } if step_id == "fake-spatial-step"
            )),
            "expected step_completed, got {events:?}"
        );
        assert!(matches!(events.last(), Some(AgentEvent::RunCompleted { .. })));
    }

    #[tokio::test]
    async fn fake_run_cancel_emits_failed() {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::clone(&cancel);
        let (tx, mut rx) = mpsc::channel(32);

        let run = tokio::spawn(async move {
            emit_fake_run(
                FakeRunOptions {
                    run_id: "r-cancel".into(),
                    thread_id: "t1".into(),
                    prompt: "slow".into(),
                    chunk_delay: Duration::from_millis(40),
                    cancel: cancel_flag,
                },
                tx,
            )
            .await;
        });

        // Wait for run_started, then cancel.
        let first = rx.recv().await.expect("run_started");
        assert!(matches!(first, AgentEvent::RunStarted { .. }));
        cancel.store(true, Ordering::SeqCst);

        let mut saw_failed = false;
        while let Some(event) = rx.recv().await {
            if matches!(
                event,
                AgentEvent::RunFailed {
                    message,
                    ..
                } if message == "Run cancelled"
            ) {
                saw_failed = true;
                break;
            }
        }
        run.await.expect("join");
        assert!(saw_failed, "expected run_failed after cancel");
    }
}
