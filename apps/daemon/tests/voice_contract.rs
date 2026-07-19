//! Voice-plane contract: DaemonClient ↔ latticed ↔ lattice-voice-host (fake).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lattice_client::{
    request, response, DaemonClient, EventFilter, LatticeClient, Request, PROTOCOL_VERSION,
};
use lattice_daemon::{
    resolve_voice_host_bin, serve_with_shutdown_and_controllers, DaemonConfig, VoiceController,
    VoiceProviderMode,
};
use lattice_protocol::{
    event, AudioSampleFormat, CancelVoiceSessionRequest, EndVoiceSessionRequest,
    FinishUtteranceRequest, GetVoiceCapabilitiesRequest, PrepareModelRequest,
    PushAudioChunkRequest, SessionContext, SpeechSessionConfig, StartVoiceSessionRequest,
};
use lattice_runtime::LatticeRuntime;
use tempfile::TempDir;
use tokio::sync::oneshot;

struct ServerGuard {
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<lattice_daemon::Result<()>>>,
    _dir: TempDir,
    socket_path: PathBuf,
    auth_token: String,
    voice: Option<Arc<VoiceController>>,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(voice) = self.voice.take() {
            voice.shutdown();
        }
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

fn ensure_voice_host_bin() -> PathBuf {
    if let Some(path) = resolve_voice_host_bin() {
        return path;
    }
    let status = std::process::Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "lattice-voice-host",
            "--bin",
            "lattice-voice-host",
        ])
        .status()
        .expect("spawn cargo build lattice-voice-host");
    assert!(
        status.success(),
        "cargo build -p lattice-voice-host failed: {status}"
    );
    resolve_voice_host_bin().expect(
        "lattice-voice-host binary missing after build (set LATTICE_VOICE_HOST_BIN)",
    )
}

async fn spawn_daemon_with_voice() -> ServerGuard {
    let dir = TempDir::new().expect("tempdir");
    let socket_path = dir.path().join("latticed.sock");
    let voice_socket = dir.path().join("voice-host.sock");
    let auth_token = "voice-contract-token".to_string();
    let config = DaemonConfig::new(&socket_path, auth_token.clone())
        .with_instance_id("voice-contract")
        .with_api_port(None)
        .with_keep_services_running(true);

    let binary = ensure_voice_host_bin();
    let voice = VoiceController::start(VoiceProviderMode::SpawnHost {
        binary,
        socket: voice_socket,
        fake: true,
    })
    .await
    .expect("start voice controller");

    let runtime = Arc::new(LatticeRuntime::new());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let join = tokio::spawn(serve_with_shutdown_and_controllers(
        config,
        runtime,
        None,
        Some(Arc::clone(&voice)),
        shutdown_rx,
    ));

    for _ in 0..100 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(socket_path.exists(), "daemon socket should appear");

    ServerGuard {
        shutdown: Some(shutdown_tx),
        join: Some(join),
        _dir: dir,
        socket_path,
        auth_token,
        voice: Some(voice),
    }
}

