# Daemon Protocol

## Scope

Versioned local IPC between Tauri clients (and future CLI/helpers) and
`latticed` for voice sessions. Define immediately even if M1–M3 run in-process
([architecture.md](./architecture.md)).

## Transport

Locked by [ADR 0041](../decisions/0041-daemon-ipc-protobuf.md) and
[ADR 0043](../decisions/0043-voice-ownership-in-latticed.md):

- Unix-domain sockets on macOS and Linux
- Named pipes on Windows (future)
- Length-prefixed Protobuf [`Envelope`](../../crates/lattice-protocol/proto/lattice.proto)
  frames (`prost`) shared with the daemon control plane
- Voice requests and events nest in `Request` / `Response` / `Event` oneofs —
  not a parallel stream type
- PCM travels as `bytes` (packed Float32 / I16LE) inside `PushAudioChunkRequest`
- **No** TCP listener by default

Raw audio **must not** travel over Lattice’s public HTTP or MCP APIs.

## Authentication

Even local clients **must** authenticate.

Possible mechanisms (combine as needed):

- Per-installation local token
- Socket filesystem permissions
- Client executable identity where available
- Session-specific nonce

The protocol **must** reject unrelated local processes attempting to inject
audio or commands.

## Request messages

| Message | Purpose |
|---------|---------|
| `PrepareModel` | Download/verify/warm |
| `GetVoiceCapabilities` | Feature negotiation |
| `StartVoiceSession` | Begin session with config + context |
| `PushAudioChunk` | Stream PCM |
| `FinishUtterance` | Trigger offline finalization |
| `UpdateSessionContext` | Glossary, document id, command mode |
| `CancelVoiceSession` | Abort |
| `EndVoiceSession` | Clean shutdown |
| `VoiceHostStatus` | Host metrics / model state (voice-host UDS; daemon may proxy) |
| `UnloadVoiceModel` | Unload ASR models in voice-host |

## Event messages

| Event | Purpose |
|-------|---------|
| `ModelStatusChanged` | Download/prepare/warm/unload |
| `SessionReady` | Accepting audio |
| `SpeechStarted` | Endpoint / VAD speech onset |
| `PartialTranscript` | Provisional |
| `StableTranscript` | Stable prefix update |
| `FinalTranscript` | Authoritative (`FinalizationMode` per voice ADR 0007) |
| `CommandCandidate` | Parsed voice command (pre-exec) |
| `SessionCompleted` | Terminal success |
| `SessionFailed` | Terminal failure |
| `AudioGap` | Sequence discontinuity / dropped capture frames |

## Audio chunk structure

```rust
pub struct AudioChunk {
    pub session_id: VoiceSessionId,
    pub sequence: u64,
    pub captured_at_ns: u64,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub sample_format: AudioSampleFormat,
    pub payload: Bytes,
}
```

Canonical rates/channels: [audio-capture.md](./audio-capture.md).

## Protocol versioning

Include:

- Protocol version
- Provider version
- Model version
- Feature negotiation via `SpeechCapabilities`
- Unknown-field behavior (ignore unknowns on read where safe; fail on required missing)
- Minimum client version
- Graceful downgrade when a capability is unavailable

## In-process adapter

During prototype, the same request/event types **may** be dispatched in-process
without a socket. The daemon mode **must** remain a thin transport over the
same message schema.

## Security implications

See [privacy-security.md](./privacy-security.md). Untrusted local process
injection is a primary threat.

## Testing requirements

- Schema compatibility across versions
- Auth rejection of foreign clients
- Sequence gap handling
- Daemon restart mid-session recovery
- Large chunk backpressure

## Open questions

- Executable identity attestation quality on macOS

## Acceptance criteria

- [x] Schema is shared by in-process and daemon modes (Protobuf Envelope)
- [ ] Auth rejects unauthenticated local peers
- [x] No audio on public HTTP/MCP surfaces (PCM only on private UDS Envelope)
- [ ] Capability negotiation is tested
