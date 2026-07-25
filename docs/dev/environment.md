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
| `LATTICE_GITHUB_APP_CLIENT_ID` | `.env` / shell | GitHub App client id | Enables GitHub App device-flow connect ([ADR 0045](../decisions/0045-github-connected-repos-are-readonly-extracts.md)). App needs Contents + Metadata read-only. | No | Works today (desktop connector) |
| `RUST_BACKTRACE` | your shell | n/a (`1` or `full`) | Backtraces on Rust panics in CLI/desktop dev | No | Works today (std behavior) |
| `RUST_LOG` | your shell | n/a (e.g. `debug`) | Log-level filter | No | **Not yet wired** — takes effect once tracing/env-logger lands (observability workstream) |

## Local macOS signing (`desktop-install`)

Used by `nxr desktop-install` / `nix run .#desktop-install`. Prefer
[`secrets/apple.env`](../../secrets/apple.env) via sops (direnv decrypts). Apple
Development identities work for **your** Mac; a paid Apple Developer Program
membership is required for Developer ID Application signing and notarization
(distribution / other machines).

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `APPLE_SIGNING_IDENTITY` | **sops** `secrets/apple.env` or `.env` | `security find-identity -v -p codesigning` | Codesign identity for the `.app` bundle | Privileged | **Required** for `desktop-install` / `desktop-release` |
| `APPLE_TEAM_ID` | same sops file (plaintext field) | Membership details / certificate OU | Team ID for notarization and some signing flows | Privileged | Recommended for install; **required** for `desktop-release` |
| `LATTICE_INSTALL_DIR` | `.env` / shell | absolute directory (default `/Applications`) | Parent directory for `desktop-install` | No | Works today |

## Notarization & DMG (`desktop-release`)

Used by `nxr desktop-release` / `nix run .#desktop-release`. Builds the same
voice-enabled `.app` as `desktop-install`, signs with **Developer ID
Application**, submits via `xcrun notarytool`, staples, then packs a DMG with
`hdiutil`. Prefer sops — never put `APPLE_PASSWORD` in plaintext `.env`.

