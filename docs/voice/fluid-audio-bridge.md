# FluidAudio Bridge

## Scope

How Swift FluidAudio code is packaged and accessed from Rust on macOS.
Provider choice: [adr/0002](./adr/0002-fluid-audio-macos-provider.md).

M0 measurements and pins:
[research/voice-m0-fluidaudio/RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md).
Unified production measurements:
[research/voice-m0-fluidaudio/RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md).
Live Rust integration:
[crates/lattice-voice-macos/tests/LIVE_RESULTS.md](../../crates/lattice-voice-macos/tests/LIVE_RESULTS.md).

## Status

**Landed (M1).** The bridge lives under `crates/lattice-voice-macos` with C ABI
version **1**, `FluidAudioSpeechProvider` implementing `lattice_voice::SpeechProvider`,
and Unified (`parakeet-unified-320ms`) wired in Swift. Crate README:
[crates/lattice-voice-macos/README.md](../../crates/lattice-voice-macos/README.md).

## FluidAudio pin (M0 / M1)

| Item | Value |
|------|-------|
| Tag | `0.15.5` |
| Commit | `19600a485baa4998812e4654b70d2bab8f2c9949` |
| License | Apache-2.0 |
| Platforms (upstream `Package.swift`) | macOS 14+, iOS 17+ |
| Architecture | arm64 only (Intel unsupported for v1) |
| Production model | `parakeet-unified-en-0.6b-coreml` / `parakeet-unified-320ms` |

Pin FluidAudio by **exact SPM tag** in the Swift package; model weights remain
download-on-setup, not in git.

## Repository shape

Landed layout:

```text
crates/
  lattice-voice/           # shared traits, protocol types, normalization (landed)
  lattice-voice-macos/   # C ABI + FluidAudioSpeechProvider (landed, M1)
    build.rs
    include/lattice_voice_bridge.h
    src/
      lib.rs               # LATTICE_VOICE_BRIDGE_ABI_VERSION = 1
      ffi.rs
      bridge.rs
      provider.rs          # FluidAudioSpeechProvider
      error.rs
    swift/
      Package.swift        # FluidAudio 0.15.5 pin
      Sources/
        LatticeVoiceBridge/
          VoiceEngine.swift
          VoiceSession.swift
          BridgeExports.swift
          BridgeErrors.swift
    tests/
      live_asr.rs
      LIVE_RESULTS.md
    README.md
```

Shared Rust code **must not** import Swift types. Platform crates implement
`SpeechProvider` behind `cfg(target_os = "macos")`. Protocol types and the
in-process foundation live in `crates/lattice-voice`.

## Unified wiring (production)

`VoiceEngine.prepare()` loads `StreamingUnifiedAsrManager` with
`StreamingModelVariant.parakeetUnified320ms`. Session flow:

1. `push_audio` — Float32 LE mono @ 16 kHz (copied at ABI boundary).
2. Partial / stable events — background-thread callbacks; Rust hops before shared state.
3. `finish_utterance` — `StreamingUnifiedAsrManager.finish()` produces the
   authoritative final from the same loaded checkpoint (no TDT or second family).

Warm-cache live Rust path: **36 partials**, final in **~1.5 s** wall time
([LIVE_RESULTS.md](../../crates/lattice-voice-macos/tests/LIVE_RESULTS.md)).
Warm first-partial on the M0 fixture: **158.3 ms**
([RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md)).

The historical M0 EOU+TDT pair remains documented in
[RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md) as a measured
fallback, not the production wire-up.

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

### ABI notes from M0 / M1

- arm64-only; Intel unsupported for this provider.
- Prefer opaque C handles; **copy transcript strings at the ABI boundary**.
- FluidAudio partial callbacks run on **background threads** (`Thread.isMainThread
  == false`), not on the main queue or Tokio executor. Rust **must** hop before
  touching shared state.
- `StreamingUnifiedAsrManager` callbacks are `@Sendable` from the actor decode
  path — treat as non-main, non-Tokio.
- Keep Swift errors / panics from crossing the C ABI.
- After cancel, late callbacks **must** be dropped.

**No Swift error or Rust panic may cross the ABI.**

## Audio sample format

FluidAudio expects **Float32, 16 kHz, mono**:

- Production (Unified): 160 ms chunks (2560 samples) as non-interleaved
  `AVAudioPCMBuffer` Float32 mono into `StreamingUnifiedAsrManager`.
- Historical M0 EOU path used the same wire format with `StreamingEouAsrManager`.

The shared protocol keeps `AudioSampleFormat::I16Le` for runtime neutrality;
the macOS FluidAudio bridge **uses `F32` on the wire**
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
  per model on M0 host for EOU/TDT; Unified cold streaming load **~59.7 s** on
  Task U host — see [RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md);
  warm cached load **~504 ms** streaming Unified)
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
- Fixture audio through streaming and authoritative final path (M1 exit criterion)

## Open questions

- Simultaneous streaming + optional Unified offline encoder residency — offline
  encoder is **~578 MB** additional in the same HF repo; not required for
  streaming `finish()` production path.
- Separate VAD vs Parakeet EOU segmentation (research Q10) — not measured.

## Acceptance criteria

- [x] Opaque-handle C ABI is stable at version 1
- [x] Ownership rules are implemented and tested
- [x] Rust integration test transcribes fixture audio streaming + authoritative final
      ([LIVE_RESULTS.md](../../crates/lattice-voice-macos/tests/LIVE_RESULTS.md))
- [x] Incompatible ABI versions fail closed
