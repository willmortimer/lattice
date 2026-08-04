# Yrs remote provider (client)

Thin remote transport for binary Yrs updates keyed by page `ResourceId`
(ADR 0055 / S8). Local `.lattice/collab/<uuid>/` journal remains source of
truth. Markdown materialization is unchanged (checkpoint/idle/close).

## Transport choice

Reuse existing cloud blob APIs (S2/S4/S5):

- `PUT /v1/blobs/{sidecar_id}` with `X-Lattice-Workspace-Id` + optional `If-Match`
- `GET /v1/blobs/{sidecar_id}`

**Sidecar ResourceId** (not the page id): UUID v5 over
`lattice.collab.yrs-snapshot.v1:{page_id}` with the URL namespace. This avoids
overwriting the Markdown open-format blob that sync-heads tracks for the page.

Payload (`LYRS` v1):

| Field | Size | Notes |
| --- | --- | --- |
| magic | 4 | `LYRS` |
| version | 1 | `1` |
| flags | 1 | `0` |
| reserved | 2 | `0` |
| page_id | 16 | UUID bytes of the page ResourceId |
| yrs_update | rest | Full lib0 v1 update from empty SV |

## Client path

1. Desktop Labs toggles: collaborative page editor + remote Yrs provider.
2. When remote is enabled and cloud is signed in, `openCollabSession` polls:
   pull sidecar → apply to Y.Doc (and daemon journal) → push merged full state.
3. Handlers: `push_collab_remote_snapshot` / `pull_collab_remote_snapshot` in
   `lattice-handlers` via Tauri commands.
4. Core types + in-memory store: `lattice_collab::remote`.

No new server routes required. Ecosystem sync-heads may list the sidecar as an
extra blob; it is not materialized as a workspace file.

## Out of scope (fence)

Capture, presence, app_lock. Awareness stays local Tauri fan-out only.