```sh
# validate required env without building:
LATTICE_RELEASE_VALIDATE_ONLY=1 nix run .#desktop-release

# full packet (decrypts apple.env for this process only):
sops exec-env secrets/apple.env -- nix run .#desktop-release
```

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `APPLE_SIGNING_IDENTITY` | **sops** `secrets/apple.env` | `Developer ID Application: … (TEAMID)` from Keychain | Codesign for distribution | Privileged | **Required** — rejects Apple Development identities |
| `APPLE_ID` | **sops** `secrets/apple.env` | Apple ID email | `notarytool` account | Privileged (encrypted) | **Required** |
| `APPLE_PASSWORD` | **sops** `secrets/apple.env` | App-specific password from [appleid.apple.com](https://appleid.apple.com) | `notarytool` auth | **Yes** | **Required** |
| `APPLE_TEAM_ID` | same sops file | Membership details / certificate OU | `notarytool --team-id` | Privileged | **Required** |
| `LATTICE_RELEASE_DIR` | shell | absolute or repo-relative dir | DMG output directory (default `target/release/bundle/dmg`) | No | Optional |
| `LATTICE_RELEASE_VALIDATE_ONLY` | shell | `1` / `true` | Exit after env checks (no Tauri build) | No | Optional |
| `TAURI_SIGNING_PRIVATE_KEY` | CI secret / local keychain | `pnpm tauri signer generate` | Signs updater artifacts | **Yes** | Not used yet |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | CI secret | chosen when generating the key | Unlocks the signing key | **Yes** | Not used yet |

Notarytool talks to Apple over the network and may prompt Keychain for the
signing private key — expect a human-attended machine for the first staple.

## Site publish (Cloudflare Pages)

Live site: <https://lattice-dop.pages.dev/>. See [secrets/README.md](../../secrets/README.md)
and [nix-workflows.md](./nix-workflows.md).

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `CLOUDFLARE_API_TOKEN` | **sops** `secrets/cloudflare.env` (direnv decrypts) or CI secret | [Cloudflare API tokens](https://developers.cloudflare.com/fundamentals/api/get-started/create-token/) — Account → Cloudflare Pages → Edit | Non-interactive auth for `nix run .#site-deploy` / wrangler | **Yes** | Preferred |
| `CLOUDFLARE_ACCOUNT_ID` | same sops file (plaintext field) or CI | Cloudflare dashboard → account overview | Disambiguates account | No | Set in `secrets/cloudflare.env` |

Do **not** put the API token or Apple passwords in `.env`. Encrypted
`secrets/*.env` files are **tracked** in this public repo (ciphertext only);
edit them with `sops secrets/<name>.env`.

```sh
# after rotating the token into sops + direnv reload:
nix run .#site-deploy

# one-shot:
sops exec-env secrets/cloudflare.env -- nix run .#site-deploy
```

Tag-only CI (no deploy on every `main` push): push a `v*` tag, or run the
**Site deploy** workflow manually. Set GitHub Actions secrets
`CLOUDFLARE_API_TOKEN` and optional `CLOUDFLARE_ACCOUNT_ID`.

## Rules

- Never commit real plaintext secrets; `.env` is gitignored (`.env.example` is
  the only tracked dotenv template).
- API tokens and passwords belong in `secrets/*.env` (sops + age) or CI secret
  stores — not in `.env`, chats, or screenshots.
- Age private keys stay in `~/.config/sops/age/keys.txt` on the machine.
- Add a row here in the same PR that introduces a new variable.

## Cell Apple VZ lab (macOS — optional)

Supervises `cell-host-macos` and `celld --backend=vz` inside `latticed` when
`LATTICE_CELL_VZ=1`. Intended for local Lattice + Cell integration demos on
Apple Silicon; not required for normal desktop use. Set vars in `.env` (loaded by
`pnpm tauri:dev` / direnv) so the desktop forwards them when spawning
`latticed`.

| Variable | Where to set | Where to get it | What it does | Secret? | Status |
| --- | --- | --- | --- | --- | --- |
| `LATTICE_CELL_VZ` | `.env` / shell | `1`, `true`, or `yes` | Enable Cell VZ child supervision in `latticed` | No | Works today |
| `LATTICE_CELL_HOST_BIN` | `.env` / shell | path to signed `cell-host-macos` | Override helper binary (else `PATH` / sibling of `latticed`) | No | Works today |
| `LATTICE_CELLD_BIN` | `.env` / shell | path to `celld` | Override daemon binary (else `PATH`) | No | Works today |
| `CELL_VZ_IMAGES_DIR` | `.env` / shell | staged aarch64 artifacts dir | Rootfs/kernel/initrd for `--vz-*` flags (default `~/.cell/images/cellos-aarch64`) | No | Works today |
| `CELL_VZ_HELPER_SOCKET` | `.env` / shell | UDS path | `cell-host-macos --socket` (default `{Lattice run}/cell-host-macos.sock`) | No | Works today |
| `LATTICE_CELL_DATA_DIR` | `.env` / shell | writable directory | `celld --data-dir` / state DB (default `~/Library/Application Support/Lattice/cell-lab`) | No | Works today |
| `LATTICE_CELL_LISTEN` | `.env` / shell | `host:port` | `celld --listen` (default `127.0.0.1:18788`) | No | Works today |

Example (local Cell worktree binaries):

```sh
export LATTICE_CELL_VZ=1
export LATTICE_CELL_HOST_BIN=/path/to/cell/apps/cell-host-macos/.build/debug/cell-host-macos
export LATTICE_CELLD_BIN=/path/to/cell/.build/celld
export CELL_VZ_IMAGES_DIR=$HOME/.cell/images/cellos-aarch64
pnpm --filter @lattice/desktop tauri:dev
```

Build/sign the helper first (`./scripts/vz-codesign-helper.sh` in the Cell repo).
Missing binaries fail `latticed` start with an actionable message naming
`LATTICE_CELL_HOST_BIN` / `LATTICE_CELLD_BIN`.
