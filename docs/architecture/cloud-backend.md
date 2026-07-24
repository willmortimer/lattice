# Cloud backend layout

**Status:** CB0 skeleton  
**See also:** [`cloud-backend-dag.md`](./cloud-backend-dag.md)

## Crate split

| Crate | Role |
| --- | --- |
| [`lattice-cloud`](../../crates/lattice-cloud) | Shared config, HTTP router, and future auth/storage/MCP modules |
| [`lattice-server`](../../apps/server) | Thin binary: load env config, bind, serve |

CB0 exposes `GET /healthz` and reserves on-disk paths for SQLite metadata and
filesystem objects. Auth, share/publish/backup APIs, and MCP stubs land in
later DAG slices (CB1–CB3).
