//! Subprocess handshake against the `lattice-agentd` binary.

use std::process::Stdio;
use std::time::Duration;

use lattice_agentd::protocol::{AgentCommand, AgentEvent, PROTOCOL_VERSION};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

fn agentd_bin() -> &'static str {
    // Same pattern as apps/daemon and apps/embed-host integration tests.
    env!("CARGO_BIN_EXE_lattice-agentd")
}

#[tokio::test]
async fn subprocess_hello_handshake() {
    let mut child = Command::new(agentd_bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn lattice-agentd");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut lines = BufReader::new(stdout).lines();

    let hello = AgentCommand::Hello {
        protocol_version: PROTOCOL_VERSION,
    };
    stdin
        .write_all(hello.to_line().unwrap().as_bytes())
        .await
        .expect("write hello");
    stdin.flush().await.expect("flush");

    let line = timeout(Duration::from_secs(5), lines.next_line())
        .await
        .expect("hello_ack timeout")
        .expect("read stdout")
        .expect("stdout closed before hello_ack");

    let event = AgentEvent::from_line(&line).expect("parse hello_ack");
    assert_eq!(
        event,
        AgentEvent::HelloAck {
            protocol_version: PROTOCOL_VERSION
        }
    );

    let shutdown = AgentCommand::Shutdown;
    stdin
        .write_all(shutdown.to_line().unwrap().as_bytes())
        .await
        .expect("write shutdown");
    stdin.flush().await.expect("flush shutdown");
    drop(stdin);

    let status = timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("wait timeout")
        .expect("wait");
    assert!(status.success(), "agentd exit status: {status}");
}

#[tokio::test]
async fn subprocess_fake_start_run_stream() {
    let mut child = Command::new(agentd_bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut lines = BufReader::new(stdout).lines();

    stdin
        .write_all(
            br#"{"type":"hello","protocolVersion":1}
{"type":"start_run","threadId":"t-sub","runId":"r-sub","provider":"fake","model":"m","prompt":"yo"}
"#,
        )
        .await
        .expect("write");
    stdin.flush().await.expect("flush");

    let mut saw_started = false;
    let mut saw_chunk = false;
    let mut saw_completed = false;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        let next = timeout(Duration::from_millis(500), lines.next_line()).await;
        let Ok(Ok(Some(line))) = next else {
            continue;
        };
        let event = AgentEvent::from_line(&line).expect("event");
        match event {
            AgentEvent::HelloAck { .. } => {}
            AgentEvent::RunStarted { run_id, .. } if run_id == "r-sub" => saw_started = true,
            AgentEvent::MessageChunk { run_id, .. } if run_id == "r-sub" => saw_chunk = true,
            AgentEvent::RunCompleted { run_id } if run_id == "r-sub" => {
                saw_completed = true;
                break;
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    assert!(saw_started, "missing run_started");
    assert!(saw_chunk, "missing message_chunk");
    assert!(saw_completed, "missing run_completed");

    stdin
        .write_all(br#"{"type":"shutdown"}
"#)
        .await
        .expect("shutdown");
    drop(stdin);
    let _ = child.wait().await;
}
