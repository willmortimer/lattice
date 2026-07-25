# `@lattice/agentd`

Node 22 sidecar that runs the Lattice embedded agent (Phase B tools over
latticed's authenticated localhost HTTP API — same semantics as `latticed mcp`).

`latticed` supervises this process. Commands arrive as JSONL on **stdin**;
lifecycle and UI message events leave as JSONL on **stdout**. Diagnostics go
to **stderr** only.

## Quick start (fake provider)

```bash
LATTICE_AGENT_FAKE=1 pnpm --filter @lattice/agentd start
```

Then send JSONL commands on stdin, for example:

```json
{"type":"hello","protocolVersion":1}
{"type":"start_run","threadId":"t1","runId":"r1","provider":"fake","model":"fake-model","prompt":"Hello"}
{"type":"shutdown"}
```

## Environment

| Variable | Meaning |
| --- | --- |
| `LATTICE_AGENT_FAKE` | When `1`/`true`, force the hermetic fake provider (no network) |
| `LATTICE_AGENT_PROVIDER` | Default provider: `pioneer` (default), `openai`, or `fake` |
| `LATTICE_AGENT_MODEL` | Default model id |
| `LATTICE_AGENTD_BIN` | Path to Node/tsx entry or packaged executable (see `scripts/run.sh`) |
| `PIONEER_API_KEY` | Required for Pioneer (`https://api.pioneer.ai/v1`) |
| `OPENAI_API_KEY` | Required for direct OpenAI fallback |
| `LATTICE_API_BASE_URL` | Localhost API base (e.g. `http://127.0.0.1:18787`) for tools |
| `LATTICE_AUTH_TOKEN` | Bearer token for the localhost API (same as daemon handshake) |

`start_run.provider` selects the provider for that run. `LATTICE_AGENT_FAKE=1`
always wins. Provider `fake` (or the force-fake env) streams deterministic
text-delta chunks without calling a model API (and without tools).

When `latticed` supervises `agentd`, it injects `LATTICE_AUTH_TOKEN` and
`LATTICE_API_BASE_URL` from its config. Desktop-spawned daemons listen on
`127.0.0.1:18787` by default.

## Lattice tools (Phase B)

Attached OpenAI Agents SDK tools mirror MCP names:

`get_current_context`, `search`, `read`, `related`, `build_context`,
`get_dataset_schema`, `profile_dataset`, `create_proposal`, `list_proposals`,
`get_proposal`, `propose_page`, `propose_resource`, `propose_workflow`,
`propose_interface`, `propose_artifact`.

`start_run` may include `workspaceId` / `workspaceRoot`; tools default to that
binding. There is **no** apply tool — proposals stay in the inbox.

## Scripts

```bash
pnpm --filter @lattice/agentd start
pnpm --filter @lattice/agentd test
pnpm --filter @lattice/agentd typecheck
```

## Protocol

Wire types live in `@lattice/agent-protocol` (`PROTOCOL_VERSION = 1`):

- Commands: `hello`, `start_run`, `cancel_run`, `health`, `shutdown`
- Events: `hello_ack`, `run_started`, `message_chunk`, `run_completed`,
  `run_failed`, `health`

## Desktop integration

The Tauri shell does **not** spawn `agentd` directly. `latticed` supervises the
sidecar (or uses an in-process fake backend). See
[`docs/architecture/embedded-agent.md`](../../docs/architecture/embedded-agent.md).

**Pioneer + tools:**

```sh
export LATTICE_AGENTD_BIN="$(pwd)/apps/agentd/scripts/run.sh"
export LATTICE_AGENT_PROVIDER=pioneer
export LATTICE_AGENT_MODEL=MiniMaxAI/MiniMax-M3
export PIONEER_API_KEY=…                # never commit
pnpm --filter @lattice/desktop tauri:dev:novoice
```

Quit any stale `latticed` / Lattice.app first so spawn picks up agent + API env.

### Pioneer model tool-loop notes

Many Gemini Flash / Claude IDs fail the **second** chat-completions turn after
tool results (`invalid argument`, or `annotations: Extra inputs are not
permitted`). The UI then looks stalled on the first `TOOL` row if the failure
is not surfaced.

Verified multi-turn tool loops on Pioneer today: `MiniMaxAI/MiniMax-M3`,
`gpt-5.4-nano`. Prefer `pioneer/auto` when your account enables the router.
Probe with `apps/agentd/scripts/probe-tool-loop.mts`.
