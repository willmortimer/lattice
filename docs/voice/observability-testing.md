# Observability and Testing

## Scope

Correctness, performance, and quality validation for voice dictation. Aligns
with [ADR 0016](../decisions/0016-opentelemetry-first-class.md) and
[docs/24](../24-observability-logging-and-telemetry.md).

## Local metrics

Collect **without** transcript content:

- Model load time
- First partial latency
- Finalization latency
- Audio duration
- Real-time factor
- Number of transcript revisions
- Dropped audio chunks
- IPC queue depth
- Session failure reason
- Memory usage
- Model unload count

Telemetry remains local unless the user explicitly opts into diagnostics
export.

## Unit tests

Cover:

- Session state transitions
- Audio sequence validation
- Transcript revision ordering
- Anchor resolution
- Final transcript replacement
- Command grammar
- Risk classification
- Model manifest validation
- License manifest generation
- IPC protocol compatibility

## Integration tests

Cover:

- Captured fixture audio through streaming decode
- Final re-decode replacing partial output
- Tauri-to-daemon IPC
- Model restart during session
- Editor anchor movement
- Quick Note persistence failure
- Microphone disconnection
- Permission denial
- Memory-pressure unload

## Golden audio suite

Create an internal, redistributable test corpus including:

- Quiet close-microphone speech
- Laptop microphone speech
- Fan noise
- Street noise
- Fast speech
- Slow speech
- Long pauses
- Technical vocabulary
- Project names
- Punctuation-heavy prose
- Spoken formatting commands
- Different English accents
- False starts and self-corrections

Store expected transcripts and permitted variants. Ensure corpus licensing
allows redistribution in the repo or a private CI cache with documented rights.

## Accuracy measurement

Measure:

- Word error rate
- Character error rate
- Command-intent accuracy
- Formatting-command accuracy
- First partial latency
- Final result latency
- Rate of harmful false command activation

Do **not** optimize only for WER. Dictation quality also depends on
punctuation, responsiveness, text stability, and command safety.

## Testing requirements

Milestone 8 hardens the full suite ([implementation-roadmap.md](./implementation-roadmap.md)).

## Open questions

- Where golden audio is stored (in-repo vs private artifact store)
- Acceptable WER bands per hardware tier

## Acceptance criteria

- [ ] Metrics exclude transcript/audio content by default
- [ ] Golden suite exists before release hardening sign-off
- [ ] False destructive command rate measured and near zero
