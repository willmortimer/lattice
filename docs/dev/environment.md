# Environment variables

Single source of truth for every environment variable this project uses or
will use. Keep this table and [.env.example](../../.env.example) in sync when
adding one.

**Current state: no environment variables are required.** The app is
local-first by design — no API keys, no backend endpoints, no telemetry.
Most runtime overrides are optional developer conveniences documented below.
An audit of the source (Rust `env::var`, JS `process.env` /
`import.meta.env`) confirms nothing beyond these is read today except Vite's
built-in `DEV` flag.

`.envrc` at the repo root is **direnv configuration** (it loads the nix dev
shell), not an env-var file.

## Optional — developer convenience

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `LATTICE_DEV_HOME` | `nix run .#desktop-dev`, `pnpm tauri:dev`, or your shell | absolute `…/target/dev-home` (set automatically by `tauri:dev`) | Isolated Lattice profile root for local Tauri dev. Takes precedence over `LATTICE_HOME` and `~/Lattice`. Relative values are resolved against the process current directory. First-run seeds the **First Look** (`demo`) template instead of Personal. Delete the directory to reset. | No | Works today |
| `LATTICE_HOME` | your shell | any writable directory | Override the Lattice profile root (`~/Lattice` by default). Relative values are resolved against the process current directory. Ignored when `LATTICE_DEV_HOME` is set. | No | Works today |
| `RUST_BACKTRACE` | your shell | n/a (`1` or `full`) | Backtraces on Rust panics in CLI/desktop dev | No | Works today (std behavior) |
| `RUST_LOG` | your shell | n/a (e.g. `debug`) | Log-level filter | No | **Not yet wired** — takes effect once tracing/env-logger lands (observability workstream) |

## Future — release signing & distribution (none used yet)

These become relevant when we ship signed builds. None are read by any code
in the repo today; they are documented so they land in one place.

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | CI secret / local keychain | `pnpm tauri signer generate` | Signs updater artifacts | **Yes** | Not used yet |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | CI secret | chosen when generating the key | Unlocks the signing key | **Yes** | Not used yet |
| `APPLE_ID` | CI secret | your Apple developer account email | macOS notarization | Privileged | Not used yet |
| `APPLE_PASSWORD` | CI secret | app-specific password from appleid.apple.com | macOS notarization | **Yes** | Not used yet |
| `APPLE_TEAM_ID` | CI secret | developer.apple.com membership page | macOS notarization | Privileged | Not used yet |
| `APPLE_SIGNING_IDENTITY` | CI secret / keychain | "Developer ID Application: …" cert | Codesigning identity | Privileged | Not used yet |

Site deployment tokens (host-dependent — e.g. Cloudflare/Netlify) will be
added here when a deploy target is chosen.

## Rules

- Never commit real values; `.env` and `.env.*` are gitignored
  (`.env.example` is the only tracked one).
- Secrets belong in CI secret stores or the macOS keychain, not in files.
- Add a row here in the same PR that introduces a new variable.
