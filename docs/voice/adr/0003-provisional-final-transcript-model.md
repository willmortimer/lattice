# ADR 0003: Provisional and final transcript model

## Status

Accepted (voice subsystem).

## Context

Streaming ASR is responsive but often lower quality than offline decode over a
complete utterance. Users need live feedback without committing unstable text.

## Decision

Use streaming recognition **only** for provisional UX. Use offline re-decoding
at utterance boundaries for **authoritative** final text.

## Consequences

- Audio must be buffered per utterance.
- Final text may differ from displayed provisional text.
- The editor must support atomic replacement of the provisional range.
- Recognition quality improves without sacrificing responsiveness.
- See [transcription-pipeline.md](../transcription-pipeline.md).
