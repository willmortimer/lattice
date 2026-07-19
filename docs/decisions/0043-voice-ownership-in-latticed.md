# ADR 0043: Voice model ownership moves to latticed (D5)

## Status

Accepted.

## Context

Local voice dictation is accepted under
[ADR 0040](0040-local-voice-dictation-documentation.md). The M2 prototype keeps
FluidAudio / Parakeet model state inside the Tauri process
([docs/voice/implementation-roadmap.md](../voice/implementation-roadmap.md) M4).
Daemon foundations now exist ([ADR 0041](0041-daemon-ipc-protobuf.md),
`apps/daemon`, embed-host supervision). Global Quick Note dictation must not
ship while the UI process owns inference.

## Decision

Execute Phase D5 of the
[latticed migration plan](../architecture/latticed-daemon-migration-plan.md):

1. **Native capture stays in the trusted desktop client** (ADR 0004 / voice ADR
   0008) and streams binary PCM to a daemon-authorized path.
2. **`latticed` owns** voice session policy, model prepare/residency, context
   building, transcript provenance, and host supervision.
3. **`lattice-voice-host` (macOS)** owns FluidAudio / Core ML inference in an
   isolated process, analogous to `lattice-embed-host`.
4. **Tauri becomes a thin client** over `LatticeClient`; it must not hold
   `FluidAudioSpeechProvider` as production model owner.
5. **Final text still commits** only through the semantic command core.

Voice Quick Note (M5) requires this exit criterion before claiming background
or shared residency.

## Consequences

- New voice host app + daemon supervisor module.
- Voice envelopes on the daemon control/event plane (or a dedicated framed
  stream compatible with ADR 0041).
- Crash isolation: voice-host faults must not corrupt workspace state.
- Implementation DAG: [docs/dev/voice-d5-quick-note-dag.md](../dev/voice-d5-quick-note-dag.md).
