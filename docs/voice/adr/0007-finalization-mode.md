# ADR 0007: Explicit finalization mode

## Status

Accepted (voice subsystem).

## Context

`SpeechCapabilities` previously exposed `offline_final_decode: bool`. The
macOS FluidAudio path reported `true` while finals came only from
`StreamingUnifiedAsrManager.finish()` — an authoritative flush of the streaming
checkpoint, not an independent offline re-decode. That misled product and
evaluation language about “offline final quality.”

See
[current-implementation-review-and-ml-architecture.md](../current-implementation-review-and-ml-architecture.md).

## Decision

Replace the boolean with an explicit enum on capabilities and finals:

```text
FinalizationMode =
  StreamingFlush
  | SameFamilyOfflineRedecode
  | IndependentOfflineRedecode
```

- Report `StreamingFlush` for the current Unified `finish()` path.
- Report `SameFamilyOfflineRedecode` only when a separate offline encoder in
  the same model family re-decodes buffered utterance audio.
- Report `IndependentOfflineRedecode` only when a distinct final model (for
  example Parakeet TDT v2) re-decodes the full utterance.

Final transcripts must carry the declared mode. Do not claim offline
independence without an actual second decode path.

## Consequences

- Protocol and provider code must be updated before shipping quality claims.
- Evaluation must score acoustic finals separately from post-normalization.
- Model memory policy must account for an optional second loaded model.
