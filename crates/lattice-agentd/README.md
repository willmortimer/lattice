# lattice-agentd

Opt-in Rust sidecar that speaks the Phase A agent JSONL protocol over stdio
([ADR 0051](../../docs/decisions/0051-rust-embedded-agent-harness.md)). This is
the scaffold for replacing Node `apps/agentd`; it does **not** change default
daemon/desktop discovery.

## Protocol

One JSON object per line on stdin (commands) / stdout (events):

| Command | Event response |
| --- | --- |
| `hello` | `hello_ack` (`protocolVersion: 1`) |
| `health` | `health` (`ok: true`) |
| `start_run` (`provider: fake`) | `run_started` → `message_chunk`(s) → `run_completed` |
| `start_run` (`provider: openai`) | Responses stream → `message_chunk`(s) → `run_completed` / `run_failed` |
| `cancel_run` | stops in-flight run → `run_failed` (`Run cancelled`) |
| `shutdown` | exits after cancelling any active run |

`provider: pioneer` currently fails with a clear `run_failed` (no network).

Wire shapes match `apps/daemon/src/agent/protocol.rs` (camelCase fields,
snake_case `type` discriminators). UI chunks use AI SDK shapes
(`text-start` / `text-delta` / `text-end`).

## Build

```sh
cargo build -p lattice-agentd
```

Binary: `target/debug/lattice-agentd` (or `target/release/lattice-agentd`).

## Opt in via `LATTICE_AGENTD_BIN`

Default discovery still points at Node `apps/agentd`. To exercise this binary
from `latticed` / desktop:

```sh
cargo build -p lattice-agentd
export LATTICE_AGENTD_BIN="$(pwd)/target/debug/lattice-agentd"
# optional: force fake provider end-to-end
export LATTICE_AGENT_PROVIDER=fake
```

Unset `LATTICE_AGENTD_BIN` (and leave discovery alone) to keep the Node path.

## OpenAI Responses (`provider: openai`)

Requires `OPENAI_API_KEY` in the agentd environment (latticed injects it at
spawn when present). Missing key → immediate `run_failed` with a clear message
(no hang / no network).

```sh
cargo build -p lattice-agentd
export LATTICE_AGENTD_BIN="$(pwd)/target/debug/lattice-agentd"
export LATTICE_AGENT_PROVIDER=openai
export OPENAI_API_KEY=sk-...
# optional overrides
# export LATTICE_AGENT_MODEL=gpt-4.1-mini
# export OPENAI_BASE_URL=https://api.openai.com/v1
```

Then start a run from desktop chat (or stdio) with `provider: openai`. Streamed
Responses `response.output_text.delta` events become `message_chunk` events.

Stdio smoke (expects a live key):

```sh
printf '%s\n' \
  '{"type":"hello","protocolVersion":1}' \
  '{"type":"start_run","threadId":"t1","runId":"r1","provider":"openai","model":"gpt-4.1-mini","prompt":"Say hi in one word"}' \
  '{"type":"shutdown"}' \
  | ./target/debug/lattice-agentd
```

## Manual smoke (fake)

```sh
cargo build -p lattice-agentd
printf '%s\n' '{"type":"hello","protocolVersion":1}' | ./target/debug/lattice-agentd
# → {"type":"hello_ack","protocolVersion":1}
```

## Tests

```sh
cargo test -p lattice-agentd
```

OpenAI coverage uses recorded SSE fixtures + wiremock (no live network).

## Non-goals (this slice)

No Wasmtime sandbox, Seatbelt, Pioneer provider, Lattice tool bridge, or
deletion of `apps/agentd`.
