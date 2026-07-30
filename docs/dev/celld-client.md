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

### Network egress (microVM vs OCI)

`KernelFSHydrationPlan` defaults to `network_deny_all: true`, which maps to
`networks[].egress: none` on Apply. That policy is **enforced on Linux
Firecracker** (guest launched without a NIC) and is the right default for
microVM / Firecracker dogfood.

OCI backends (`execution_mode: oci` / `EXECUTION_MODE_OCI`) **reject**
`egress: none` at Apply. The client never silently sends deny-all networks to
OCI:

- Unset `execution_mode` (microVM): deny-all networks are attached as today.
- `execution_mode: oci`: network attachments are **omitted** when
  `network_deny_all` is true (stderr warning). Call
  `with_network_deny_all(false)` explicitly when OCI egress is acceptable.

Set `ProjectionRunRequest.execution_mode` to `EXECUTION_MODE_OCI` (or `"oci"`)
for OCI cells. Optional `oci_bundle_path` is forwarded to `CellSpec`.

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
| Lab / live (microVM) | `scripts/cell-firecracker-dogfood.sh --live` | Running celld (`CELLD_BASE_URL`), latticed (`LATTICE_API_BASE_URL`, `LATTICE_AUTH_TOKEN`), Firecracker guest media |
| Lab / live (OCI, Mac) | `… --live --execution-mode=oci --oci-bundle-path PATH` | `celld --backend=vz` + ivisor-interim env, VirtioFS CellOS image, OCI bundle (see § Mac OCI dogfood) |

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

### Mac OCI dogfood (VirtioFS + ivisor-interim)

After staging a VirtioFS-capable aarch64 CellOS image (Cell
[`virtiofs-cellos-image.md`](https://github.com/willmortimer/cell/blob/main/docs/virtiofs-cellos-image.md)
checklist) and starting `celld --backend=vz` with ivisor-interim enabled, the
same dogfood script can exercise the OCI execution lane instead of Firecracker
microVM.

**Prerequisites (Cell repo, Apple Silicon lab):**

1. Staged `cellos-lattice-artifacts` under `~/.cell/images/cellos-aarch64` (or
   `CELL_VZ_IMAGES_DIR`) with guest `virtiofs`/`fuse` — see Cell
   `docs/virtiofs-cellos-image.md`.
2. `cell-host-macos` running (`CELL_VZ_HELPER_SOCKET`).
3. `celld --backend=vz --http-dev` with OCI ivisor-interim:
   `CELL_OCI_IVISOR_INTERIM=1`, staged bundle root (`CELL_OCI_BUNDLE_ROOT`), and
   ivisor workspace (`CELL_OCI_IVISOR_WORKSPACE`). Full runbook: Cell
   `docs/10-macos-local-backend.md` § OCI mode smoke.
4. A prepared OCI bundle on the host (e.g. `examples/oci-busybox.cell.yaml` in
   Cell). Pass its path via `--oci-bundle-path` or `CellSpec.oci_bundle_path`.

**Network egress on OCI:** `KernelFSHydrationPlan` defaults to
`network_deny_all: true`, which maps to `egress: none` on microVM Apply. OCI
backends reject that at Apply. With `execution_mode: oci`, `lattice-cell-client`
**omits** network attachments when deny-all is still true (stderr warning) — you
do not need to change the plan for a basic OCI dogfood run. Call
`with_network_deny_all(false)` (or `--allow-network` on the dogfood binary) only
when OCI egress is explicitly acceptable.

**Live OCI example:**

```sh
export CELLD_BASE_URL=http://127.0.0.1:8080
export LATTICE_API_BASE_URL=http://127.0.0.1:18787
export LATTICE_AUTH_TOKEN=…
export CELL_OCI_IVISOR_INTERIM=1
export CELL_OCI_BUNDLE_ROOT=/tmp/cell-oci-bundles
export CELL_OCI_IVISOR_WORKSPACE=/tmp/cell-ivisor-interim

scripts/cell-firecracker-dogfood.sh --live \
  --execution-mode=oci \
  --oci-bundle-path /tmp/cell-oci-bundles/cell_oci01 \
  --workspace /path/to/workspace \
  --hydrate input/hello.txt
```

`--execution-mode=oci` sets `ProjectionRunRequest.execution_mode` to
`EXECUTION_MODE_OCI`. Agent mount paths under the worker `agent-share` tree are
remapped to VirtioFS guest paths at OCI Start (Cell
`docs/28-oci-agent-mount-contract.md`).

**Dry-run** (`--dry-run`) stays microVM-only — it runs mocked `cell_propose`
tests and does not require celld or an OCI bundle.

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
