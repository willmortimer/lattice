# Dev Container (cell demo)

Linux entrypoint for running Lattice inside a Dev Container or DevCell cell.
Nix + Tauri on your Mac remain the source of truth for the native desktop shell.

## What this is for

| Goal | Use |
| --- | --- |
| Open the browser demo UI remotely | `./scripts/devcontainer/web` → port **5173** |
| Open marketing / Starlight docs | `./scripts/devcontainer/site` → port **4321** |
| Build the headless `lattice` CLI | `./scripts/devcontainer/cli` |
| Seed a real demo workspace on disk | `./scripts/devcontainer/seed` |
| Run headless checks in the cell | `./scripts/devcontainer/test` |
| Native window + real filesystem | **Not here** — `nix run .#desktop-dev` on macOS |

The browser UI on :5173 is the **demo fixture** (no native filesystem authority).
That is intentional for the cell / Tailscale Serve path.

## Start

With VS Code / Cursor Dev Containers, or any host that understands
`.devcontainer/devcontainer.json`:

1. Open this repository in the container (build uses `.devcontainer/Dockerfile`).
2. `postCreate` runs `pnpm install`.
3. Start the surfaces you need (they are **not** auto-started):

```sh
./scripts/devcontainer/web
./scripts/devcontainer/site
./scripts/devcontainer/cli
./scripts/devcontainer/seed
./scripts/devcontainer/test
```

Bind address is `0.0.0.0` so Docker / DevCell published ports and
`tailscale serve` can reach the processes.

## CLI (headless workspace)

The cell image includes Rust but not Nix or Tauri. Use the `lattice` CLI for
real filesystem workspaces, scripts, and headless automation inside the cell.

```sh
# Build debug binary (default) → target/debug/lattice
./scripts/devcontainer/cli

# Release build
./scripts/devcontainer/cli --release
# or: LATTICE_CLI_RELEASE=1 ./scripts/devcontainer/cli

# Ensure demo home + First Look workspace (sets LATTICE_DEV_HOME when unset)
./scripts/devcontainer/seed
```

`seed` defaults `LATTICE_DEV_HOME` to `<repo>/target/cell-home` (in the
container: `/workspaces/lattice/target/cell-home`). That mirrors local Tauri
dev isolation: `lattice home ensure` seeds the **First Look** (`demo`) template
instead of Personal. Override with `LATTICE_DEV_HOME` before running `seed` or
the CLI.

After seeding, smoke the workspace:

```sh
export LATTICE_DEV_HOME=/workspaces/lattice/target/cell-home
LATTICE=target/debug/lattice   # or target/release/lattice

$LATTICE --help
$LATTICE home path
$LATTICE ls "${LATTICE_DEV_HOME}/Workspaces/First Look"
$LATTICE index "${LATTICE_DEV_HOME}/Workspaces/First Look"
$LATTICE search workspace "${LATTICE_DEV_HOME}/Workspaces/First Look" --limit 5
```

`./scripts/devcontainer/test` runs a short CLI smoke (build + seed + `ls` /
`search`) after the existing cargo and pnpm checks.

## HTTP bridge (browser demo backend)

The cell has no Tauri IPC. Run [`lattice-bridge`](../../apps/bridge/README.md)
beside Vite so the browser demo can call the same handlers as the desktop shell:

```sh
cargo run -p lattice-bridge -- --host 127.0.0.1 --port 8787
# optional default workspace after seed:
# cargo run -p lattice-bridge -- --root "${LATTICE_DEV_HOME}/Workspaces/First Look"

curl -s http://127.0.0.1:8787/health
```

CORS allows `http://localhost:5173` and `http://127.0.0.1:5173` only. React
transport wiring is separate; this task only ships the Rust daemon.

See [environment.md](./environment.md) for `LATTICE_DEV_HOME` and `LATTICE_HOME`.

## Ports

| Port | Process | Start command | Notes |
| --- | --- | --- | --- |
| **5173** | Vite (`@lattice/desktop`) | `./scripts/devcontainer/web` | Browser demo shell |
| **4321** | Astro (`@lattice/site`) | `./scripts/devcontainer/site` | Marketing + docs |

Expose them from the cell (DevCell published ports or Tailscale Serve). Do not
assume Mac `localhost` is the cell.

## Toolchain (no Nix)

Image base: `mcr.microsoft.com/devcontainers/javascript-node:1-22-bookworm`,
plus Rust **1.85** (matches `rust-version` in `Cargo.toml`), `pkg-config`, and
`libssl-dev`. pnpm is activated via Corepack to **11.11.0** (root
`packageManager`).

Dockerfile lineage: adapted from [willmortimer/Templates](https://github.com/willmortimer/Templates)
`pnpm-monorepo` + `rust-cli` at `e7fac162` — without the compose / `bin/dc`
launcher stack, so DevCell’s workspace planner can treat this like a normal
repo `.devcontainer`.

## Out of scope

- `tauri` / `nix run .#desktop-dev` (needs a display / WebView stack)
- Full Nix flake inside the container
- Secrets or Cloudflare Pages tokens in the image

## Architecture notes

- Prefer **amd64/x86_64** for the DevCell appliance lab; arm64 images may work
  for local Docker on Apple Silicon but are not the dogfood target.
- Cold image build needs network for `rustup` and apt; first `cargo test` and
  `pnpm install` also pull caches (volume mounts for cargo registry/git and the
  pnpm store are declared in `devcontainer.json`).

See also: [nix-workflows.md](./nix-workflows.md) for the three desktop/web
surfaces and when to use each.
