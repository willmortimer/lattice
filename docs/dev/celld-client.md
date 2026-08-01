# celld client (Lattice → Cell)

Connect/HTTP client in `crates/lattice-cell-client` and agentd host glue for the
guest **hydrate → run → collect → propose** loop against a running `celld`.
Roles follow KernelFS vocabulary only: `input` / `work` / `output` (guest mounts
`/input`, `/work`, `/output`).

## Overview

```text
KernelFS role host paths → KernelFSHydrationPlan
  → ApplyCell + StartCell (volumes from plan; optional OCI bundle)
  → Invoke lattice.runtime.v1 HydrateProjection   (mirror inputs into guest)
  → Invoke lattice.runtime.v1 RunTask
  → Invoke cell.mirror.v1 CollectOutput           (mirror /output back)
  → OutputFileMap (path → bytes/sha256)
  → propose_resource drafts (agentd run_cell_task / dogfood binaries)
```

Dogfood and `run_cell_task` end in **≥1** reviewable `propose_resource` draft
under a workspace prefix (default `Reports/`). Nothing applies silently — ADR
0063.

## Mirror vs live-bind

Two layers stack; do not conflate them.

| Layer | Mechanism | What moves |
| --- | --- | --- |
| **Mirror (guest protocol)** | `HydrateProjection` + `CollectOutput` | File bytes cross the guest session boundary via `lattice.runtime.v1` / `cell.mirror.v1`. Always used by `lattice-cell-client::run_projection`. |
| **Live-bind (host volume sources)** | `VolumeAttachment.source` on Apply/Start | Host directories the OCI runtime or microVM provider bind-mounts at `/input`, `/work`, `/output`. Writes land on the host immediately — no copy-back at task end. |

**Firecracker / microVM (default):** Lattice creates an ephemeral temp tree
(`{tmpdir}/input`, `{tmpdir}/output`, optional `work`). Hydrate still mirrors
workspace files into the guest; collect mirrors `/output` back. Volume sources
point at the temp dirs.

