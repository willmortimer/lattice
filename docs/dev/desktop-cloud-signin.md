# Desktop cloud sign-in (bearer)

Desktop opt-in sign-in against `lattice-server`. The web app uses a Next BFF cookie; the desktop uses `Authorization: Bearer` and stores
the opaque token in the OS keychain (`lattice.cloud` / `lattice.cloud.user`).

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `LATTICE_CLOUD_URL` | `https://cloud.lattice-notes.com` | lattice-server origin (no trailing slash) |
| `LATTICE_CLOUD_TOKEN` | _(unset)_ | CLI escape hatch bearer when keychain ACL blocks unsigned tools |

Contract: desktop uses API bearer auth (ADR 0067); browser uses Next cookie BFF.

## Settings UI

Settings → **Cloud account**: Sign in with Apple (native), email/password, sign-out, and session status.
Rust owns HTTP (`lattice-cloud-client`); React invokes Tauri commands only.

Settings → **Features → Labs**: cloud blob put/get for a workspace-relative path
(`cloud_blob_materialize` / `cloud_blob_open`). Unsigned sessions fail closed with a clear
error (sign in via Cloud account, or set `LATTICE_CLOUD_TOKEN`).

**Inspect → Properties** exposes the same round-trip for the selected resource:
**Back up to Lattice Cloud** / **Reopen from cloud**, then shows Authority after a
successful backup. Resource ID is the registry UUID when known; otherwise an honest
`path:` placeholder (never a fabricated UUID).

## Manual smoke (live VPS)

After password auth is enabled on the VPS:

1. Export `LATTICE_CLOUD_URL` if not using production (optional).
2. Launch the desktop app (`pnpm tauri:dev` or installed build).
3. Open Settings → Cloud account.
4. Sign in with a lab account (`POST /v1/auth/password/register` on the server if needed).
5. Confirm status shows your email; quit and relaunch — session should restore from keychain.
6. Sign out; confirm status returns to “Not signed in”.

## Tests

```sh
cargo test -p lattice-cloud-client -p lattice-handlers -p latticefs-core
cargo test -p lattice-desktop --lib cloud::tests
cargo check -p lattice-desktop
```

## Blob round-trip smoke (live VPS)

Requires a signed-in cloud session (desktop Settings → Cloud account, or any process
that stores the bearer in keychain `lattice.cloud` / `lattice.cloud.user`).

```sh
export LATTICE_CLOUD_URL=https://cloud.lattice-notes.com   # optional; this is the default
cd /path/to/workspace
lattice cloud blob-roundtrip notes/example.md
lattice resource stat notes/example.md
# authority should be `cloud`, content_hash set
```

Use a **new** `ResourceId` on each live PUT (the server is write-once per id). The CLI
registers the workspace path first, so re-running against the same file after a successful
upload will return **409** from the server unless you use a fresh registry entry.

After a successful round-trip, open the resource in the desktop app and check **Inspect →
Properties → Authority**. It should show **Cloud** (from the registry
`authority: cloud` written by `mark_cloud_backed`). Materialization is typically
**Metadata only** until a later pin/cache path lands.

## Cloud fetch failures (offline / auth)

Cloud-backed resources (`authority: cloud`) must not silently use stale local file bytes
when the cloud GET fails.

- `lattice cloud blob-open <path>` fetches canonical bytes from cloud and writes them to stdout.
  On network error, **401**, or **5xx**, the command exits non-zero with an explicit error
  (for example `cloud blob error: cloud API error (401): invalid session`).
- Failed upload round-trips (`lattice cloud blob-roundtrip`) do **not** mark the resource
  as cloud-authoritative; authority stays `local`.
- Inspect still shows registry metadata (`authority`, `materialization`); it does not
  substitute local disk content when cloud is unreachable.

```sh
# After a successful blob-roundtrip, offline fetch fails visibly:
lattice cloud blob-open notes/example.md
# cloud blob error: cloud request failed: network unreachable
```
