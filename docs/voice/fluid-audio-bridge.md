# FluidAudio Bridge

## Scope

How Swift FluidAudio code is packaged and accessed from Rust on macOS.
Provider choice: [adr/0002](./adr/0002-fluid-audio-macos-provider.md).

M0 measurements and pins:
[research/voice-m0-fluidaudio/RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md).

## FluidAudio pin (M0)

| Item | Value |
|------|-------|
| Tag | `0.15.5` |
| Commit | `19600a485baa4998812e4654b70d2bab8f2c9949` |
| License | Apache-2.0 |
| Platforms (upstream `Package.swift`) | macOS 14+, iOS 17+ |
| Architecture | arm64 only (Intel unsupported for v1) |

Pin FluidAudio by **exact SPM tag** in the Swift package; model weights remain
download-on-setup, not in git.

## Repository shape

Proposed layout:

```text
crates/
  lattice-voice/           # shared traits, protocol types, normalization (landed)
  lattice-voice-macos/
    build.rs
    src/
      lib.rs
      ffi.rs
      error.rs
    swift/
      Package.swift
      Sources/
        LatticeVoiceBridge/
          VoiceEngine.swift
          VoiceSession.swift
          BridgeExports.swift
          BridgeErrors.swift
```

Shared Rust code **must not** import Swift types. Platform crates implement
`SpeechProvider` behind `cfg(target_os = "macos")`. Protocol types and the
in-process foundation live in `crates/lattice-voice`.

## ABI design

Use opaque handles rather than exposing Swift objects:

```c
typedef uint64_t lattice_voice_engine_t;
typedef uint64_t lattice_voice_session_t;
```

Required operations:

```c
int32_t lattice_voice_engine_create(...);
int32_t lattice_voice_engine_prepare(...);
int32_t lattice_voice_session_start(...);
int32_t lattice_voice_session_push_audio(...);
int32_t lattice_voice_session_finish_utterance(...);
int32_t lattice_voice_session_cancel(...);
void lattice_voice_session_destroy(...);
void lattice_voice_engine_destroy(...);
```

Callbacks:

```c
typedef void (*lattice_voice_event_callback)(
    const lattice_voice_event_t *event,
    void *context
);
```

### Versioning

```rust
pub const LATTICE_VOICE_BRIDGE_ABI_VERSION: u32 = 1;
```

The Rust side **must** reject incompatible native bridge versions with a clear
error before creating sessions.

### ABI notes from M0

- arm64-only; Intel unsupported for this provider.
- Prefer opaque C handles; **copy transcript strings at the ABI boundary**.
- FluidAudio partial callbacks run on **background threads** (`Thread.isMainThread
  == false`), not on the main queue or Tokio executor. Rust **must** hop before
  touching shared state.
- `StreamingEouAsrManager` is a Swift `actor`; callbacks are `@Sendable` from the
  actor’s decode path — treat as non-main, non-Tokio.
- Keep Swift errors / panics from crossing the C ABI.
- After cancel, late callbacks **must** be dropped.

**No Swift error or Rust panic may cross the ABI.**

## Audio sample format

M0 confirmed FluidAudio expects **Float32, 16 kHz, mono**:

- Streaming: 160 ms chunks as non-interleaved `AVAudioPCMBuffer` Float32 mono
  (`StreamingEouAsrManager`, 160 ms variant).
- Offline: `[Float]` into `AsrManager.transcribe` (TDT v2).

The shared protocol keeps `AudioSampleFormat::I16Le` for runtime neutrality;
the macOS FluidAudio bridge **should** use `F32` on the wire
([audio-capture.md](./audio-capture.md)).

## Memory ownership

| Resource | Owner |
|----------|-------|
| Transcript strings in callbacks | Bridge allocates; Rust copies then bridge frees (or explicit free fn documented) |
| Callback event buffers | Valid only for the duration of the callback unless documented otherwise |
| Audio data in `push_audio` | Borrowed for the call; bridge **must** copy if asynchronous use is required |
| Engine / session handles | Destroyed explicitly; destroy session before engine |
| After cancel | No further callbacks; late callbacks **must** be dropped |
| Panic / Swift error | Caught at the ABI boundary; never unwind across languages |

## Build integration

Document and implement:

- Swift Package Manager dependency pinned to FluidAudio tag `0.15.5`
- arm64-only libraries for v1 (Intel unsupported)
- Tauri development builds linking the bridge
- Release builds with code signing
- CI cache strategy for SPM and compiled Core ML artifacts
- Reproducibility limitations of Core ML compilation (cold compile **~98–110 s**
  per model on M0 host; warm cached load **~400–680 ms**)
- How generated bridge artifacts are cleaned and rebuilt

## Interfaces

See session and event shapes in [transcription-pipeline.md](./transcription-pipeline.md)
and [daemon-protocol.md](./daemon-protocol.md).

## Security implications

- Tampered bridge libraries are in the threat model
  ([privacy-security.md](./privacy-security.md)).
- Bridge binaries are signed and version-checked.

## Testing requirements

- Memory-safety tests for create/destroy ordering
- Cancel-then-late-callback dropped safely
- ABI version mismatch rejected
- Fixture audio through streaming and offline paths (M1 exit criterion)

## Open questions

- **Unified vs EOU+TDT production pin** — M0 measured EOU streaming +
  TDT v2 offline (~890 MB combined cache); `parakeet-unified-en-0.6b-coreml` was
  not exercised.
- Simultaneous streaming + offline decode residency (research Q4) — not
  instrumented in M0; both models can load sequentially.
- Separate VAD vs Parakeet EOU segmentation (research Q10) — not measured.

## Acceptance criteria

- [ ] Opaque-handle C ABI is stable at version 1
- [ ] Ownership rules are implemented and tested
- [ ] Rust integration test transcribes fixture audio streaming + offline
- [ ] Incompatible ABI versions fail closed
