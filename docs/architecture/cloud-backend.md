# Cloud backend layout

**Status:** CB0 skeleton (moved 2026-07-27)  
**See also:** [`cloud-backend-dag.md`](./cloud-backend-dag.md)

The cloud backend source now lives in the private
[`lattice-ecosystem`](https://github.com/willmortimer/lattice-ecosystem)
repository:

| Path in lattice-ecosystem | Role |
| --- | --- |
| `crates/lattice-cloud` | Shared config, HTTP router, auth/storage/MCP modules |
| `apps/server` | Thin `lattice-server` binary |
| `infra/cloud` | NixOS VPS flake and host modules |

This public client repository retains local-first desktop / daemon / CLI code
only. Do not reintroduce `lattice-server` here without an explicit open-core
boundary decision.
