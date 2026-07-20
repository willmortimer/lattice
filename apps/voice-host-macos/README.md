# lattice-voice-host

Isolated voice inference helper for Lattice (ADR 0043 / D5).

The host runs as a **separate process**, listens on a **private Unix-domain
socket**, and never writes workspace content. `latticed` will supervise it in
`d5_daemon_voice`; for now you can launch it directly.

## Protocol choice

The host reuses **`lattice-protocol` length-delimited `Envelope` frames** and the
existing voice request / response / event messages (same shapes as the daemon
UDS control plane). This keeps PCM as packed `bytes` inside
`PushAudioChunkRequest` and lets a future daemon supervisor translate with a
thin adapter.

Host-admin RPCs that are not part of the workspace voice plane:

| RPC | Purpose |
| --- | --- |
| `voice_host_status` | Backend, model status, session/chunk metrics |
| `unload_voice_model` | Drop models + active sessions |

These use reserved request fields (`40` / `41`) on the shared schema so the
private host socket does not invent a second framing style (contrast:
`lattice-embed-host` keeps a dedicated protobuf package because embeddings are
not on the daemon Envelope today).

## Capabilities

| RPC | Purpose |
| --- | --- |
| `health` | Liveness + backend name |
| `voice_host_status` | Install/model state + metrics |
| `prepare_model` | Prepare/warm ASR models |
| `get_voice_capabilities` | Feature negotiation |
| `start_voice_session` | Begin session |
| `push_audio_chunk` | Packed PCM ingress |
| `finish_utterance` | Emit final transcript events |
| `update_session_context` | Glossary / document context |
| `cancel_voice_session` / `end_voice_session` | Abort / clean shutdown |
| `unload_voice_model` | Unload models |

Streaming partials / finals arrive as sequenced `Event` envelopes on the same
connection (same demux pattern as `LatticeClient` / daemon).

## Backends

### `fake` (default, always available)

Deterministic transcripts via `lattice_voice::NullSpeechProvider`. Used by CI
and unit/integration tests. **No model download.**

```sh
cargo run -p lattice-voice-host -- serve \
  --socket /tmp/lattice-voice-host.sock \
  --backend fake
```

### `fluidaudio` (optional feature)

```sh
cargo build -p lattice-voice-host --features fluidaudio
```

Requires macOS + the `lattice-voice-macos` Swift bridge (`link-bridge`). Point
`--model-cache-dir` at a prepared Parakeet cache when running for real.

Keep `fake` as the default so `cargo test -p lattice-voice-host` stays offline.

`latticed` honors the `fake` flag from `VoiceProviderMode` / `LATTICE_VOICE_FAKE`:
`--backend fake` for tests, `--backend fluidaudio` when spawning a feature-gated
host without fake. Pass `--model-cache-dir` via `LATTICE_VOICE_MODEL_CACHE`.

## Tests

```sh
cargo test -p lattice-voice-host
```

Tests use the fake backend and tiny PCM fixtures only.

## Crash isolation

Clients talk over UDS. If the host process exits, the client surfaces an I/O /
connection error; workspace state in other processes is unaffected.

## Client library

`VoiceHostClient` (in this crate) is the EmbedHostClient analogue used by
`latticed` supervision (`apps/daemon/src/voice_host.rs`).
