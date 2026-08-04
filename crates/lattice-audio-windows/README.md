# lattice-audio-windows

Windows microphone capture for Lattice voice via **cpal** (WASAPI).

Implements [`lattice_audio::CaptureProvider`](../lattice-audio) with canonical
output: **16 kHz mono Float32** (`CANONICAL_AUDIO_FORMAT`), ~20 ms frames, and
~300 ms pre-roll while armed.

## Stack

| Piece | Choice |
| --- | --- |
| Capture API | cpal → WASAPI default input device |
| Format | Device-native → mono downmix → linear resample to 16 kHz F32 |
| Transport | Unbounded `CaptureEvent` channel (client-owned; not `latticed`) |
| ASR | **Deferred** — no FluidAudio / voice-host on Windows yet |

Non-Windows hosts compile the public types and return
`CaptureError::Unsupported` so CI/unit tests stay green without WASAPI.

Desktop status: build with `--features voice` (or `capture,voice`).
`voice_status` reports `nativeCapture: true` when a default input exists;
`available` stays false until an ASR/dictation host lands.

## Tests

```sh
cargo test -p lattice-audio-windows
cargo check -p lattice-audio-windows
cargo check -p lattice-audio-windows --target x86_64-pc-windows-msvc
cargo check -p lattice-desktop --features voice
```

## Out of scope

- FluidAudio / Parakeet ASR on Windows
- `lattice-voice-host` Windows port
- AGC / noise suppression / echo cancellation
