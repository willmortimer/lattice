# ADR 0002: FluidAudio macOS provider

## Status

Accepted (voice subsystem).

## Context

Lattice needs high-quality, fully local speech-to-text on Apple Silicon without
cloud ASR. FluidAudio with Parakeet Unified English Core ML is the leading
native stack for this goal.

## Decision

Use **FluidAudio** with **Parakeet Unified English Core ML** as the initial
macOS `SpeechProvider`. Pin exact revisions during Milestone 0. Linux and
Windows require separate providers later.

## Consequences

- Excellent Apple Silicon integration.
- A native Swift bridge with a stable C ABI is required.
- Model and runtime versions must be pinned and audited for license and hash.
- Intel Macs are unsupported for this provider in v1.
- See [fluid-audio-bridge.md](../fluid-audio-bridge.md) and
  [licensing-distribution.md](../licensing-distribution.md).
