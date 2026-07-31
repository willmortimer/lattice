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
| Lab / live (OCI, Mac) | `scripts/cell-mac-oci-dogfood.sh --live …` | `celld --backend=vz` + ivisor-interim + agent-share under `CELL_VZ_RUNTIME_DIR` (see § Lattice uses Cells on a Mac) |

**Dry-run (no live celld):**

```sh
scripts/cell-firecracker-dogfood.sh --dry-run
# or Mac-named alias (same mocked tests):
scripts/cell-mac-oci-dogfood.sh --dry-run
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

### Lattice uses Cells on a Mac

First product beat: Lattice drives a Mac Cell through `CELLD_BASE_URL` → hydrate
→ run → collect → **≥1** `propose_resource` draft, using OCI + VirtioFS
agent-share (not Firecracker). Prefer the dedicated wrapper:

```sh
scripts/cell-mac-oci-dogfood.sh --dry-run   # CI-safe
scripts/cell-mac-oci-dogfood.sh --live …    # Apple Silicon lab
```

This is the **product** loop (`GuestSessionService.Invoke` →
`lattice.runtime.v1` HydrateProjection / RunTask / CollectOutput). It is **not**
the same as Cell’s `cellctl exec` live-bind demo: VirtioFS agent-share can PASS
via Exec while RunTask still fails if Invoke framing or CellOS lattice agent
media is wrong.

**Invoke framing (required):** `lattice-cell-client` must send Connect
**enveloped** bodies for `/cell.v1.GuestSessionService/Invoke`
(`application/connect+json`). Raw JSON under that content type makes celld
reject the call with
`protocol error: promised N bytes in enveloped message` (often N≈576939372 from
misreading `{"cel…` as a length). Unary Apply/Start stay `application/json`.

KernelFS role **host** directories for OCI live must sit under the ivisor worker
`agent-share` tree (same contract as Cell live-bind):

```text
${CELL_VZ_RUNTIME_DIR}/ivisor-worker-<cell-id>/agent-share/{input,output[,work]}
```

The dogfood binary creates those dirs and passes them as `VolumeAttachment.source`
when `--execution-mode=oci`. MicroVM live still uses an ephemeral temp tree.

**CellOS lattice artifacts (required for RunTask):** ivisor-interim boots a
CellOS VZ worker that runs `cell-agent`. Stage **lattice** aarch64 media under
`CELL_VZ_IMAGES_DIR` so `profile-manifest.json` has `"profile":"lattice"` and
advertises `lattice.runtime.v1` / `cell.mirror.v1` (see Cell
`docs/10-macos-local-backend.md` § Lattice profile prove). Spec profile default
`lattice-runtime` matches staged `lattice` (`ProfileMatchesSpec`). A busybox OCI
bundle (`cell/scripts/macos-oci-bundle.sh`) is fine as the **container** rootfs;
it does **not** replace CellOS — without lattice worker artifacts, Invoke cannot
serve RunTask. Live-bind demos that only `cellctl exec` never exercise this path.

Live-bind / VirtioFS proof and helper wiring: Cell
[`docs/mac-live-bind-demo.md`](https://github.com/willmortimer/cell/blob/main/docs/mac-live-bind-demo.md)
(agent-share layout, `CELL_VZ_RUNTIME_DIR` must match `cell-host-macos`). Image
staging: Cell `docs/virtiofs-cellos-image.md`. Backend runbook: Cell
`docs/10-macos-local-backend.md` § OCI mode smoke.

**Guest OCI sync:** Prefer `CELL_OCI_IVISOR_SYNC=guest` when the staged CellOS
image has `tar`/`gzip` on PATH. If StartCell fails mid guest-channel sync
(missing gzip/tar), use `CELL_OCI_IVISOR_SYNC=orbctl` (OrbStack ext4 path) until
the image is restaged — VirtioFS live-bind still works either way.

**Live flags (hardware; not required in CI):**

| Flag / env | Role |
| --- | --- |
| `CELLD_BASE_URL` | Running `celld --backend=vz --http-dev` |
| `LATTICE_API_BASE_URL` + `LATTICE_AUTH_TOKEN` | latticed propose |
| `--oci-bundle-path PATH` | Host OCI bundle (`config.json` + rootfs); busybox OK |
| `CELL_VZ_RUNTIME_DIR` or `--vz-runtime-dir` | Parent of `ivisor-worker-*/agent-share` (or derive `$CELL_OCI_IVISOR_WORKSPACE/vz-runtime`) |
| `CELL_OCI_IVISOR_INTERIM=1` | On celld — select ivisor-interim provider |
| `CELL_OCI_IVISOR_WORKSPACE` | Parent of the bundle dir |
| `CELL_OCI_IVISOR_SYNC` | `guest` (preferred) or `orbctl` fallback when guest lacks gzip/tar |
| `CELL_VZ_HELPER_SOCKET` / `CELL_VZ_IMAGES_DIR` | Helper + staged **lattice** CellOS artifacts |
| `--with-work` | Also mount `agent-share/work` |
| `--allow-network` | Explicit OCI egress (`with_network_deny_all(false)`) |
| Do **not** set `CELL_OCI_AGENT_MOUNT_COPY=1` | Forces copy-into-rootfs and hides live-bind |

**Network egress on OCI:** `KernelFSHydrationPlan` defaults to
`network_deny_all: true`. OCI backends reject `egress: none` at Apply. With
`execution_mode: oci`, `lattice-cell-client` **omits** network attachments when
deny-all is still true (stderr warning). Use `--allow-network` only when OCI
egress is explicitly acceptable.

**Secrets:** remain opt-in via existing agentd env
(`LATTICE_WASI_SECRET_HANDLES` / tool arg `secretHandlesJson`). Dogfood does not
inject secrets or enable ambient network — see `crates/lattice-agentd/README.md`.

**Live OCI one-liner sketch (Apple Silicon lab):**

```sh
# Cell side (separate terminals / prior steps):
#   ./scripts/macos-oci-bundle.sh   # → /tmp/cell-oci-bundles/cell_mac_live_bind
#   # stage lattice CellOS under CELL_VZ_IMAGES_DIR (profile-manifest profile=lattice)
#   CELL_OCI_IVISOR_INTERIM=1 CELL_OCI_IVISOR_WORKSPACE=/tmp/cell-oci-bundles \
#     CELL_VZ_RUNTIME_DIR=/tmp/cell-oci-bundles/vz-runtime \
#     CELL_OCI_IVISOR_SYNC="${CELL_OCI_IVISOR_SYNC:-guest}" \
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

Equivalent via the Firecracker script:

```sh
scripts/cell-firecracker-dogfood.sh --live \
  --execution-mode=oci \
  --oci-bundle-path /tmp/cell-oci-bundles/cell_mac_live_bind \
  --vz-runtime-dir /tmp/cell-oci-bundles/vz-runtime \
  --workspace /path/to/workspace
```

**Out of scope for this beat:** `kernelfs-mac` packaging; full hardware proof is
lab-only (document live flags above). Dry-run stays green without Apple Silicon.

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

Structured `hydrationInputs` (`path` + `contentHash` + optional `resourceId`)
persist on the proposal source. When the user accepts/applies the proposal,
LatticeFS mints a `ResourceVersionId` for each accepted path and copies those
digests onto the resource registry entry. Inspect surfaces them via
`lattice resource stat` / `--json` (`hydration_inputs`) and the desktop Inspect
properties panel.

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
- Cell `docs/mac-live-bind-demo.md` — Mac VirtioFS agent-share live-bind contract
- Cell `docs/28-oci-agent-mount-contract.md` — OCI bind remap at Start
- Cell `docs/lattice-runtime.md` / `docs/mirror-broker.md` — guest invoke JSON
- ADR 0063 — governed propose/overlay (no silent canonical writes)
