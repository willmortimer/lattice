# Desktop cloud sign-in (bearer)

Desktop opt-in sign-in against `lattice-server`. The web app uses a Next BFF cookie; the desktop uses `Authorization: Bearer` and stores
the opaque token in the OS keychain (`lattice.cloud` / `lattice.cloud.user`).

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `LATTICE_CLOUD_URL` | `https://cloud.lattice-notes.com` | lattice-server origin (no trailing slash) |

Contract: desktop uses API bearer auth (ADR 0067); browser uses Next cookie BFF.

## Settings UI

Settings → **Cloud account**: email/password sign-in, sign-out, and session status.
Rust owns HTTP (`lattice-cloud-client`); React invokes Tauri commands only.

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
