# latticed

Long-lived Lattice daemon: Unix-domain control plane, optional semantic indexing,
optional voice-host supervision, and an authenticated **localhost-only** HTTP /
MCP context API.

## Voice host (D5)

`latticed` can supervise `lattice-voice-host` the same way it supervises
`lattice-embed-host`. Voice RPCs on the control-plane socket
(`PrepareModel`, `StartVoiceSession`, `PushAudioChunk`, `FinishUtterance`,
`UpdateSessionContext`, `CancelVoiceSession` / `EndVoiceSession`,
`GetVoiceCapabilities`, `VoiceHostStatus`, `UnloadVoiceModel`) are forwarded to
the host. Partial / final / gap / model-status events are fanned out to
subscribed clients.

Session policy: **one active voice session per daemon**. A second
`StartVoiceSession` fails with `voice_session_busy` until the first session is
ended or cancelled.

### Environment

| Variable | Purpose |
| --- | --- |
| `LATTICE_VOICE_FAKE=1` | Spawn a fake-backend `lattice-voice-host` (tests / CI) |
| `LATTICE_VOICE_HOST_BIN` | Path to the `lattice-voice-host` binary |
| `LATTICE_VOICE_HOST_SOCKET` | Existing host UDS (connect only), or socket path when spawning |

Without these, voice RPCs return `voice_unavailable` (not `unimplemented`).

```sh
# Example: supervised fake host for local testing
LATTICE_VOICE_FAKE=1 \
  LATTICE_VOICE_HOST_BIN=./target/debug/lattice-voice-host \
  cargo run -p lattice-daemon -- --auth-token dev-token --api-port 0
```

## Local HTTP API (D6)

Binds **`127.0.0.1` only** (never `0.0.0.0`). Default port: `18787`
(`--api-port 0` disables).

Authenticate every `/v1/*` call with the daemon instance token:

```http
Authorization: Bearer <token>
```

or

```http
X-Lattice-Token: <token>
```

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Liveness (no auth) |
| `POST` | `/v1/search` | Hybrid (default) or FTS search with provenance |
| `POST` | `/v1/read` | Bounded page/resource read by path |
| `POST` | `/v1/related` | Backlinks + FTS related stub |
| `POST` | `/v1/build_context` | Bounded excerpts; `export_policy=ask/deny` omitted or flagged |

Bodies accept `workspaceId` (open session) or `root` (opens a read session).
Payloads are capped (`maxBytes` / hit limits). Hybrid hits with
`export_policy` of `ask` or `deny` redact excerpts; `build_context` never
exfiltrates `ask` text freely (`needsConsent: true`).

This surface is **read-oriented**. Mutations continue through the semantic
command / Unix protocol path — the API is not a second write authority.

### Example

```sh
cargo run -p lattice-daemon -- --auth-token dev-token --api-port 18787

curl -s -X POST http://127.0.0.1:18787/v1/search \
  -H 'authorization: Bearer dev-token' \
  -H 'content-type: application/json' \
  -d '{"root":"/path/to/workspace","query":"notes","mode":"fts"}'
```

## MCP stdio

Minimal JSON-RPC MCP adapter exposing the same four tools:

```sh
LATTICE_AUTH_TOKEN=dev-token cargo run -p lattice-daemon -- mcp
```

Tools: `search`, `read`, `related`, `build_context`. Prefer the HTTP contract
for automated tests; use MCP when wiring Claude Desktop / other stdio clients.

Example Claude Desktop snippet:

```json
{
  "mcpServers": {
    "lattice": {
      "command": "latticed",
      "args": ["mcp"],
      "env": { "LATTICE_AUTH_TOKEN": "dev-token" }
    }
  }
}
```

## Tests

```sh
cargo build -p lattice-voice-host
cargo test -p lattice-daemon
```

Voice contract tests spawn a fake `lattice-voice-host` (from
`LATTICE_VOICE_HOST_BIN`, `PATH`, or `target/debug`).

## Lifecycle and keep-running (D7)

By default `latticed` shuts down after the last client disconnects and a
short idle period (30 seconds). This keeps on-demand launches from leaving a
background process running unintentionally.

### Preference

The desktop profile stores the preference in
`~/Lattice/Settings/desktop.yaml`:

```yaml
services:
  keepServicesRunning: true
```

When `keepServicesRunning` is `true`, the daemon remains running after clients
disconnect until it receives `SIGTERM`/`SIGINT` or an explicit stop. The
desktop shell can set this preference; the on-demand spawn helper
([`spawn_latticed`](src/spawn.rs)) reads it automatically.

### CLI overrides

```sh
# Stay resident after clients disconnect
latticed --keep-services-running

# Short idle timeout (seconds) when keep-running is off
latticed --idle-shutdown-secs 5
```

Environment overrides (tests / launchers):

- `LATTICE_KEEP_SERVICES_RUNNING=1`
- `LATTICE_IDLE_SHUTDOWN_SECS=0.5`

### Clean shutdown

On exit (signal, idle timeout, or explicit stop), `latticed`:

1. Stops the localhost HTTP API and semantic workers
2. Releases held workspace leases and stops index watchers
3. Removes the Unix socket file under
   `~/Library/Application Support/Lattice/run/latticed.sock` (macOS) or the
   platform equivalent

### Disable / uninstall

There is **no login item or LaunchAgent** in this phase. To stop the daemon:

- Quit clients that hold connections, then wait for the idle timeout (default),
  or send `SIGTERM` to the `latticed` process
- Set `services.keepServicesRunning: false` in desktop settings if you do not
  want it to stay resident between sessions
- Remove the socket manually only if a process crashed without cleaning up:
  `rm ~/Library/Application\ Support/Lattice/run/latticed.sock`

A user-controlled login item for always-on Quick Note and schedules remains
future work (see `docs/architecture/latticed-daemon-migration-plan.md` Phase D7).
