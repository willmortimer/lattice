# Yrs remote provider (client)

Thin remote transport for binary Yrs updates keyed by page `ResourceId`
(ADR 0055 / S8). Local `.lattice/collab/<uuid>/` journal remains source of
truth. Markdown materialization is unchanged (checkpoint/idle/close).

## Transport choice

Reuse existing cloud blob APIs (S2/S4/S5):

- `PUT /v1/blobs/{sidecar_id}` with `X-Lattice-Workspace-Id` + optional `If-Match`
- `GET /v1/blobs/{sidecar_id}`

**Snapshot sidecar ResourceId** (not the page id): UUID v5 over
`lattice.collab.yrs-snapshot.v1:{page_id}` with the URL namespace. This avoids
overwriting the Markdown open-format blob that sync-heads tracks for the page.

Payload (`LYRS` v1) — **default** remote catch-up path (S8):

| Field | Size | Notes |
| --- | --- | --- |
| magic | 4 | `LYRS` |
| version | 1 | `1` |
| flags | 1 | `0` |
| reserved | 2 | `0` |
| page_id | 16 | UUID bytes of the page ResourceId |
| yrs_update | rest | Full lib0 v1 update from empty SV |

**Append-log sidecar ResourceId**: UUID v5 over
`lattice.collab.yrs-log.v1:{page_id}` (distinct prefix so snapshot and log
blobs never collide).

Payload (`LYRL` v1) — incremental catch-up without replacing the whole Y.Doc
each poll (available for desktop wire-up; compaction is caller-driven):

| Field | Size | Notes |
| --- | --- | --- |
| magic | 4 | `LYRL` |
| version | 1 | `1` |
| flags | 1 | `0` |
| reserved | 2 | `0` |
| page_id | 16 | UUID bytes of the page ResourceId |
| base_hash | 32 | SHA-256 of the snapshot this log is based on, or 32 zero bytes if unknown |
| updates | rest | Concatenated `u32le` length + lib0 v1 update bytes |

Log limits: 256 updates or 1 MiB of update payload bytes. When exceeded,
`append_update` returns `LogNeedsCompact`; callers should write a fresh `LYRS`
snapshot and start a new log (optionally recording the snapshot content hash in
`base_hash`).

## Client path

1. Desktop Labs toggles: collaborative page editor + remote Yrs provider.
2. When remote is enabled and cloud is signed in, `openCollabSession` polls:
   pull sidecar → apply to Y.Doc (and daemon journal) → push merged full state.
   <!-- TODO(I1/YW): optional LYRL append-log poll/append alongside LYRS snapshot -->
3. Handlers: `push_collab_remote_snapshot` / `pull_collab_remote_snapshot` in
   `lattice-handlers` via Tauri commands.
4. Core types + in-memory stores: `lattice_collab::remote`
   (`YrsRemoteStore`, `YrsRemoteLogStore`, `encode_remote_log`,
   `decode_remote_log`, `append_update`, `collab_log_resource_id`).

No new server routes required. Ecosystem sync-heads may list the sidecars as
extra blobs; they are not materialized as workspace files.

## Out of scope (fence)

Capture, presence, app_lock. Awareness stays local Tauri fan-out only.
