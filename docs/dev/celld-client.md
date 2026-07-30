# celld client (Lattice → Cell)

Minimal Connect/HTTP client in `crates/lattice-cell-client` for the guest
hydrate → run → collect loop against a running `celld`. Prefer this path over
VirtioFS live binds; Cell already materializes KernelFS trees under the guest
data volume via `lattice.runtime.v1` / `cell.mirror.v1`.

## Environment

| Variable | Required | Purpose |
| --- | --- | --- |
| `CELLD_BASE_URL` | **yes** | celld Connect/HTTP origin (no trailing slash), e.g. `http://127.0.0.1:8080` |

The client **fails closed** when `CELLD_BASE_URL` is unset or blank — there is
no default localhost guess.

## Public loop

```text
KernelFS role paths → KernelFSHydrationPlan
  → ApplyCell (volumes/networks from plan) + StartCell
  → Invoke lattice.runtime.v1 HydrateProjection
  → Invoke lattice.runtime.v1 RunTask
  → Invoke cell.mirror.v1 CollectOutput
  → OutputFileMap (path → bytes/sha256)
  → propose_resource drafts (agentd `run_cell_task`)
```

Roles are KernelFS only: `input` / `work` / `output` (guest mounts `/input`,
`/work`, `/output`). Do not invent parallel mount vocabulary.

```sh
export CELLD_BASE_URL=http://127.0.0.1:8080
cargo test -p lattice-cell-client
cargo test -p lattice-agentd --test cell_propose
```

## agentd tool: `run_cell_task`

When `CELLD_BASE_URL` is set, Rust `lattice-agentd` registers an extra host
tool:

| Tool | When available | What it does |
| --- | --- | --- |
| `run_cell_task` | `CELLD_BASE_URL` configured | Apply/start cell → hydrate → run → collect → `propose_resource` for each collected `/output` file |

Required arguments: `cellId`, `projectionId`, `argv`, `outputProposalTarget`.
Optional: `hydrateResourcePaths` (workspace-relative files hydrated under
`input/`), `profile` (default `lattice-runtime`), `taskId`.

The tool is **absent** from the model tool list when `CELLD_BASE_URL` is unset.
Direct calls return a clear error naming the env var.

Collected mirror paths like `output/out.txt` are mapped to workspace proposal
paths under `outputProposalTarget` (e.g. `Reports/out.txt`). Provenance uses
`sourceResource` `cell://{cellId}/{projectionId}` — same propose/overlay path
as WASI `run_wasi_guest`.

Implementation: `crates/lattice-agentd/src/cell_host.rs` +
`crates/lattice-agentd/src/tools.rs` (`dispatch_run_cell_task`).

## Non-goals

- Desktop UI / Settings wiring
- `apply_proposal` tool (user reviews in Proposals inbox)
- Fleet / multi-host cell scheduling
- Requiring VirtioFS or host bind mounts into the guest
- CellOS image builds or OCI bundle packaging

## Related

- Cell `docs/04-api.md` — Connect host services
- Cell `docs/27-kernelfs-cellspec-hydration.md` — plan → `VolumeAttachment`
- Cell `docs/lattice-runtime.md` / `docs/mirror-broker.md` — guest invoke JSON
- ADR 0063 — governed propose/overlay (no silent canonical writes)
