# lattice-kernel

Out-of-process Jupyter/`ipykernel` supervision for Lattice desktop (Phase-4 J2).

Rust never speaks ZMQ and never embeds CPython ([ADR 0009](../../docs/decisions/0009-dual-python-and-jupyter-runtime.md)).
A Python child (`bridge/lattice_ipykernel_bridge.py`) owns `jupyter_client` +
`ipykernel` and talks to Rust over **stdio JSON-lines** (one JSON object per line).

## Session API

| Method | Behavior |
|---|---|
| `KernelSessionMap::start` | Capability-gate `cwd` under `workspace_root`; spawn bridge; wait for `ready` |
| `execute` | Send `execute`; collect `stream` / `execute_result` / `error` until `done` |
| `interrupt` | Send `interrupt` (safe while execute is in flight) |
| `shutdown` / drop | Send `shutdown` when possible, then kill the child |

## Bridge protocol

**Requests** (Rust → bridge stdin):

```json
{"type":"execute","id":"<req>","code":"print(1)"}
{"type":"interrupt","id":"<req>"}
{"type":"shutdown","id":"<req>"}
```

**Responses** (bridge stdout → Rust):

```json
{"type":"ready"}
{"type":"stream","id":"<req>","name":"stdout","text":"1\n"}
{"type":"execute_result","id":"<req>","data":{"text/plain":"2"}}
{"type":"error","id":"<req>","ename":"ValueError","evalue":"...","traceback":["..."]}
{"type":"done","id":"<req>","status":"ok"}
{"type":"bridge_error","id":"<req>","message":"..."}
```

Diagnostics from the bridge go to stderr (not protocol lines).

## Python discovery (crate-local)

1. Prefer `uv` on `PATH`: `uv run --with ipykernel --with jupyter_client -- python <bridge>`
2. Else `python3` on `PATH` (caller must have packages installed)

Shared `lattice-env` resolution lands in J4; this crate keeps discovery local.

## Manual bridge invoke

```sh
# With uv (pulls deps ephemerally):
uv run --with ipykernel --with jupyter_client -- \
  python crates/lattice-kernel/bridge/lattice_ipykernel_bridge.py

# Or system Python with packages installed:
python3 crates/lattice-kernel/bridge/lattice_ipykernel_bridge.py
```

Then write a request line and read response lines until `done`.

## Tests

```sh
cargo test -p lattice-kernel
```

Protocol framing and session-map logic use a mock stdio bridge (no Jupyter).
Live ipykernel coverage is `#[ignore]` behind `LATTICE_KERNEL_LIVE=1`.
