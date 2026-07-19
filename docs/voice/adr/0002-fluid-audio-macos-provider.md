# ADR 0002: FluidAudio macOS provider

## Status

Accepted (voice subsystem). **Amended** after M0 research spike
([research/voice-m0-fluidaudio/RESULTS.md](../../../research/voice-m0-fluidaudio/RESULTS.md))
and M1 production-path decision
([research/voice-m0-fluidaudio/DECISION.md](../../../research/voice-m0-fluidaudio/DECISION.md)).

## Context

Lattice needs high-quality, fully local speech-to-text on Apple Silicon without
cloud ASR. FluidAudio with NVIDIA Parakeet Core ML artifacts is the leading
native stack for this goal.

Original docs named **“Parakeet Unified English Core ML”** as a single model.
FluidAudio `0.15.5` exposes multiple artifacts. M0 **measured**:

- Streaming: `parakeet-realtime-eou-120m-coreml` (160 ms chunks) via
  `StreamingEouAsrManager` — NVIDIA Open Model License
- Offline: `parakeet-tdt-0.6b-v2-coreml` via `AsrManager` v2 — CC-BY-4.0

Upstream **also** ships `parakeet-unified-en-0.6b-coreml` via
`UnifiedAsrManager` / `StreamingUnifiedAsrManager` (one checkpoint for
streaming + offline). M0 exercised the EOU+TDT pair only; Task U later
measured Unified on the same fixture
([RESULTS-unified.md](../../../research/voice-m0-fluidaudio/RESULTS-unified.md)).

## Decision

Use **FluidAudio** (pinned **0.15.5**, commit
`19600a485baa4998812e4654b70d2bab8f2c9949`) as the initial macOS
`SpeechProvider`.

**Production path (M1, normative):** **Unified** — `parakeet-unified-en-0.6b-coreml`
via `StreamingUnifiedAsrManager` with the **`parakeet-unified-320ms`** streaming
tier. Streaming partials and the authoritative final both come from one loaded
checkpoint (`finish()` on the streaming manager; no second model family
required). Warm first-partial latency on the M0 fixture was **158.3 ms**
([RESULTS-unified.md](../../../research/voice-m0-fluidaudio/RESULTS-unified.md)).

**Measured fallback / historical M0 path:** EOU streaming (`parakeet-realtime-eou-120m-coreml`,
160 ms) plus TDT v2 offline (`parakeet-tdt-0.6b-v2-coreml`). This dual-artifact
stack (~890 MB combined cache) remains documented for comparison and regression
but is **not** the production pin.

The landed bridge (`crates/lattice-voice-macos`) wires Unified in
`VoiceEngine.prepare()`. See
[fluid-audio-bridge.md](../fluid-audio-bridge.md) and
[crates/lattice-voice-macos/README.md](../../../crates/lattice-voice-macos/README.md).

Minimum macOS: **14 (Sonoma)** per FluidAudio upstream platforms; M0 ran on
macOS 26.5 / M2 — oldest Apple Silicon pass remains open.

Linux and Windows require separate providers later.

## Consequences

- Excellent Apple Silicon integration.
- A native Swift bridge with a stable C ABI is required — **landed** at
  `crates/lattice-voice-macos` (ABI v1).
- Model and runtime versions must be pinned and audited for license and hash.
- Production licensing: Apache-2.0 (FluidAudio) + CC-BY-4.0 (Unified weights).
  The EOU+TDT pair adds NVIDIA Open Model (EOU) attribution if used as fallback.
- Intel Macs are unsupported for this provider in v1.
- Callbacks arrive on background threads; Rust must hop before shared state.
- Audio wire format for the bridge: Float32 @ 16 kHz mono.
- See [fluid-audio-bridge.md](../fluid-audio-bridge.md) and
  [licensing-distribution.md](../licensing-distribution.md).
