#![cfg(all(feature = "live-asr", target_os = "macos", link_bridge))]

//! Live ASR integration test — requires cached FluidAudio models and fixture WAV.

use std::path::PathBuf;
use std::time::Instant;

use bytes::Bytes;
use hound::{SampleFormat, WavReader};
use lattice_voice::{
    AudioChunk, AudioSampleFormat, PrepareModelRequest, SessionContext, SpeechEventSender,
    SpeechProvider, SpeechSessionConfig, VoiceEvent,
};
use lattice_voice_macos::{default_model_cache_dir, FluidAudioSpeechProvider};

const CHUNK_SAMPLES: usize = 2_560; // 160 ms @ 16 kHz

fn fixture_path() -> PathBuf {
    if let Ok(path) = std::env::var("LATTICE_VOICE_FIXTURE_WAV") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../research/voice-m0-fluidaudio/Fixtures/technical-dictation-16k-mono.wav")
}

fn load_f32_mono_16k(path: &std::path::Path) -> Vec<f32> {
    let reader = WavReader::open(path).expect("open fixture wav");
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000, "fixture must be 16 kHz");
    assert_eq!(spec.channels, 1, "fixture must be mono");

    match spec.sample_format {
        SampleFormat::Float => reader
            .into_samples::<f32>()
            .map(|sample| sample.expect("sample"))
            .collect(),
        SampleFormat::Int => reader
            .into_samples::<i32>()
            .map(|sample| sample.expect("sample") as f32 / i32::MAX as f32)
            .collect(),
    }
}

fn chunk_from_samples(session_id: &str, sequence: u64, samples: &[f32]) -> AudioChunk {
    let mut payload = Vec::with_capacity(samples.len() * 4);
    for sample in samples {
        payload.extend_from_slice(&sample.to_le_bytes());
    }
    AudioChunk {
        session_id: session_id.into(),
        sequence,
        captured_at_ns: 0,
        sample_rate_hz: 16_000,
        channels: 1,
        sample_format: AudioSampleFormat::F32,
        payload: Bytes::from(payload),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_fixture_streams_partials_and_final() {
    let fixture = fixture_path();
    if !fixture.is_file() {
        eprintln!("skipping live-asr: fixture missing at {}", fixture.display());
        return;
    }

    let cache = default_model_cache_dir();
    std::fs::create_dir_all(&cache).expect("create model cache dir");

    let provider = FluidAudioSpeechProvider::with_model_cache(&cache).expect("provider");
    provider
        .prepare(PrepareModelRequest {
            model_id: "parakeet-unified-320ms".into(),
            warm: true,
        })
        .await
        .expect("prepare");

    let (events, mut rx) = SpeechEventSender::pair();
    let config = SpeechSessionConfig {
        session_id: "live_voice".into(),
        language: Some("en".into()),
        context: SessionContext {
            document_id: None,
            glossary_terms: Vec::new(),
            known_paths: Vec::new(),
            command_mode: false,
        },
        endpoint: lattice_voice::EndpointOptions::default(),
    };

    let mut session = provider.start_session(config, events).await.expect("session");
    let samples = load_f32_mono_16k(&fixture);
    let started = Instant::now();
    let mut sequence = 0_u64;

    for chunk in samples.chunks(CHUNK_SAMPLES) {
        session
            .push_audio(chunk_from_samples("live_voice", sequence, chunk))
            .await
            .expect("push");
        sequence += 1;
    }

    let final_transcript = session.finish_utterance().await.expect("finish");
    let elapsed_ms = started.elapsed().as_millis();

    let mut partial_count = 0_usize;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, VoiceEvent::PartialTranscript(_)) {
            partial_count += 1;
        }
    }

    eprintln!("live-asr partial_count={partial_count}");
    eprintln!("live-asr final_text={}", final_transcript.text);
    eprintln!("live-asr processing_ms={}", final_transcript.processing_ms);
    eprintln!("live-asr total_elapsed_ms={elapsed_ms}");

    assert!(partial_count > 0, "expected at least one partial transcript");
    assert!(
        !final_transcript.text.trim().is_empty(),
        "expected non-empty final transcript"
    );
}
