# lattice-agentd

Rust sidecar that speaks the Phase A agent JSONL protocol over stdio
([ADR 0051](../../docs/decisions/0051-rust-embedded-agent-harness.md)). Desktop /
`latticed` prefer this binary by default. Node `apps/agentd` is **opt-in only**
via `LATTICE_AGENTD_PREFER_NODE=1` or an explicit `LATTICE_AGENTD_BIN`.

## Protocol

One JSON object per line on stdin (commands) / stdout (events):

| Command | Event response |
| --- | --- |
| `hello` | `hello_ack` (`protocolVersion: 1`) |
| `health` | `health` (`ok: true`) |
| `start_run` (`provider: fake`) | `run_started` → `message_chunk`(s) → `run_completed` |
| `start_run` (`provider: pioneer`) | Pioneer SSE chat completions → tool trail steps + live `message_chunk` streaming when Lattice HTTP env is set |
| `start_run` (`provider: openai`) | OpenAI Responses stream → tool trail steps + live `message_chunk` streaming when Lattice HTTP env is set |
| `cancel_run` | stops in-flight run → `run_failed` (`Run cancelled`) |
| `shutdown` | exits after cancelling any active run |

Wire shapes match `apps/daemon/src/agent/protocol.rs` (camelCase fields,
snake_case `type` discriminators). UI chunks use AI SDK shapes
(`text-start` / `text-delta` / `text-end`). Pioneer tool rounds use SSE
(`stream: true`) so final answers stream live; `step_started` /
`step_completed` mark model/tool waits.

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

Force Node (escape hatch only):

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

### Host tools (latticed HTTP)

When both are set, Pioneer uses Chat Completions with a Lattice tool loop
(search / read / related / proposals / …) against the daemon localhost API
(same routes as Node `apps/agentd`). `latticed` injects these when it
supervises the sidecar:

| Env | Purpose |
| --- | --- |
| `LATTICE_API_BASE_URL` | Daemon HTTP base (e.g. `http://127.0.0.1:18787`) |
| `LATTICE_AUTH_TOKEN` | Bearer token (same as daemon handshake) |

`start_run` may include `workspaceId` / `workspaceRoot`; tools bind to that
workspace when arguments omit them. Without the Lattice env pair, Pioneer
falls back to chat-only streaming and prints a one-time stderr warning.

### KernelFS WASI → proposals

Sandboxed WASI guests write under KernelFS `/output`. The host helper in
`wasi_host` materializes the run dir, runs `_start`, collects proposal drafts,
and pushes each draft via `POST /v1/proposals/propose_resource` (same body shape
as the host `propose_resource` tool). Lattice search/read/related stay host HTTP
tools — they are not exposed inside the guest.

Pioneer exposes this path as the `run_wasi_guest` tool when `workspaceRoot` is
bound on `start_run`:

| Argument | Purpose |
| --- | --- |
| `wasmPath` | Workspace-relative `.wasm` module |
| `inputsJson` | JSON array of `{hostPath,guestPath}` input mounts |
| `outputProposalTarget` | Workspace prefix for proposed `/output` paths (e.g. `Reports`) |
| `runId` | Optional run label (defaults to a timestamped id) |

The tool returns proposal ids/paths only — it never applies. Human review stays
in the Proposals inbox.

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
# optional: LATTICE_AGENT_MODEL=gpt-5-nano
# Prefer ecosystem sops + `nxr desktop-dev` (auto-injects secrets/ai.env).
```

### Host tools (latticed HTTP)

When `LATTICE_API_BASE_URL` and `LATTICE_AUTH_TOKEN` are set, OpenAI uses the
Responses API with a Lattice tool loop (same tools as Pioneer: search / read /
related / proposals / `run_wasi_guest` / …). Tool defs are mapped to Responses
flat function shape (`type` / `name` / `description` / `parameters`). Max 8
assistant→tool rounds; `step_started` / `step_completed` mark model and tool
waits; final answers stream as AI SDK `text-*` chunks.

`start_run` may include `workspaceId` / `workspaceRoot`; tools bind to that
workspace when arguments omit them. Without the Lattice env pair, OpenAI falls
back to text-only Responses streaming and prints a one-time stderr warning.

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
