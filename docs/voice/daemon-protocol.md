# Daemon Protocol

## Scope

Versioned local IPC between Tauri clients (and future CLI/helpers) and
`latticed` for voice sessions. Define immediately even if M1–M3 run in-process
([architecture.md](./architecture.md)).

## Transport

Recommended initial transports:

- Unix-domain sockets on macOS and Linux
- Named pipes on Windows (future)
- Length-prefixed binary frames
- MessagePack, CBOR, or Protobuf payloads — **choose in an ADR before locking
  crates**; examples below are schema-shaped, not library-mandating
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

## Event messages

| Event | Purpose |
|-------|---------|
| `ModelStatusChanged` | Download/prepare/warm/unload |
| `SessionReady` | Accepting audio |
| `SpeechStarted` | Endpoint / VAD speech onset |
| `PartialTranscript` | Provisional |
| `StableTranscript` | Stable prefix update |
| `FinalTranscript` | Authoritative |
| `CommandCandidate` | Parsed voice command (pre-exec) |
| `SessionCompleted` | Terminal success |
| `SessionFailed` | Terminal failure |

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

- Exact serialization format (MessagePack vs CBOR vs Protobuf)
- Executable identity attestation quality on macOS

## Acceptance criteria

- [ ] Schema is shared by in-process and daemon modes
- [ ] Auth rejects unauthenticated local peers
- [ ] No audio on public HTTP/MCP surfaces
- [ ] Capability negotiation is tested
