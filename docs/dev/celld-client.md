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

## Firecracker dogfood script

`scripts/cell-firecracker-dogfood.sh` exercises the full loop (hydrate → run →
collect → `propose_resource`) and asserts **≥1** reviewable proposal.

| Mode | Command | Requires |
| --- | --- | --- |
| CI / default | `scripts/cell-firecracker-dogfood.sh` or `--dry-run` | Rust toolchain only (mocked celld + latticed via `cell_propose` tests) |
| Lab / live | `scripts/cell-firecracker-dogfood.sh --live` | Running celld (`CELLD_BASE_URL`), latticed (`LATTICE_API_BASE_URL`, `LATTICE_AUTH_TOKEN`), Firecracker guest media |

**Dry-run (no live celld):**

```sh
scripts/cell-firecracker-dogfood.sh --dry-run
```

**Live Firecracker lab** — start celld with `--backend=firecracker` and
`lattice-runtime` profile (see Cell `scripts/lattice-cell-loop.sh` for guest
kernel/rootfs env: `CELL_FC_KERNEL`, `CELL_FC_ROOTFS`, jailer/vsock vars), then:

```sh
export CELLD_BASE_URL=http://127.0.0.1:8080
export LATTICE_API_BASE_URL=http://127.0.0.1:18787
export LATTICE_AUTH_TOKEN=…
scripts/cell-firecracker-dogfood.sh --live \
  --workspace /path/to/workspace \
  --hydrate input/hello.txt
```

Live mode runs `cell-firecracker-dogfood` (same path as agentd
`run_cell_task` / `run_cell_task_and_propose`). Default guest argv copies
`input/hello.txt` to `/output/out.txt` and proposes under `Reports/`.
Override with `--` and guest `argv`, or set `CELL_DOGFOOD_WORKSPACE` instead of
`--workspace`.

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
as WASI `run_wasi_guest`. Non-UTF-8 collected bytes use `contentBase64` on
`propose_resource` (via shared `propose_output_drafts*` / `ContentKind::Bytes`).

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
