//! Stdio JSONL command loop matching latticed sidecar expectations.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::fake::{emit_fake_run, FakeRunOptions};
use crate::protocol::{AgentCommand, AgentEvent, ProviderKind, PROTOCOL_VERSION};
use crate::responses::{self, OpenaiRunOptions};

/// Runtime knobs for the JSONL loop (chunk delay helps cancel tests).
#[derive(Debug, Clone)]
pub struct LoopConfig {
    pub chunk_delay: Duration,
    /// Override `OPENAI_API_KEY` (tests). `None` reads the process environment.
    pub openai_api_key: Option<String>,
    /// Override Responses API base URL including `/v1` (wiremock / proxies).
    pub openai_base_url: Option<String>,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            // Small delay so cancel_run can interrupt an in-flight fake stream.
            chunk_delay: Duration::from_millis(5),
            openai_api_key: None,
            openai_base_url: None,
        }
    }
}

struct ActiveRun {
    run_id: String,
    cancel: Arc<AtomicBool>,
    join: tokio::task::JoinHandle<()>,
}

/// Drive the Phase A JSONL loop until `shutdown` or EOF.
pub async fn run_jsonl_loop<R, W>(
    mut reader: R,
    mut writer: W,
    config: LoopConfig,
) -> anyhow::Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);

    let mut active: Option<ActiveRun> = None;
    let mut line = String::new();
    let mut stdin_open = true;

    loop {
        // Reap finished runs so a later start_run does not wait on a dead join.
        if let Some(run) = active.as_ref() {
            if run.join.is_finished() {
                if let Some(run) = active.take() {
                    let _ = run.join.await;
                }
            }
        }

        if !stdin_open && active.is_none() {
            break;
        }

        tokio::select! {
            biased;

            Some(event) = event_rx.recv() => {
                write_event(&mut writer, &event).await?;
            }

            read = reader.read_line(&mut line), if stdin_open => {
                let n = read?;
                if n == 0 {
                    stdin_open = false;
                    continue;
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    line.clear();
                    continue;
                }

                let command = match AgentCommand::from_line(trimmed) {
                    Ok(command) => command,
                    Err(err) => {
                        warn!(error = %err, line = %trimmed, "failed to parse agent command");
                        line.clear();
                        continue;
                    }
                };
                line.clear();

                match command {
                    AgentCommand::Hello { protocol_version } => {
                        if protocol_version != PROTOCOL_VERSION {
                            warn!(
                                got = protocol_version,
                                expected = PROTOCOL_VERSION,
                                "hello protocolVersion mismatch; still acking PROTOCOL_VERSION"
                            );
                        }
                        write_event(
                            &mut writer,
                            &AgentEvent::HelloAck {
                                protocol_version: PROTOCOL_VERSION,
                            },
                        )
                        .await?;
                    }
                    AgentCommand::Health => {
                        write_event(&mut writer, &AgentEvent::Health { ok: true }).await?;
                    }
                    AgentCommand::Shutdown => {
                        stdin_open = false;
                        if let Some(run) = active.as_ref() {
                            run.cancel.store(true, Ordering::SeqCst);
                        }
                    }
                    AgentCommand::CancelRun { run_id } => {
                        if let Some(run) = active.as_ref() {
                            if run.run_id == run_id {
                                run.cancel.store(true, Ordering::SeqCst);
                            } else {
                                debug!(%run_id, active = %run.run_id, "cancel_run ignored; run not active");
                            }
                        } else {
                            debug!(%run_id, "cancel_run ignored; no active run");
                        }
                    }
                    AgentCommand::StartRun {
                        thread_id,
                        run_id,
                        provider,
                        model,
                        messages,
                        prompt,
                        workspace_id: _,
                        workspace_root: _,
                    } => {
                        // Only one in-flight run for Phase A (matches Node agentd).
                        if let Some(run) = active.take() {
                            run.cancel.store(true, Ordering::SeqCst);
                            let _ = run.join.await;
                        }

                        let prompt_text =
                            match prompt_from_start(prompt.as_deref(), messages.as_deref()) {
                                Ok(text) => text,
                                Err(message) => {
                                    write_event(
                                        &mut writer,
                                        &AgentEvent::RunFailed {
                                            run_id: run_id.clone(),
                                            message,
                                            retryable: false,
                                        },
                                    )
                                    .await?;
                                    continue;
                                }
                            };

                        let cancel = Arc::new(AtomicBool::new(false));
                        let events = event_tx.clone();
                        let chunk_delay = config.chunk_delay;
                        let openai_api_key = config.openai_api_key.clone();
                        let openai_base_url = config.openai_base_url.clone();
                        let cancel_for_task = Arc::clone(&cancel);
                        let run_id_task = run_id.clone();

                        let join = tokio::spawn(async move {
                            match provider {
                                ProviderKind::Fake => {
                                    emit_fake_run(
                                        FakeRunOptions {
                                            run_id: run_id_task,
                                            thread_id,
                                            prompt: prompt_text,
                                            chunk_delay,
                                            cancel: cancel_for_task,
                                        },
                                        events,
                                    )
                                    .await;
                                }
                                ProviderKind::Openai => {
                                    let api_key = openai_api_key
                                        .or_else(responses::api_key_from_env)
                                        .unwrap_or_default();
                                    let base_url = openai_base_url
                                        .unwrap_or_else(responses::base_url_from_env);
                                    responses::emit_openai_run(
                                        OpenaiRunOptions {
                                            run_id: run_id_task,
                                            thread_id,
                                            model,
                                            prompt: prompt_text,
                                            api_key,
                                            base_url,
                                            cancel: cancel_for_task,
                                        },
                                        events,
                                    )
                                    .await;
                                }
                                ProviderKind::Pioneer => {
                                    let _ = events
                                        .send(AgentEvent::RunStarted {
                                            run_id: run_id_task.clone(),
                                            thread_id,
                                            provider: Some(ProviderKind::Pioneer),
                                        })
                                        .await;
                                    let _ = events
                                        .send(AgentEvent::RunFailed {
                                            run_id: run_id_task,
                                            message: "pioneer provider is not implemented in lattice-agentd (use provider fake)".into(),
                                            retryable: false,
                                        })
                                        .await;
                                }
                            }
                        });

                        active = Some(ActiveRun {
                            run_id,
                            cancel,
                            join,
                        });
                    }
                }
            }
        }
    }

    drop(event_tx);
    while let Some(event) = event_rx.recv().await {
        write_event(&mut writer, &event).await?;
    }
    Ok(())
}

