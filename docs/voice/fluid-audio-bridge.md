# FluidAudio Bridge

## Scope

How Swift FluidAudio code is packaged and accessed from Rust on macOS.
Provider choice: [adr/0002](./adr/0002-fluid-audio-macos-provider.md).

## Repository shape

Proposed layout:

```text
crates/
  lattice-voice/           # shared traits, protocol types, normalization
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
`SpeechProvider` behind `cfg(target_os = "macos")`.

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

## Memory ownership

| Resource | Owner |
|----------|-------|
| Transcript strings in callbacks | Bridge allocates; Rust copies then bridge frees (or explicit free fn documented) |
| Callback event buffers | Valid only for the duration of the callback unless documented otherwise |
| Audio data in `push_audio` | Borrowed for the call; bridge **must** copy if asynchronous use is required |
| Engine / session handles | Destroyed explicitly; destroy session before engine |
| After cancel | No further callbacks; late callbacks **must** be dropped |
| Panic / Swift error | Caught at the ABI boundary; never unwind across languages |

**No Swift error or Rust panic may cross the ABI.**

### Callback-thread constraints

Document the FluidAudio callback thread model after M0 (research Q7). Rust
**must** treat callbacks as potentially non-Tokio-executor threads and hop to
the appropriate runtime before touching shared state.

## Build integration

Document and implement:

- Swift Package Manager dependency pinning
- Exact FluidAudio revision or release policy (research Q1)
- arm64-only libraries for v1 (Intel unsupported)
- Tauri development builds linking the bridge
- Release builds with code signing
- CI cache strategy for SPM and compiled Core ML artifacts
- Reproducibility limitations of Core ML compilation
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

- FluidAudio pin and Parakeet artifact (research Q1)
- Callback scheduling vs Swift concurrency (research Q7)
- Simultaneous streaming + offline decode on one loaded model (research Q3–Q4)

## Acceptance criteria

- [ ] Opaque-handle C ABI is stable at version 1
- [ ] Ownership rules are implemented and tested
- [ ] Rust integration test transcribes fixture audio streaming + offline
- [ ] Incompatible ABI versions fail closed
