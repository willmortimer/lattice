# ADR 0041: Daemon IPC uses Protobuf over a private Unix-domain socket

## Status

Accepted.

## Context

Lattice intends a long-lived `latticed` process for workspace watching,
indexing, model supervision, local API/MCP, and durable jobs
([docs/04](../04-system-architecture.md),
[docs/18](../18-automation-events-workflows-and-daemon.md),
[latticed migration plan](../architecture/latticed-daemon-migration-plan.md)).
The desktop already shares MVP handlers with a development HTTP bridge
([ADR 0037](0037-localhost-bridge-shares-handlers-with-tauri.md)), but that
bridge is not a secure stateful control plane.

Voice planning already requires a versioned local IPC with length-prefixed
frames ([docs/voice/daemon-protocol.md](../voice/daemon-protocol.md)). The
daemon control plane needs the same properties for commands, events, leases,
and bulk planes (audio, Arrow), without using Rust in-memory layout or
unauthenticated localhost HTTP as the privileged transport.

## Decision

- Use **Protobuf** encoded with **`prost`** as the daemon control/event
  protocol, carried over **length-delimited frames** on a **private
  Unix-domain socket** (macOS path under Application Support).
- Version the protocol explicitly; include request IDs, deadlines,
  cancellation, idempotency keys, and sequenced events.
- Authenticate with user-only socket permissions plus a per-instance token
  (and peer credentials where available). Do not expose an unauthenticated
  TCP control surface.
- Keep JSON for human diagnostics, MCP/HTTP APIs, and configuration — not
  for PCM, Arrow batches, or high-frequency control envelopes.
- Support temporary **embedded** and **daemon** clients over one
  `LatticeClient` contract; a workspace lease prevents dual writers.

## Consequences

- New crates: `lattice-protocol`, `lattice-client`, later `lattice-runtime`
  and `apps/daemon`.
- `apps/bridge` remains a development adapter and must call through the
  client/runtime rather than becoming a second write authority.
- Protocol evolution follows protobuf compatibility rules; golden
  encode/decode and framing tests are required before migrating mutations.
