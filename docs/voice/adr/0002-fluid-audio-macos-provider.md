# ADR 0002: FluidAudio macOS provider

## Status

Accepted (voice subsystem). **Amended** after M0 research spike
([research/voice-m0-fluidaudio/RESULTS.md](../../../research/voice-m0-fluidaudio/RESULTS.md)).

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
streaming + offline). That Unified path was **not measured** in M0.

## Decision

Use **FluidAudio** (pinned **0.15.5**, commit
`19600a485baa4998812e4654b70d2bab8f2c9949`) as the initial macOS
`SpeechProvider`. For the M0 spike and M1 bridge prototyping, use the
**EOU streaming + TDT v2 offline** pair documented in RESULTS.

**Production pin (still open):** ship EOU+TDT (two artifacts, ~890 MB cache) or
migrate to **Unified** (single artifact, not yet benchmarked in Lattice).

Minimum macOS: **14 (Sonoma)** per FluidAudio upstream platforms; M0 ran on
macOS 26.5 / M2 — oldest Apple Silicon pass remains open.

Linux and Windows require separate providers later.

## Consequences

- Excellent Apple Silicon integration.
- A native Swift bridge with a stable C ABI is required.
- Model and runtime versions must be pinned and audited for license and hash.
- Dual-artifact licensing: Apache-2.0 (FluidAudio) + NVIDIA Open Model (EOU) +
  CC-BY-4.0 (TDT v2); Unified would change attribution if adopted.
- Intel Macs are unsupported for this provider in v1.
- Callbacks arrive on background threads; Rust must hop before shared state.
- Audio wire format for the bridge: Float32 @ 16 kHz mono.
- See [fluid-audio-bridge.md](../fluid-audio-bridge.md) and
  [licensing-distribution.md](../licensing-distribution.md).
