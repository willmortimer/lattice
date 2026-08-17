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
each poll (desktop `openCollabSession` default when a log sidecar exists):

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
2. When remote is enabled and cloud is signed in, `openCollabSession` polls the
   **LYRL append log** on each remote sync interval: pull sidecar → apply each
   lib0 update to the Y.Doc (and daemon journal). If no log exists yet, fall back
   once to the **LYRS snapshot** sidecar (first peer or pre-LYRL deployments).
   Local edits debounce-append to LYRL via `pushCollabRemoteLog`; they do not
   push a full snapshot on every poll. When append returns `log_needs_compact`,
   compact writes a fresh LYRS snapshot then `replace_collab_remote_log` resets
   the log (empty updates, `base_hash` = snapshot content hash) before retrying
   the pending append.
3. Handlers (cloud blob PUT/GET, no new server routes):
   - Snapshot: `push_collab_remote_snapshot` / `pull_collab_remote_snapshot`
     (Tauri: `push_collab_remote_snapshot_cmd` / `pull_collab_remote_snapshot_cmd`).
   - Append log: `push_collab_remote_log` / `pull_collab_remote_log`
     (Tauri: `push_collab_remote_log_cmd` / `pull_collab_remote_log_cmd`).
     Push pulls the existing LYRL blob (404 → empty), `append_update`, then PUT
     with `If-Match` so two peers do not clobber. Exceeding log limits returns
     an error string containing `log_needs_compact`.
   - Replace log: `replace_collab_remote_log`
     (Tauri: `replace_collab_remote_log_cmd`). PUTs `encode_remote_log` without
     append — used after compaction to reset the sidecar.
4. Core types + in-memory stores: `lattice_collab::remote`
   (`YrsRemoteStore`, `YrsRemoteLogStore`, `encode_remote_log`,
   `decode_remote_log`, `append_update`, `collab_log_resource_id`).

### `base_hash` (Tauri)

`base_hash` is 32 raw SHA-256 bytes of the LYRS snapshot this log is based on,
or 32 zero bytes (`REMOTE_LOG_UNKNOWN_BASE_HASH`) when unknown.

`push_collab_remote_log_cmd` accepts an optional **hex string** (64 characters,
no `sha256:` prefix). Omit or pass empty for the unknown-base zeros. If you
already have the 32 raw bytes, hex-encode them before invoke
(`hex::encode` / `Buffer.from(bytes).toString('hex')`).

`pull_collab_remote_log_cmd` returns `baseHash` as a 32-byte array (serde
`Vec<u8>`), not hex. Hex-encode that array if you need to pass it back into
the push command. `contentHash` remains the hex digest of the LYRL blob.

No new server routes required. Ecosystem sync-heads may list the sidecars as
extra blobs; they are not materialized as workspace files.

## Out of scope (fence)

Capture, presence, app_lock. Awareness stays local Tauri fan-out only.
