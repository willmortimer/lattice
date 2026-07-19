# Audio Capture

## Scope

Canonical audio format and capture pipeline. Clients own capture
([adr/0004](./adr/0004-client-owned-audio-capture.md)); inference consumes
normalized PCM only.

## Canonical format

One internal format for the shared protocol:

| Property | Value |
|----------|-------|
| Encoding | Signed PCM |
| Sample rate | 16,000 Hz |
| Channels | Mono |
| Sample format | Prefer the cleanest FluidAudio bridge contract: either 16-bit little-endian **or** normalized `f32`. Pick one in M0/M1 and keep the protocol runtime-neutral via an explicit `AudioSampleFormat` enum. |
| Frame duration | 20 ms |
| Transport chunk duration | 40–100 ms |

Clients **must** resample and downmix before sending chunks to the voice
service. Providers **must not** assume device-native rates on the wire.

## Capture pipeline

```text
CoreAudio input
    ↓
Input-device native sample rate
    ↓
Channel downmix
    ↓
Resampling to 16 kHz mono
    ↓
Optional conservative preprocessing
    ↓
Ring buffer
    ↓
Chunk sequencing
    ↓
Local IPC (or in-process handoff)
```

Audio capture **must never** block the UI thread.

## Pre-roll buffering

Maintain approximately **250–500 ms** of rolling audio before dictation is
activated.

When the user presses the shortcut:

1. Include recent pre-roll audio.
2. Begin the live stream.
3. Mark the actual activation timestamp separately.
4. Let endpoint detection discard irrelevant leading silence.

This prevents clipped first syllables. Exact pre-roll duration is tuned in
research Q11.

## Backpressure

Documented requirements:

| Concern | Policy |
|---------|--------|
| Maximum queued audio duration | Bound (suggested starting point: 5–10 s of pending chunks). Exact value set after M0 latency measurements. |
| Dropped frames | Emit gap events with sequence discontinuity; do not silently continue |
| Sequence numbers | Monotonic `u64` per session |
| Gap detection | Client and server validate contiguous sequences |
| Sustained backpressure | Cancel the session with a visible error rather than unbounded growth |
| Degraded provisional decode | May reduce provisional update rate before dropping audio |

## Audio preprocessing

Start conservatively.

**May** include:

- Mono downmix
- Sample-rate conversion
- DC offset removal if necessary
- Basic gain normalization when proven beneficial

**Must not** initially add:

- Aggressive denoising
- Automatic gain control that distorts speech
- Echo cancellation without a playback use case
- Voice isolation with undocumented latency

Every preprocessing stage **must** be benchmarked against recognition quality
rather than assumed to help ([observability-testing.md](./observability-testing.md)).

## Interfaces

See `AudioChunk` in [daemon-protocol.md](./daemon-protocol.md).

## Security implications

- Raw PCM **must not** travel over Lattice’s public HTTP or MCP APIs.
- Buffers **must** be zeroed or released promptly after utterance finalization
  ([privacy-security.md](./privacy-security.md)).

## Testing requirements

- Resampler correctness across common device rates (44.1 / 48 / 96 kHz)
- Pre-roll inclusion on push-to-talk
- Sequence gap detection
- UI-thread non-blocking under load
- Device switch mid-session

## Open questions

- Exact sample type for FluidAudio bridge (i16 vs f32)
- Pre-roll duration (research Q11)
- Whether a separate VAD improves segmentation (research Q10)

## Acceptance criteria

- [ ] One canonical on-wire format is fixed before M2
- [ ] Pre-roll is measurable in fixture tests
- [ ] Backpressure cancels rather than unbounded queue growth
- [ ] No capture work on the UI thread
