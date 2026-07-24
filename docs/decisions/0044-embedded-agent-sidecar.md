# ADR 0044: Embedded agent sidecar (`agentd`)

## Status

Accepted.

## Context

Lattice treats AI as an interchangeable external client ([ADR 0008](0008-ai-is-an-external-client.md)): models do not own canonical workspace state, and mutations flow through the semantic command core ([ADR 0007](0007-semantic-command-transaction-core.md)). The product still needs an optional embedded agent in the desktop shell for grounded workspace assistance, reviewable proposals, and spatial UI.

The embedded agent requires fast-moving JavaScript agent SDKs (OpenAI Agents SDK, MCP clients, streaming orchestration) while `latticed` remains Rust infrastructure that owns workspace sessions, export policy, proposals, and MCP server behavior. Collapsing both into one process would couple Node dependency churn to the workspace daemon and blur authority boundaries.

The full architecture — frontend stack, tool contracts, cell migration, and hybrid cloud — is documented in [`docs/architecture/embedded-agent.md`](../architecture/embedded-agent.md).

## Decision

Adopt a **Node 22 `agentd` sidecar** supervised by `latticed` as the Phase A embedded agent runtime:

1. **`latticed` supervises `agentd`.** Process lifecycle, workspace sessions, secret injection, and the agent event sink remain in the Rust daemon. Tauri does not spawn `agentd` directly.
2. **The desktop never talks to a model provider.** React streams through `latticed` (Tauri ordered Channel → custom AI SDK `ChatTransport` → assistant-ui). Provider API keys are injected at `agentd` launch and never reach the webview.
3. **`agentd` uses the OpenAI Agents SDK** for agent loops, tool selection, streaming, sessions, and MCP client integration. Pioneer (OpenAI-compatible) is the initial provider; direct OpenAI is a fallback.
4. **Proposals remain Rust command authority.** `agentd` may create drafts and proposals through `latticed` tools; proposal application stays a desktop-authorized semantic transaction. `agentd` does not receive an `apply_proposal` tool.
5. **The embedded agent is optional.** It is a sidecar orchestrator, not the workspace. External MCP clients, CLI, and other agents continue to use the same public `latticed` surfaces ([ADR 0008](0008-ai-is-an-external-client.md)).

Later phases may run the same `agentd` service inside a persistent workspace cell without changing the React protocol or tool contract.

## Consequences

- A Node or provider SDK failure can restart `agentd` without taking down `latticed`.
- The desktop agent UI uses assistant-ui and AI SDK primitives; Lattice-specific surfaces (trail, overlays, drafts, proposal split view) live in `@lattice/ui` or the desktop agent module.
- Implementers follow Phase A in the architecture doc before adding Lattice tools, spatial anchors, Draft Studio, or cell migration.
- New irreversible choices about agent authority, connector credential storage, or cloud routing should update the architecture doc and, when cross-cutting, land a follow-on ADR.