**Mac OCI (`execution_mode: oci`):** Volume `source` paths must sit under the
ivisor worker VirtioFS share root (Cell
[`docs/28-oci-agent-mount-contract.md`](https://github.com/willmortimer/cell/blob/main/docs/28-oci-agent-mount-contract.md)).
Lattice **materializes + live-exports** KernelFS roles under `agent-share` via
`lattice-agentd::kernelfs_export::export_oci_roles_under_agent_share` (macOS
path). Hydrate/Collect mirror semantics are unchanged — live-bind only affects
where the guest's role mounts are rooted on the host.

**Linux OCI (`execution_mode: oci`):** Same helper exports role dirs under
`/run/kernelfs` or `$XDG_RUNTIME_DIR/kernelfs/{run_id}/{input,work,output}` via
`kernelfs_linux::export_live`. No `CELL_VZ_RUNTIME_DIR` — gVisor/`runsc` binds
host paths directly at Prepare (Cell § gVisor row in
[`docs/28-oci-agent-mount-contract.md`](https://github.com/willmortimer/cell/blob/main/docs/28-oci-agent-mount-contract.md)).

Do **not** set `CELL_OCI_AGENT_MOUNT_COPY=1` when proving live-bind; that forces
copy-into-rootfs and hides VirtioFS behavior.

## Environment

| Variable | Required | Purpose |
| --- | --- | --- |
| `CELLD_BASE_URL` | **yes** (client + `run_cell_task`) | celld Connect/HTTP origin (no trailing slash), e.g. `http://127.0.0.1:8080`. Fails closed when unset — no localhost guess. |
| `LATTICE_API_BASE_URL` | live dogfood / agent tools | latticed HTTP API (e.g. `http://127.0.0.1:18787`). Injected when `latticed` supervises `lattice-agentd`. |
| `LATTICE_AUTH_TOKEN` | live dogfood / agent tools | Bearer token for `propose_resource` (same as daemon handshake). |
| `CELL_VZ_RUNTIME_DIR` | Mac OCI live | Parent of `ivisor-worker-<cellId>/agent-share`. Must match `cell-host-macos` runtime. **Not used on Linux OCI.** |
| `CELL_OCI_IVISOR_WORKSPACE` | Mac OCI alt | When `CELL_VZ_RUNTIME_DIR` unset, runtime resolves to `$CELL_OCI_IVISOR_WORKSPACE/vz-runtime`. **Mac only.** |
| `XDG_RUNTIME_DIR` | Linux OCI optional | When set, kernelfs export parent is `$XDG_RUNTIME_DIR/kernelfs`; else `/run/kernelfs`. |
| `CELL_OCI_IVISOR_INTERIM` | celld (Mac lab) | Set `1` on celld to select ivisor-interim OCI provider (`celld --backend=vz`). |
| `CELL_OCI_IVISOR_SYNC` | celld (Mac lab) | `guest` (preferred when CellOS has `tar`/`gzip`) or `orbctl` fallback. |
| `CELL_VZ_HELPER_SOCKET` | celld (Mac lab) | Path to `cell-host-macos` helper socket. |
| `CELL_VZ_IMAGES_DIR` | celld (Mac lab) | Staged **lattice** aarch64 CellOS artifacts (`profile-manifest.json` → `"profile":"lattice"`). |
| `CELL_FC_*` / `DEVCELL_FC_*` | Firecracker lab | Guest kernel, rootfs, jailer, vsock paths — see `scripts/cell-firecracker-dogfood.sh --help`. |
| `CELL_DOGFOOD_WORKSPACE` | optional | Default workspace root for live dogfood when `--workspace` omitted. |

## KernelFS export for OCI live-bind

Helper: `export_oci_roles_under_agent_share` in
`crates/lattice-agentd/src/kernelfs_export.rs`. Used by
`scripts/cell-mac-oci-dogfood.sh`, `scripts/cell-linux-oci-dogfood.sh`,
`cell-firecracker-dogfood` (OCI branch), and `run_cell_task` when
`executionMode=oci`.

### macOS (VirtioFS agent-share)

Locked layout (volume sources stay under the VirtioFS share root so Cell remap
works without API changes):

```text
{CELL_VZ_RUNTIME_DIR}/ivisor-worker-<cell-id>/agent-share/
  .kernelfs-runs/{run_id}/
    input/
    work/
    output/
```

- `run_id` defaults to `--projection-id` / `taskId` in dogfood and tool paths.
- `VolumeAttachment.source` values are nested per run:
  `agent-share/.kernelfs-runs/{run_id}/input`, `…/output`, optional `…/work`.
  Concurrent runs on one cell do not share flat symlink names.
- Re-export for the same `run_id` is idempotent (materialize refreshes that
  run tree only).

### Linux (gVisor / runsc)

Export parent defaults to `/run/kernelfs` or `$XDG_RUNTIME_DIR/kernelfs`:

```text
{export_parent}/{run_id}/
  input/
  work/
  output/
```

- `run_id` defaults to `--projection-id` / `taskId` (same as macOS).
- `CELL_VZ_RUNTIME_DIR` is **not** required; `vz_runtime_dir` on
  `OciKernelfsExportRequest` is ignored.
- Volume `source` values are `layout.input`, `layout.work`, `layout.output` from
  `kernelfs_linux::export_live`.

Implementation sketch (both platforms):

```rust
// crates/lattice-agentd/src/kernelfs_export.rs
export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
    vz_runtime_dir,
    cell_id,
    run_id,
    input_mounts,      // workspace files → /input
    host_path_roots,   // workspace + agent_share in allow_roots
    with_work,
    include_secrets: false,
})?;
```

## Network egress (microVM vs OCI)

`KernelFSHydrationPlan` defaults to `network_deny_all: true` → `networks[].egress:
none` on Apply. Enforced on **Linux Firecracker** (guest without a NIC).

OCI backends (`execution_mode: oci` / `EXECUTION_MODE_OCI`) **reject**
`egress: none` at Apply. `lattice-cell-client` never silently sends deny-all to
OCI:

| Mode | `network_deny_all: true` (default) |
| --- | --- |
| microVM / Firecracker | Attach deny-all network |
| OCI | **Omit** network attachments (stderr warning) |

Use `with_network_deny_all(false)` or dogfood `--allow-network` only when OCI
egress is explicitly acceptable.

Set `ProjectionRunRequest.execution_mode` to `EXECUTION_MODE_OCI` (or `"oci"`)
for OCI cells. Optional `oci_bundle_path` is forwarded to `CellSpec`.

```sh
export CELLD_BASE_URL=http://127.0.0.1:8080
cargo test -p lattice-cell-client
cargo test -p lattice-agentd --test cell_propose
```

## Firecracker dogfood (`scripts/cell-firecracker-dogfood.sh`)

Default lane: Firecracker microVM temp role dirs. Same loop as agentd
`run_cell_task` / `run_cell_task_and_propose`. Default guest argv copies
`input/hello.txt` → `/output/out.txt` and proposes under `Reports/`.

| Mode | Command | Requires |
| --- | --- | --- |
| CI / default | `scripts/cell-firecracker-dogfood.sh` or `--dry-run` | Rust toolchain (mocked celld + latticed via `cell_propose` tests) |
| Lab / live (microVM) | `… --live` | `CELLD_BASE_URL`, `LATTICE_API_*`, Firecracker guest media, `celld --backend=firecracker` |
| Lab / live (OCI, Mac) | `scripts/cell-mac-oci-dogfood.sh --live …` or `… --live --execution-mode=oci` | `celld --backend=vz` + kernelfs export under `CELL_VZ_RUNTIME_DIR` (§ macOS OCI) |
| Lab / live (OCI, Linux) | `scripts/cell-linux-oci-dogfood.sh --live …` | `celld` gVisor/`runsc` + kernelfs export under `/run/kernelfs` (no VZ env) |

**Dry-run (no live celld):**

```sh
scripts/cell-firecracker-dogfood.sh --dry-run
scripts/cell-mac-oci-dogfood.sh --dry-run   # same mocked tests
scripts/cell-linux-oci-dogfood.sh --dry-run
```

**Live Firecracker lab** — start celld with `--backend=firecracker` and
`lattice-runtime` profile (Cell `scripts/lattice-cell-loop.sh` for
`CELL_FC_KERNEL`, `CELL_FC_ROOTFS`, jailer/vsock vars), then:

```sh
export CELLD_BASE_URL=http://127.0.0.1:8080
export LATTICE_API_BASE_URL=http://127.0.0.1:18787
export LATTICE_AUTH_TOKEN=…
scripts/cell-firecracker-dogfood.sh --live \
  --workspace /path/to/workspace \
  --hydrate input/hello.txt
```

## Linux OCI (`scripts/cell-linux-oci-dogfood.sh`)

Linux product beat: same hydrate → run → collect → **≥1** `propose_resource`
loop as Mac OCI, but KernelFS role dirs export under `/run/kernelfs` (or
`$XDG_RUNTIME_DIR/kernelfs`) via `kernelfs_linux::export_live`. Wrapper always
sets `--execution-mode=oci` and does **not** require `CELL_VZ_RUNTIME_DIR`.

**Live OCI sketch (Linux lab / gVisor):**

```sh
export CELLD_BASE_URL=http://127.0.0.1:8080
export LATTICE_API_BASE_URL=http://127.0.0.1:18787 LATTICE_AUTH_TOKEN=…
# optional: export XDG_RUNTIME_DIR=/run/user/$UID
# start celld with Linux gVisor OCI backend (runsc on PATH)
scripts/cell-linux-oci-dogfood.sh --live \
  --oci-bundle-path /path/to/oci-bundle \
  --workspace /path/to/ws --hydrate input/hello.txt
```

| Flag / env | Purpose |
| --- | --- |
| `--oci-bundle-path` | Host OCI bundle (`config.json` + rootfs) |
| `CELLD_BASE_URL` | celld Connect/HTTP origin |
| `XDG_RUNTIME_DIR` | Optional export parent override (`$XDG_RUNTIME_DIR/kernelfs`) |
| `--with-work` | Export and mount `{run_id}/work` |
| `--allow-network` | `with_network_deny_all(false)` when OCI egress is OK |

## Mac OCI (`scripts/cell-mac-oci-dogfood.sh`)

First product beat: Lattice drives a Mac Cell through `CELLD_BASE_URL` →
hydrate → run → collect → **≥1** `propose_resource`, using OCI + VirtioFS
agent-share (not Firecracker). Wrapper always sets `--execution-mode=oci` and
calls the shared `cell-firecracker-dogfood` binary.

This exercises **`GuestSessionService.Invoke`** → `lattice.runtime.v1` (not
`cellctl exec` alone). VirtioFS agent-share can PASS via Exec while RunTask
still fails if Invoke framing or CellOS lattice agent media is wrong.

**Invoke framing:** `lattice-cell-client` sends Connect **enveloped** bodies for
`/cell.v1.GuestSessionService/Invoke` (`application/connect+json`). Raw JSON
under that content type makes celld reject with
`protocol error: promised N bytes in enveloped message`. Unary Apply/Start stay
`application/json`.

**CellOS lattice artifacts (required for RunTask):** ivisor-interim boots a
CellOS VZ worker running `cell-agent`. Stage **lattice** aarch64 media under
`CELL_VZ_IMAGES_DIR` so `profile-manifest.json` has `"profile":"lattice"` and
advertises `lattice.runtime.v1` / `cell.mirror.v1` (Cell
[`docs/10-macos-local-backend.md`](https://github.com/willmortimer/cell/blob/main/docs/10-macos-local-backend.md)
§ Lattice profile prove). A busybox OCI bundle is fine as the **container**
rootfs; it does not replace CellOS lattice worker artifacts.

Live-bind proof and helper wiring: Cell
[`docs/mac-live-bind-demo.md`](https://github.com/willmortimer/cell/blob/main/docs/mac-live-bind-demo.md).
OCI bind remap at Start: Cell
[`docs/28-oci-agent-mount-contract.md`](https://github.com/willmortimer/cell/blob/main/docs/28-oci-agent-mount-contract.md).

**Live OCI sketch (Apple Silicon lab):**

```sh
# Cell side (separate terminals):
#   ./scripts/macos-oci-bundle.sh   # → /tmp/cell-oci-bundles/cell_mac_live_bind
#   # stage lattice CellOS under CELL_VZ_IMAGES_DIR
#   CELL_OCI_IVISOR_INTERIM=1 CELL_OCI_IVISOR_WORKSPACE=/tmp/cell-oci-bundles \
#     CELL_VZ_RUNTIME_DIR=/tmp/cell-oci-bundles/vz-runtime \
#     cell-host-macos --socket /tmp/cell-host-macos.sock &
#   celld --backend=vz --http-dev --vz-helper-socket /tmp/cell-host-macos.sock

export CELLD_BASE_URL=http://127.0.0.1:8080
export LATTICE_API_BASE_URL=http://127.0.0.1:18787
export LATTICE_AUTH_TOKEN=…
export CELL_VZ_RUNTIME_DIR=/tmp/cell-oci-bundles/vz-runtime

scripts/cell-mac-oci-dogfood.sh --live \
  --oci-bundle-path /tmp/cell-oci-bundles/cell_mac_live_bind \
  --workspace /path/to/workspace \
  --hydrate input/hello.txt
```

| Flag / env | Role |
| --- | --- |
| `--oci-bundle-path` | Host OCI bundle (`config.json` + rootfs) |
| `CELL_VZ_RUNTIME_DIR` or `--vz-runtime-dir` | Parent of `ivisor-worker-*/agent-share` |
| `--with-work` | Export and mount `.kernelfs-runs/{run_id}/work` |
| `--allow-network` | `with_network_deny_all(false)` when OCI egress is OK |

Secrets stay opt-in via `LATTICE_WASI_SECRET_HANDLES` / tool `secretHandlesJson`.
Dogfood does not inject secrets or enable ambient network.

## Concurrent OCI dogfood

ApplyCell `SpecDigest` includes `volumes[].source`. Two OCI dogfood runs that share
the same `cell_id` but export different projection volume trees will collide with
HTTP 409 on the second Apply.

**Rule:** one `cell_id` = one mount set = one OCI sandbox. Distinct concurrent
runs need distinct cell ids.

For OCI (`execution_mode: oci`), dogfood binaries default to a unique effective
cell id:

```text
{base_cell_id}_{projection_id}
```

Example: `--cell-id cell_dogfood --projection-id proj_run_a` → celld cell id
`cell_dogfood_proj_run_a`. Helper: `lattice_cell_client::oci_run_cell_id`.

Pass `--shared-cell-id` to keep the literal `--cell-id` (legacy single-sandbox
reuse — only when volume sources are unchanged or you accept Apply conflicts).

**Concurrent recipe (Mac OCI lab):**

```sh
# Terminal A
scripts/cell-mac-oci-dogfood.sh --live \
  --oci-bundle-path /tmp/cell-oci-bundles/cell_mac_live_bind \
  --workspace /path/to/ws_a --projection-id proj_run_a

# Terminal B (same base --cell-id, different projection)
scripts/cell-mac-oci-dogfood.sh --live \
  --oci-bundle-path /tmp/cell-oci-bundles/cell_mac_live_bind \
  --workspace /path/to/ws_b --projection-id proj_run_b
```

Effective cell ids: `cell_dogfood_proj_run_a` and `cell_dogfood_proj_run_b`.
Each gets its own ivisor worker / kernelfs export tree under a distinct
`ivisor-worker-<cellId>/agent-share` (Mac) or `{export_parent}/{run_id}/` tree
(Linux). `run_id` still tracks the projection (`proj_run_a`, `proj_run_b`).

Linux OCI uses the same suffix rule via `scripts/cell-linux-oci-dogfood.sh`.

## agentd tool: `run_cell_task`

When `CELLD_BASE_URL` is set, `lattice-agentd` registers an extra host tool.

| Tool | When available | What it does |
| --- | --- | --- |
| `run_cell_task` | `CELLD_BASE_URL` configured | Apply/start → hydrate → run → collect → `propose_resource` per collected `/output` file |

Required: `cellId`, `projectionId`, `argv`, `outputProposalTarget`, and
`workspaceRoot` on `start_run`. Optional: `hydrateResourcePaths`, `profile`
(default `lattice-runtime`), `taskId`, `withWork`, `inputResourceIds`.

**OCI live-bind** (chat path):

| Argument | Purpose |
| --- | --- |
| `executionMode` | `"oci"` (or empty / `"microvm"` for temp dirs) |
| `ociBundlePath` | Host OCI bundle; required when `executionMode=oci` |
| (env, Mac only) `CELL_VZ_RUNTIME_DIR` or `CELL_OCI_IVISOR_WORKSPACE` | Required on macOS — fails closed before contacting celld |
| (env, Linux) `XDG_RUNTIME_DIR` | Optional — export parent `$XDG_RUNTIME_DIR/kernelfs`; else `/run/kernelfs` |

OCI mode calls `export_oci_roles_under_agent_share` with `run_id = taskId`
(defaults to `projectionId`), then passes export paths into
`KernelFSHydrationPlan::from_role_paths`. Default microVM behavior is unchanged.

The tool is **absent** from the model tool list when `CELLD_BASE_URL` is unset.
Collected mirror paths like `output/out.txt` map to workspace proposal paths
under `outputProposalTarget`. See **Provenance** below for propose → accept →
inspect lineage.

Implementation: `crates/lattice-agentd/src/cell_host.rs`,
`crates/lattice-agentd/src/kernelfs_export.rs`,
`crates/lattice-agentd/src/tools.rs` (`dispatch_run_cell_task`).

## Provenance (propose → accept → inspect)

**Propose (review):** `run_cell_task` / dogfood attach `sourceResource`
`cell://{cellId}/{projectionId}` and structured `hydrationInputs` digests (input
path, `contentHash`, optional `resourceId`) to each `propose_resource` draft.
Review surfaces those digests before accept.

**Accept (registry):** Accepting a proposal applies the transaction, then
`lattice-commands` calls `latticefs_core::attach_accept_hydration_lineage` for
each touched path. That mints a [`ResourceVersionId`](../crates/latticefs-core/README.md)
in `.lattice/resource-registry.json` — even when the proposal carried no
`hydrationInputs` (ordinary accepts still get a version stub). When digests were
present on the proposal, they are copied into the registry record as
`hydration_inputs`.

**Inspect (read-back):** After accept, lineage is visible without reopening the
proposal archive:

| Surface | Fields |
| --- | --- |
| Desktop **Inspect → Properties** | `Version ID`, `Hydration inputs` (when non-empty) |
| CLI `lattice resource stat <path>` | `version_id`, `hydration_inputs` |

```sh
lattice resource stat Reports/out.txt
# version_id: <uuid>
# hydration_inputs:
#   - path=input/hello.txt contentHash=sha256:…
```

Registry API: [`resource_stat`](../crates/latticefs-core/README.md) /
[`attach_accept_hydration_lineage`](../crates/latticefs-core/src/stat.rs).

## Non-goals

- Desktop UI / Settings wiring
- `apply_proposal` tool (user reviews in Proposals inbox)
- Fleet / multi-host cell scheduling
- CellOS image builds or OCI bundle packaging
- kernelfs-mac FUSE daemon

## Related

- Cell [`docs/04-api.md`](https://github.com/willmortimer/cell/blob/main/docs/04-api.md) — Connect host services
- Cell [`docs/27-kernelfs-cellspec-hydration.md`](https://github.com/willmortimer/cell/blob/main/docs/27-kernelfs-cellspec-hydration.md) — plan → `VolumeAttachment`
- Cell [`docs/mac-live-bind-demo.md`](https://github.com/willmortimer/cell/blob/main/docs/mac-live-bind-demo.md) — VirtioFS agent-share live-bind
- Cell [`docs/28-oci-agent-mount-contract.md`](https://github.com/willmortimer/cell/blob/main/docs/28-oci-agent-mount-contract.md) — OCI bind remap at Start
- Cell [`docs/10-macos-local-backend.md`](https://github.com/willmortimer/cell/blob/main/docs/10-macos-local-backend.md) — VZ backend + lattice profile
- Cell [`docs/lattice-runtime.md`](https://github.com/willmortimer/cell/blob/main/docs/lattice-runtime.md) / [`mirror-broker.md`](https://github.com/willmortimer/cell/blob/main/docs/mirror-broker.md) — guest invoke JSON
- `crates/lattice-agentd/README.md` — Pioneer tools, WASI guests, OCI export note
- `crates/latticefs-core/README.md` — `ResourceVersionId`, accept lineage, `resource stat`
- ADR 0063 — governed propose/overlay (no silent canonical writes)
