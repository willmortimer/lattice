# kernelfs

This is the public Lattice harness crate for **KernelFS** — the scoped execution projection for WASI and agent runs (`/input`, `/work`, `/output`, `/tmp`). It was adapted from the private ecosystem package at `packages/kernelfs` so `lattice-agentd` and other public crates can depend on it via path. See `docs/architecture/kernelfs-mvp.md` in the ecosystem repo for the full MVP spec.

## Capabilities (fail closed)

`ExecutionManifest.capabilities` is deny-by-default. Preview1 `_start` guests today only get:

| Supported | Notes |
| --- | --- |
| Preopens | `/input` (ro), `/work`, `/output`, `/tmp` |
| Fuel / epoch | Via `WasmtimeLimits` / `WasiRunOptions` |

| Rejected until implemented | Behavior |
| --- | --- |
| Non-empty `capabilities.network.allow` | `materialize` errors with `UnsupportedCapabilities` |
| Any `capabilities.secrets` | Same fail-closed error |

Empty `network.allow` and empty `secrets` are allowed (schema defaults).

## Output bridge

`collect_output_commit_plan` walks `/output` and allowlisted `work_promote_paths`, returning `OutputCommitEntry` values with:

- `content: Vec<u8>` (binary-safe)
- `kind: text | bytes` (UTF-8 classification)
- `content_type_hint` (host-oriented MIME hint)

Mapping those entries onto latticed `propose_*` APIs remains a host concern.
