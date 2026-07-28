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
cargo test -p lattice-cloud-client -p lattice-handlers
cargo check -p lattice-desktop
```
