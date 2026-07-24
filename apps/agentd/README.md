# `@lattice/agentd`

Node 22 sidecar that runs the Lattice embedded agent (Phase A / EA2).

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
| `PIONEER_API_KEY` | Required for Pioneer (`https://api.pioneer.ai/v1`) |
| `OPENAI_API_KEY` | Required for direct OpenAI fallback |

`start_run.provider` selects the provider for that run. `LATTICE_AGENT_FAKE=1`
always wins. Provider `fake` (or the force-fake env) streams deterministic
text-delta chunks without calling a model API.

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

Phase A ships a single manager agent with **no tools**. Lattice HTTP tools,
MCP, drafts, and desktop UI arrive in later EA tasks.
