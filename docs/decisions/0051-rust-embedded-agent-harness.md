# ADR 0051: Rust embedded agent harness replaces Node `agentd`

## Status

Accepted (implementation on `feat/adr-0066-rust-agent-harness`).  
Supersedes the Node runtime choice in [ADR 0044](0044-embedded-agent-sidecar.md).

## Context

ADR 0044 shipped a Node 22 `agentd` sidecar for an early embedded agent. The
product direction is a Rust harness with sandboxed tools, still supervised by
`latticed`, without changing the desktop chat protocol.

## Decision

1. **Replace Node `agentd`.** On merge to `main`, the embedded agent is a Rust
   binary supervised by `latticed` (daemon sibling or Tauri sidecar process).
   There is no Node fallback.
2. **Keep the wire protocol.** Existing `@lattice/agent-protocol` JSONL and
   `latticed` supervision remain so React / assistant-ui need not change.
3. **Authority unchanged.** The agent creates proposals through `latticed`;
   it does not apply proposals or write canonical workspace state directly
   ([ADR 0007](0007-semantic-command-transaction-core.md),
   [ADR 0008](0008-ai-is-an-external-client.md),
   [ADR 0018](0018-explicit-capabilities-and-proposed-writes.md)).
4. **Sandbox tools, not the LLM client.** Tool/FS execution runs in Wasmtime
   with a short-term read-only input + overlay output projection mapped into
   the existing proposal UI. The OpenAI **Responses API** client runs in the
   Rust host outside Wasmtime. macOS Seatbelt wraps the tool worker for v1.
5. **OpenAI only for v1.** Further provider abstraction is deferred.

## Consequences

- Desktop packaging points `LATTICE_AGENTD_BIN` at the Rust binary.
- Contributors treat `apps/agentd` (Node) as removed after merge.
- Deeper sandbox / KernelFS / multi-provider details may evolve in follow-on
  ADRs without breaking the JSONL contract.