async fn write_event<W>(writer: &mut W, event: &AgentEvent) -> anyhow::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let line = event.to_line()?;
    writer.write_all(line.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

fn prompt_from_start(
    prompt: Option<&str>,
    messages: Option<&[serde_json::Value]>,
) -> Result<String, String> {
    if let Some(prompt) = prompt {
        if !prompt.is_empty() {
            return Ok(prompt.to_string());
        }
    }
    let Some(messages) = messages else {
        return Err("start_run requires messages or prompt".into());
    };
    if messages.is_empty() {
        return Err("start_run requires messages or prompt".into());
    }
    let mut parts = Vec::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let content = message
            .get("content")
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_else(|| message.to_string());
        parts.push(format!("{role}: {content}"));
    }
    Ok(parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    async fn drive(input: &str) -> String {
        drive_with(
            input,
            LoopConfig {
                chunk_delay: Duration::ZERO,
                openai_api_key: Some(String::new()),
                openai_base_url: None,
            },
        )
        .await
    }

    async fn drive_with(input: &str, config: LoopConfig) -> String {
        let reader = BufReader::new(input.as_bytes());
        let mut stdout = Vec::new();
        run_jsonl_loop(reader, &mut stdout, config)
            .await
            .expect("loop");
        String::from_utf8(stdout).expect("utf8")
    }

    #[tokio::test]
    async fn hello_yields_hello_ack() {
        let out = drive(r#"{"type":"hello","protocolVersion":1}"#).await;
        let event = AgentEvent::from_line(out.lines().next().unwrap()).unwrap();
        assert_eq!(
            event,
            AgentEvent::HelloAck {
                protocol_version: 1
            }
        );
    }

    #[tokio::test]
    async fn health_and_fake_start_run() {
        // EOF (no shutdown) waits for the in-flight fake run to complete.
        let input = concat!(
            r#"{"type":"hello","protocolVersion":1}"#,
            "\n",
            r#"{"type":"health"}"#,
            "\n",
            r#"{"type":"start_run","threadId":"t1","runId":"r1","provider":"fake","model":"m","prompt":"hi"}"#,
            "\n",
        );
        let out = drive(input).await;
        let events: Vec<AgentEvent> = out
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| AgentEvent::from_line(l).expect("event"))
            .collect();
        assert!(matches!(events[0], AgentEvent::HelloAck { .. }));
        assert!(matches!(events[1], AgentEvent::Health { ok: true }));
        assert!(events
            .iter()
            .any(|e| matches!(e, AgentEvent::RunStarted { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, AgentEvent::MessageChunk { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, AgentEvent::RunCompleted { run_id } if run_id == "r1")));
    }

    #[tokio::test]
    async fn openai_provider_fails_without_api_key() {
        let input = concat!(
            r#"{"type":"start_run","threadId":"t1","runId":"r-oai","provider":"openai","model":"gpt","prompt":"hi"}"#,
            "\n",
        );
        let out = drive(input).await;
        assert!(
            out.contains("OPENAI_API_KEY"),
            "expected missing-key failure, got {out}"
        );
        assert!(out.contains("run_failed"));
        assert!(out.contains("run_started"));
    }
}
