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
```

Roles are KernelFS only: `input` / `work` / `output` (guest mounts `/input`,
`/work`, `/output`). Do not invent parallel mount vocabulary.

```sh
export CELLD_BASE_URL=http://127.0.0.1:8080
cargo test -p lattice-cell-client
```

## Non-goals

- Desktop UI / Settings wiring
- Propose/overlay into LatticeFS (see follow-on L2)
- Fleet / multi-host cell scheduling
- Requiring VirtioFS or host bind mounts into the guest
- CellOS image builds or OCI bundle packaging

## Related

- Cell `docs/04-api.md` — Connect host services
- Cell `docs/27-kernelfs-cellspec-hydration.md` — plan → `VolumeAttachment`
- Cell `docs/lattice-runtime.md` / `docs/mirror-broker.md` — guest invoke JSON
