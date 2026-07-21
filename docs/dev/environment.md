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

`.envrc` at the repo root is **direnv configuration**: it loads the nix
dev shell (`use flake`) and, when present, a gitignored `.env` via
`dotenv_if_exists .env`.

## Optional — developer convenience

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `LATTICE_DEV_HOME` | `nix run .#desktop-dev`, `pnpm tauri:dev`, Dev Container (`devcontainer.json`), `./scripts/devcontainer/seed`, or your shell | absolute `…/target/dev-home` (Tauri) or `…/target/cell-home` (cell; default for `seed`) | Isolated Lattice profile root for local Tauri dev and Linux cell scripts. Takes precedence over `LATTICE_HOME` and `~/Lattice`. Relative values are resolved against the process current directory. First-run seeds the **First Look** (`demo`) template instead of Personal. Delete the directory to reset. | No | Works today |
| `LATTICE_DEV_RESET_DEMO` | `pnpm tauri:dev` / `desktop-dev` (default), or your shell | `1`, `true`, or `yes` | When set with `LATTICE_DEV_HOME` (or other demo seeding), wipe and re-provision **First Look** from the current `demo` template on every launch. Opt out with `pnpm --filter @lattice/desktop tauri:dev:keep`. | No | Works today |
| `LATTICE_HOME` | your shell | any writable directory | Override the Lattice profile root (`~/Lattice` in release; in **debug** builds without `LATTICE_DEV_HOME`, the default is `target/dev-home` under the process cwd). Relative values are resolved against the process current directory. Ignored when `LATTICE_DEV_HOME` is set. Selects Personal seeding instead of First Look. | No | Works today |
| `LATTICE_FORCE_PROD_HOME` | your shell | `1`, `true`, or `yes` | In **debug** builds, use the real `~/Lattice` profile and Personal seeding instead of the automatic `target/dev-home` dev profile. Ignored in release builds. | No | Works today |
| `RUST_BACKTRACE` | your shell | n/a (`1` or `full`) | Backtraces on Rust panics in CLI/desktop dev | No | Works today (std behavior) |
| `RUST_LOG` | your shell | n/a (e.g. `debug`) | Log-level filter | No | **Not yet wired** — takes effect once tracing/env-logger lands (observability workstream) |
| `LATTICE_INSTALL_DIR` | `.env` / shell | absolute directory (default `/Applications`) | Parent directory for `desktop-install` (`Lattice.app` is placed inside) | No | Works with `nxr desktop-install` |

## Local macOS signing (`desktop-install`)

Used by `nxr desktop-install` / `nix run .#desktop-install`. Apple Development
is enough for **your** Mac; Developer ID + notarization still require a paid
account for other machines.

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `APPLE_SIGNING_IDENTITY` | `.env` (direnv) / shell / keychain | `security find-identity -v -p codesigning` | Codesign identity for the `.app` bundle | Privileged | **Required** for `desktop-install` |
| `APPLE_TEAM_ID` | `.env` / shell | Certificate OU / developer.apple.com membership | Team ID; optional for local Apple Development, required later for notarization | Privileged | Recommended |

## Future — updater signing & notarization (not used by desktop-install)

These become relevant when shipping auto-updates or distributing beyond your
Mac. None are read by `desktop-install` today.

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | CI secret / local keychain | `pnpm tauri signer generate` | Signs updater artifacts | **Yes** | Not used yet |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | CI secret | chosen when generating the key | Unlocks the signing key | **Yes** | Not used yet |
| `APPLE_ID` | CI secret | your Apple developer account email | macOS notarization | Privileged | Not used yet |
| `APPLE_PASSWORD` | CI secret | app-specific password from appleid.apple.com | macOS notarization | **Yes** | Not used yet |

## Site publish (Cloudflare Pages)

Live site: <https://lattice-dop.pages.dev/>. Prefer interactive login from the
ops shell (`nix develop .#ops` → `wrangler login`); tokens land in your home
directory, not the Nix store. See [nix-workflows.md](./nix-workflows.md).

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `CLOUDFLARE_API_TOKEN` | shell / CI secret (optional) | [Cloudflare API tokens](https://developers.cloudflare.com/fundamentals/api/get-started/create-token/) with Pages edit | Non-interactive auth for `wrangler` / `nix run .#site-deploy` when OAuth login is unavailable | **Yes** | Works with wrangler |
| `CLOUDFLARE_ACCOUNT_ID` | shell / CI (optional) | Cloudflare dashboard → account overview | Disambiguates account when the token can see more than one | No | Optional |

## Rules

- Never commit real values; `.env` and `.env.*` are gitignored
  (`.env.example` is the only tracked one).
- Secrets belong in CI secret stores or the macOS keychain, not in files.
- Add a row here in the same PR that introduces a new variable.
