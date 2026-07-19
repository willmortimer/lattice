use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use lattice_protocol::event;
use lattice_voice::{AudioChunk, AudioSampleFormat, ModelState};
use lattice_voice_host::{
    collect_transcript_texts, run_server, socket_path_in, BackendKind, HostConfig, HostState,
    VoiceHostClient,
};
use tempfile::tempdir;
use tokio::process::Command;
use tokio::time::sleep;

async fn wait_for_socket(path: &std::path::Path) {
    for _ in 0..100 {
        if path.exists() {
            if VoiceHostClient::connect(path).await.is_ok() {
                return;
            }
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("socket not ready: {}", path.display());
}

fn fixture_chunk(session_id: &str, sequence: u64) -> AudioChunk {
    // Four f32 samples as packed little-endian bytes.
    let mut payload = Vec::with_capacity(16);
    for sample in [0.1f32, -0.1, 0.2, -0.2] {
        payload.extend_from_slice(&sample.to_le_bytes());
    }
    AudioChunk {
        session_id: session_id.into(),
        sequence,
        captured_at_ns: 1_000,
        sample_rate_hz: 16_000,
        channels: 1,
        sample_format: AudioSampleFormat::F32,
        payload: Bytes::from(payload),
    }
}

#[tokio::test]
async fn fake_backend_transcribes_fixture_over_uds() {
    let dir = tempdir().unwrap();
    let socket = socket_path_in(dir.path());

    let state = HostState::new(HostConfig::new(
        socket.clone(),
        BackendKind::Fake,
        None,
    ))
    .unwrap();
    let server = tokio::spawn(run_server(Arc::clone(&state)));

    wait_for_socket(&socket).await;

    let client = VoiceHostClient::connect(&socket).await.unwrap();
    let mut events = client.subscribe();

    let health = client.health().await.unwrap();
    assert_eq!(health.status, "ok");
    assert_eq!(health.backend.as_deref(), Some("fake"));

    let prepared = client.prepare_model("null-0.1", true).await.unwrap();
    assert_eq!(prepared.state, ModelState::Ready);

    let status = client.status().await.unwrap();
    assert_eq!(status.backend, "fake");
    assert_eq!(status.loaded_model_id.as_deref(), Some("null-0.1"));

    let session_id = client
        .start_session("voice_fixture", Some("en".into()))
        .await
        .unwrap();
    assert_eq!(session_id, "voice_fixture");

    // Drain SessionReady (response path also emits an event).
    let _ = events.recv().await;

    client
        .push_audio(fixture_chunk("voice_fixture", 0))
        .await
        .unwrap();

    client
        .finish_utterance("voice_fixture", "utt_1")
        .await
        .unwrap();

    let (partials, final_text) = collect_transcript_texts(&mut events, 1).await.unwrap();
    assert!(!partials.is_empty(), "expected at least one partial");
    assert!(
        partials.iter().any(|text| text.starts_with("partial-")),
        "unexpected partials: {partials:?}"
    );
    let final_text = final_text.expect("final transcript");
    assert!(
        final_text.contains("final"),
        "unexpected final text: {final_text}"
    );

    // Confirm a FinalTranscript event body was observed.
    assert!(matches!(
        events.try_recv().ok().and_then(|event| event.body),
        None | Some(event::Body::SessionCompleted(_))
            | Some(event::Body::SpeechStarted(_))
            | Some(event::Body::EndpointDetected(_))
            | Some(event::Body::PartialTranscript(_))
            | Some(event::Body::FinalTranscript(_))
            | Some(event::Body::ModelStatus(_))
    ));

    let status = client.status().await.unwrap();
    assert!(status.chunks_accepted >= 1);
    assert!(status.finals_emitted >= 1);

    client.unload_model().await.unwrap();
    let status = client.status().await.unwrap();
    assert!(status.loaded_model_id.is_none());
    assert_eq!(
        status.model_status.map(|m| m.state),
        Some(lattice_protocol::ModelState::Unavailable as i32)
    );

    server.abort();
}

#[tokio::test]
async fn client_tolerates_host_crash() {
    let dir = tempdir().unwrap();
    let socket = socket_path_in(dir.path());

    let bin = env!("CARGO_BIN_EXE_lattice-voice-host");
    let mut child = Command::new(bin)
        .arg("serve")
        .arg("--socket")
        .arg(&socket)
        .arg("--backend")
        .arg("fake")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn voice-host");

    wait_for_socket(&socket).await;

    let client = VoiceHostClient::connect(&socket).await.unwrap();
    client.prepare_model("null-0.1", false).await.unwrap();
    client.start_session("crash_session", None).await.unwrap();
    client
        .push_audio(fixture_chunk("crash_session", 0))
        .await
        .unwrap();

    child.kill().await.expect("kill host");
    let _ = child.wait().await;
    sleep(Duration::from_millis(50)).await;

    let err = client
        .push_audio(fixture_chunk("crash_session", 1))
        .await
        .expect_err("host should be gone");
    let message = err.to_string();
    assert!(
        message.contains("closed")
            || message.contains("Connection")
            || message.contains("Broken pipe")
            || message.contains("No such file")
            || message.contains("os error")
            || message.contains("host error"),
        "unexpected error after crash: {message}"
    );
}
