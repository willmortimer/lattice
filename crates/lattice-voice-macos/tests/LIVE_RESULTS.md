# Live ASR results — Task R

Recorded on Apple Silicon macOS from `cargo test -p lattice-voice-macos --features live-asr --test live_asr`.

## Environment

| Item | Value |
|------|-------|
| Date | 2026-07-18 |
| Fixture | `research/voice-m0-fluidaudio/Fixtures/technical-dictation-16k-mono.wav` |
| Model cache | `research/voice-m0-fluidaudio/.cache/Models` (`LATTICE_VOICE_MODEL_CACHE`) |
| Provider | `FluidAudioSpeechProvider` → Swift `libLatticeVoiceBridge.dylib` |
| Model | `parakeet-unified-en-0.6b-coreml` / `parakeet-unified-320ms` |
| Chunk size | 2560 samples (160 ms @ 16 kHz mono F32) |

## Run (warm cache)

```
live-asr partial_count=36
live-asr final_text=Lattice voice dictation should preserve camelcase identifiers like ASR Manager, File Paths such as users will developer lattice, and punctuation around code
live-asr processing_ms=20
live-asr total_elapsed_ms=1519
```

## Notes

- First `prepare()` on a cold cache downloads weights and compiles Core ML artifacts (minutes); subsequent runs are sub-second to prepare.
- Streaming partials arrived during chunked `push_audio`; authoritative final came from `finish_utterance()` (Unified `finish()` dual path).
- No model blobs are committed; cache is local-only under `.cache/`.