fn fixture_pcm() -> Vec<u8> {
    let mut payload = Vec::with_capacity(16);
    for sample in [0.1f32, -0.1, 0.2, -0.2] {
        payload.extend_from_slice(&sample.to_le_bytes());
    }
    payload
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn voice_rpcs_proxy_through_daemon_with_fake_host() {
    let guard = spawn_daemon_with_voice().await;
    let client = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("connect");

    let mut events = client
        .subscribe(EventFilter {
            workspace_id: None,
        })
        .await
        .expect("subscribe");

    let caps = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::GetVoiceCapabilities(
                GetVoiceCapabilitiesRequest {},
            )),
        })
        .await
        .expect("capabilities");
    match caps.body {
        Some(response::Body::GetVoiceCapabilities(resp)) => {
            assert!(resp.capabilities.is_some());
        }
        other => panic!("unexpected capabilities: {other:?}"),
    }

    let prepared = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::PrepareModel(PrepareModelRequest {
                model_id: "null-0.1".into(),
                warm: true,
            })),
        })
        .await
        .expect("prepare");
    match prepared.body {
        Some(response::Body::PrepareModel(resp)) => {
            assert!(resp.status.is_some());
        }
        other => panic!("unexpected prepare: {other:?}"),
    }

    let started = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::StartVoiceSession(StartVoiceSessionRequest {
                config: Some(SpeechSessionConfig {
                    session_id: "daemon-voice-1".into(),
                    language: Some("en".into()),
                    context: Some(SessionContext {
                        document_id: None,
                        glossary_terms: vec!["Lattice".into()],
                        command_mode: false,
                    }),
                    endpoint: None,
                }),
            })),
        })
        .await
        .expect("start");
    match started.body {
        Some(response::Body::StartVoiceSession(resp)) => {
            assert_eq!(resp.session_id, "daemon-voice-1");
            assert_eq!(resp.protocol_version, PROTOCOL_VERSION);
        }
        other => panic!("unexpected start: {other:?}"),
    }

    // One-session policy: second start must fail while the first is active.
    let busy = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::StartVoiceSession(StartVoiceSessionRequest {
                config: Some(SpeechSessionConfig {
                    session_id: "daemon-voice-2".into(),
                    language: None,
                    context: None,
                    endpoint: None,
                }),
            })),
        })
        .await
        .expect_err("second session should be rejected");
    assert!(
        busy.to_string().contains("voice_session_busy") || busy.to_string().contains("busy"),
        "unexpected busy error: {busy}"
    );

    let pushed = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::PushAudioChunk(PushAudioChunkRequest {
                session_id: "daemon-voice-1".into(),
                sequence: 0,
                captured_at_ns: 1_000,
                sample_rate_hz: 16_000,
                channels: 1,
                sample_format: AudioSampleFormat::F32.into(),
                payload: fixture_pcm(),
            })),
        })
        .await
        .expect("push");
    match pushed.body {
        Some(response::Body::PushAudioChunk(resp)) => assert_eq!(resp.sequence, 0),
        other => panic!("unexpected push: {other:?}"),
    }

    client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::FinishUtterance(FinishUtteranceRequest {
                session_id: "daemon-voice-1".into(),
                utterance_id: "utt-1".into(),
            })),
        })
        .await
        .expect("finish");

    let mut saw_partial = false;
    let mut saw_final = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline && !(saw_partial && saw_final) {
        match tokio::time::timeout(Duration::from_millis(500), events.next()).await {
            Ok(Some(Ok(event))) => match event.body {
                Some(event::Body::PartialTranscript(_)) => saw_partial = true,
                Some(event::Body::FinalTranscript(final_transcript)) => {
                    assert!(
                        final_transcript.text.contains("final")
                            || !final_transcript.text.is_empty(),
                        "unexpected final: {}",
                        final_transcript.text
                    );
                    saw_final = true;
                }
                Some(event::Body::ModelStatus(_))
                | Some(event::Body::SessionReady(_))
                | Some(event::Body::SpeechStarted(_))
                | Some(event::Body::SessionCompleted(_))
                | Some(event::Body::AudioGap(_)) => {}
                other => {
                    let _ = other;
                }
            },
            Ok(Some(Err(err))) => panic!("event stream error: {err}"),
            Ok(None) => break,
            Err(_) => continue,
        }
    }
    assert!(saw_partial, "expected PartialTranscript fan-out");
    assert!(saw_final, "expected FinalTranscript fan-out");

    client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::EndVoiceSession(EndVoiceSessionRequest {
                session_id: "daemon-voice-1".into(),
            })),
        })
        .await
        .expect("end");

    // Session slot freed — a new start should succeed.
    let restarted = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::StartVoiceSession(StartVoiceSessionRequest {
                config: Some(SpeechSessionConfig {
                    session_id: "daemon-voice-3".into(),
                    language: None,
                    context: None,
                    endpoint: None,
                }),
            })),
        })
        .await
        .expect("restart session");
    match restarted.body {
        Some(response::Body::StartVoiceSession(resp)) => {
            assert_eq!(resp.session_id, "daemon-voice-3");
        }
        other => panic!("unexpected restart: {other:?}"),
    }

    client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::CancelVoiceSession(CancelVoiceSessionRequest {
                session_id: "daemon-voice-3".into(),
                reason: Some("test cleanup".into()),
            })),
        })
        .await
        .expect("cancel");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn voice_unavailable_without_controller() {
    let dir = TempDir::new().expect("tempdir");
    let socket_path = dir.path().join("latticed.sock");
    let auth_token = "no-voice-token".to_string();
    let config = DaemonConfig::new(&socket_path, auth_token.clone())
        .with_instance_id("no-voice")
        .with_api_port(None)
        .with_keep_services_running(true);
    let runtime = Arc::new(LatticeRuntime::new());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let join = tokio::spawn(serve_with_shutdown_and_controllers(
        config,
        runtime,
        None,
        None,
        shutdown_rx,
    ));
    for _ in 0..100 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let client = DaemonClient::connect(&socket_path, &auth_token)
        .await
        .expect("connect");
    let err = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::GetVoiceCapabilities(
                GetVoiceCapabilitiesRequest {},
            )),
        })
        .await
        .expect_err("voice should be unavailable");
    let message = err.to_string();
    assert!(
        message.contains("voice_unavailable") || message.contains("not configured"),
        "expected voice_unavailable, got: {message}"
    );
    assert!(
        !message.contains("unimplemented"),
        "should not report unimplemented once handlers exist"
    );

    let _ = shutdown_tx.send(());
    join.abort();
}
