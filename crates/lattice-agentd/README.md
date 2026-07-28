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
| `cancel_run` | stops in-flight run → `run_failed` (`Run cancelled`) |
| `shutdown` | exits after cancelling any active run |

`provider: openai` / `pioneer` currently fail with a clear `run_failed` (no
network). OpenAI Responses wiring lives in `src/responses.rs` as a stub.

Wire shapes match `apps/daemon/src/agent/protocol.rs` (camelCase fields,
snake_case `type` discriminators).

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

## Manual smoke

```sh
cargo build -p lattice-agentd
printf '%s\n' '{"type":"hello","protocolVersion":1}' | ./target/debug/lattice-agentd
# → {"type":"hello_ack","protocolVersion":1}
```

## Tests

```sh
cargo test -p lattice-agentd
```

## Non-goals (this slice)

No Wasmtime sandbox, Seatbelt, live Responses network calls, Lattice tool
bridge, or deletion of `apps/agentd`.
