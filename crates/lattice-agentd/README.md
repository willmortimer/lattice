# lattice-agentd

Rust sidecar that speaks the Phase A agent JSONL protocol over stdio
([ADR 0051](../../docs/decisions/0051-rust-embedded-agent-harness.md)). Desktop /
`latticed` prefer this binary by default and fall back to Node
`apps/agentd/scripts/run.sh` when it is missing.

## Protocol

One JSON object per line on stdin (commands) / stdout (events):

| Command | Event response |
| --- | --- |
| `hello` | `hello_ack` (`protocolVersion: 1`) |
| `health` | `health` (`ok: true`) |
| `start_run` (`provider: fake`) | `run_started` → `message_chunk`(s) → `run_completed` |
| `start_run` (`provider: pioneer`) | Pioneer chat completions SSE → `message_chunk`(s) |
| `start_run` (`provider: openai`) | OpenAI Responses stream → `message_chunk`(s) |
| `cancel_run` | stops in-flight run → `run_failed` (`Run cancelled`) |
| `shutdown` | exits after cancelling any active run |

Wire shapes match `apps/daemon/src/agent/protocol.rs` (camelCase fields,
snake_case `type` discriminators). UI chunks use AI SDK shapes
(`text-start` / `text-delta` / `text-end`).

## Build

```sh
cargo build -p lattice-agentd
# or release (what desktop-release bundles):
cargo build --release -p lattice-agentd
```

Binary: `target/debug/lattice-agentd` or `target/release/lattice-agentd`.

## Discovery (default)

When `LATTICE_AGENTD_BIN` is unset, discovery order is:

1. `target/release/lattice-agentd`
2. `target/debug/lattice-agentd`
3. `lattice-agentd` next to the running `latticed` / app binary (packaged DMG)
4. Node `apps/agentd/scripts/run.sh` (fallback)

Force Node:

```sh
export LATTICE_AGENTD_PREFER_NODE=1
# or:
export LATTICE_AGENTD_BIN="$(pwd)/apps/agentd/scripts/run.sh"
```

## Pioneer (default provider)

```sh
cargo build -p lattice-agentd
unset LATTICE_AGENT_FAKE
export LATTICE_AGENT_PROVIDER=pioneer
export LATTICE_AGENT_MODEL=gpt-5.6-luna   # or gpt-5.6-terra
export PIONEER_API_KEY=…                  # injected by latticed at spawn
```

Stdio smoke:

```sh
printf '%s\n' \
  '{"type":"hello","protocolVersion":1}' \
  '{"type":"start_run","threadId":"t1","runId":"r1","provider":"pioneer","model":"gpt-5.6-luna","prompt":"Say hi in one word"}' \
  '{"type":"shutdown"}' \
  | ./target/debug/lattice-agentd
```

## OpenAI Responses

```sh
export LATTICE_AGENT_PROVIDER=openai
export OPENAI_API_KEY=sk-...
# optional: LATTICE_AGENT_MODEL=gpt-4.1-mini
```

## Manual smoke (fake)

```sh
printf '%s\n' \
  '{"type":"hello","protocolVersion":1}' \
  '{"type":"start_run","threadId":"t1","runId":"r1","provider":"fake","prompt":"hi"}' \
  '{"type":"shutdown"}' \
  | ./target/debug/lattice-agentd
```

## Tests

```sh
cargo test -p lattice-agentd
```
