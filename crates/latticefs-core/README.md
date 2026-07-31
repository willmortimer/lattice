# latticefs-core

Public LatticeFS types for the open client: stable resource identity, authority,
materialization state, and a workspace path registry.

## Public core vs private cloud

| Layer | Crate / package | Repo |
| --- | --- | --- |
| **Core** (identity, authority, materialization, local registry) | `latticefs-core` | Public `lattice` |
| **Cloud** (hosted namespace, sync, replication, fleet policy) | `latticefs-cloud` (future) | Private `lattice-ecosystem` |

The free Mac build and open client depend only on this crate. Cloud orchestration
stays private so product strategy and hosted ops are not pulled into the public
tree.

## Registry

Workspace-relative paths map to stable [`ResourceId`](src/types.rs) values under
`.lattice/resource-registry.json`. Rename and move update the path key but keep
the same identity.

Default posture for ordinary local files:

- `AuthorityMode::Local`
- `MaterializationState::Pinned`

## API

| Function / type | Purpose |
| --- | --- |
| [`ResourceId`](src/types.rs) | Stable identity, independent of path |
| [`NamespaceEntry`](src/types.rs) | Path → resource binding |
| [`ResourceStat`](src/types.rs) | Authority + materialization snapshot |
| [`HydrationInputDigest`](src/types.rs) | Accept-lineage input path + content hash |
| [`NamespaceRegistry`](src/registry.rs) | Persisted path registry |
| [`resource_stat`](src/stat.rs) | Inspect one path |
| [`attach_accept_hydration_lineage`](src/stat.rs) | On proposal accept: mint version + store digests |
| [`materialize_to_cloud`](src/stat.rs) | PUT→GET verify and set `authority: cloud` |
| [`open_cloud_authoritative_bytes`](src/stat.rs) | GET cloud bytes; errors on failure (no local fallback) |
| [`fetch_cloud_blob`](src/cloud.rs) | Low-level cloud GET with optional hash verify |
| [`CloudBlobClient`](src/cloud.rs) | Authenticated blob PUT/GET trait |
| [`InMemoryCloudBlobClient`](src/cloud.rs) | In-memory test double |
| [`HttpCloudBlobClient`](../lattice-cloud-client/src/blob.rs) | Production HTTP impl (`lattice-cloud-client`) |

## Tests

```sh
cargo test -p latticefs-core
```
