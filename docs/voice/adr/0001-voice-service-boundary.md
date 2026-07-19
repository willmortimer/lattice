# ADR 0001: Voice-service boundary

## Status

Accepted (voice subsystem).

## Context

Dictation could be implemented as an editor-only feature inside the Tauri
WebView, or as an application service reusable by Quick Note, CLI, and future
clients. Lattice already treats optional background work via `latticed` and
requires specialized hot paths outside React.

## Decision

Treat voice recognition as an **application service** rather than an
editor-specific feature. Define a daemon-compatible protocol immediately;
allow in-process implementation for early prototypes; move model ownership to
`latticed` before shipping global Quick Note.

## Consequences

- Editor remains independent of the inference runtime.
- Quick Note and future clients can share the service.
- Additional IPC and lifecycle complexity is accepted.
- See [architecture.md](../architecture.md) and [daemon-protocol.md](../daemon-protocol.md).
