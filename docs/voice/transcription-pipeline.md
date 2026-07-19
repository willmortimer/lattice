# Transcription Pipeline

## Scope

Two-pass recognition lifecycle: streaming provisional UX and offline
authoritative finals ([adr/0003](./adr/0003-provisional-final-transcript-model.md)).

## Session state machine

```text
Created
  → Preparing
  → Ready
  → Listening
  → SpeechActive
  → Finalizing
  → Completed

Any active state may transition to Cancelled or Failed.
```

```rust
enum TranscriptionSessionState {
    Created,
    Preparing,
    Ready,
    Listening,
    SpeechActive,
    Finalizing,
    Completed,
    Cancelled,
    Failed,
}
```

Invalid transitions **must** return a typed error and leave the session in
`Failed` or unchanged per operation contract. After `Completed`, `Cancelled`,
or `Failed`, the session is terminal.

## Streaming provisional path

While audio arrives:

1. Feed chunks into the streaming decoder.
2. Receive partial hypotheses.
3. Assign monotonically increasing transcript revisions.
4. Emit provisional transcript events.
5. Mark stable and unstable token regions where supported.
6. Render the current hypothesis in the editor without committing it
   ([adr/0005](./adr/0005-final-text-only-document-commit.md)).

Example event:

```json
{
  "type": "partial_transcript",
  "session_id": "voice_123",
  "utterance_id": "utt_7",
  "revision": 14,
  "text": "The application architecture should",
  "stable_prefix_bytes": 21,
  "started_at_ms": 1280,
  "ended_at_ms": 3640
}
```

The client **must** ignore events older than the highest revision already
rendered.

## Utterance boundaries

Initially combine:

- FluidAudio endpoint information when available
- Local voice-activity detection (optional; evaluate in research Q10)
- Explicit push-to-talk release
- Maximum utterance duration
- Configurable silence timeout

Suggested initial defaults:

| Parameter | Default |
|-----------|---------|
| Minimum speech duration | 150 ms |
| Silence required to finalize | 500–800 ms |
| Maximum utterance duration | 30–60 s |
| Push-to-talk release | Immediate finalization |

Tune through usability testing.

## Offline final re-decode

At finalization:

1. Freeze the utterance audio buffer.
2. Stop accepting audio for that utterance.
3. Run offline decoding over the complete utterance.
4. Apply punctuation and text normalization.
5. Apply glossary correction.
6. Parse explicit formatting or command phrases
   ([voice-commands.md](./voice-commands.md)).
7. Emit one authoritative final transcript.
8. Discard raw audio unless the user enabled retention
   ([privacy-security.md](./privacy-security.md)).

Example final event:

```json
{
  "type": "final_transcript",
  "session_id": "voice_123",
  "utterance_id": "utt_7",
  "replaces_revision": 14,
  "text": "The application architecture should separate audio capture from inference.",
  "decode_mode": "offline",
  "duration_ms": 5120,
  "processing_ms": 92
}
```

## Failure handling

| Condition | Policy |
|-----------|--------|
| Streaming fails, offline succeeds | Prefer offline; no user-visible degrade |
| Offline fails, stable streaming exists | Fall back to latest stable streaming output **with** visible degraded-quality state |
| Both paths fail | Fail the utterance; do not insert text |
| No recognized speech | Complete with empty result; do not insert placeholder junk |
| Session exceeds max duration | Finalize current utterance or cancel with explanation |
| Model unavailable | Fail preparing/listening with recovery UI |
| Client disconnects during finalization | Daemon finishes decode; drops result if no subscriber, or queues briefly for Quick Note recovery only |

**Recommended policy:**

- Prefer offline output.
- Fall back to stable streaming only with a visible degraded-quality state.
- **Never** silently commit a known-incomplete transcript.

When final text differs significantly from provisional text, replace the full
provisional range atomically and avoid flicker where possible (research Q12).

## Security implications

- Provisional events **must not** be persisted.
- Telemetry **must not** include transcript content by default.

## Testing requirements

- State transition table
- Revision ordering / stale drop
- Offline replaces provisional
- Degraded fallback visibility
- Empty-speech completion

## Open questions

- Stable partial-token quality (research Q2)
- Dual streaming/offline memory cost (research Q3–Q4)
- Endpoint API surface (research Q9)
- Provisional vs final divergence UX (research Q12)

## Acceptance criteria

- [ ] State machine is enforced in code
- [ ] Final event always carries provenance fields
- [ ] Degraded fallback is visible when used
- [ ] Incomplete transcripts are never silently committed
