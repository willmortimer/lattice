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
| Sample format | **`f32` (Float32)** for the FluidAudio bridge — M0 confirmed this is the cleanest provider contract ([RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md)). The protocol keeps `AudioSampleFormat::I16Le` in the enum for runtime neutrality; clients targeting FluidAudio **should** resample to Float32 before handoff. |
| Frame duration | 20 ms |
| Transport chunk duration | 40–100 ms (FluidAudio streaming uses **160 ms** chunks at the provider boundary per M0) |

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
Convert to Float32 (canonical for FluidAudio)
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
| Maximum queued audio duration | Bound (suggested starting point: 5–10 s of pending chunks). M0 warm first partial **~405 ms** on M2; queue bounds still need product tuning. |
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
- Float32 normalization (canonical for FluidAudio)
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
- Float32 output matches FluidAudio fixture expectations
- Pre-roll inclusion on push-to-talk
- Sequence gap detection
- UI-thread non-blocking under load
- Device switch mid-session

## Open questions

- Pre-roll duration (research Q11)
- Whether a separate VAD improves segmentation beyond Parakeet EOU (research Q10)

## Acceptance criteria

- [x] Canonical on-wire format fixed for FluidAudio bridge (Float32 @ 16 kHz mono)
- [x] Desktop macOS path uses native capture + packed frames (`captured_at_ns`, sequence); WebView JSON `number[]` PCM retired
- [ ] Pre-roll is measurable in fixture tests
- [x] Backpressure cancels rather than unbounded queue growth (`BoundedFrameQueue` in Tauri pump)
- [x] No capture work on the UI thread (native `AVAudioEngine` path)
