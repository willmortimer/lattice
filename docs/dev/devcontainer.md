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
| Run `lattice-bridge` (HTTP handler backend) | `./scripts/devcontainer/bridge` → port **8787** |
| Seed + bridge + web instructions | `./scripts/devcontainer/up` |
| Run headless checks in the cell | `./scripts/devcontainer/test` |
| Native window + real filesystem | **Not here** — `nix run .#desktop-dev` on macOS |

### Demo fixture vs bridge mode

| Mode | How | Data |
| --- | --- | --- |
| **Demo fixture** | `./scripts/devcontainer/web` with `VITE_LATTICE_BRIDGE_URL` unset | In-memory sample workspace; no Rust core |
| **Bridge mode** | `./scripts/devcontainer/bridge` + `./scripts/devcontainer/web` (default) | Real Rust handlers via `lattice-bridge` |

`./scripts/devcontainer/web` exports `VITE_LATTICE_BRIDGE_URL` (default
`http://127.0.0.1:8787`) and, when seeded, `VITE_LATTICE_WORKSPACE` pointing
at the First Look workspace path **as seen by the bridge process** (same
container → local path). Unset `VITE_LATTICE_BRIDGE_URL` to fall back to the
demo fixture.

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
./scripts/devcontainer/bridge
./scripts/devcontainer/up
./scripts/devcontainer/test
```

Bind address is `0.0.0.0` for Vite, Astro, and bridge inside the container so
Docker / DevCell published ports and `tailscale serve` can reach the processes.

## Real core in the browser (recommended cell flow)

```sh
# Terminal 1 — seed workspace + start bridge
./scripts/devcontainer/seed
./scripts/devcontainer/bridge

# Terminal 2 — Vite with bridge env vars
./scripts/devcontainer/web
```

Or use the helper (seeds, starts bridge in background, prints web command):

```sh
./scripts/devcontainer/up
# then in another terminal:
./scripts/devcontainer/web
```

Open the forwarded UI on port **5173**. The shell talks to `lattice-bridge` on
**8787**, which runs the same handlers as the Tauri desktop shell.

Smoke the bridge:

```sh
curl -s http://127.0.0.1:8787/health
```

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

## HTTP bridge (`lattice-bridge`)

The cell has no Tauri IPC. [`lattice-bridge`](../../apps/bridge/README.md) exposes
the handler surface over HTTP so the browser shell can call the real Rust core.

`./scripts/devcontainer/bridge`:

- Binds **`0.0.0.0:8787`** inside the Dev Container (`DEVCONTAINER=1` /
  `DEV_CELL=true`) so published ports work; **`127.0.0.1:8787`** elsewhere.
- Defaults `--root` to the seeded First Look workspace under `LATTICE_DEV_HOME`.
- Override with `LATTICE_BRIDGE_HOST`, `LATTICE_BRIDGE_PORT`, or `LATTICE_DEV_HOME`.

Manual equivalent:

```sh
cargo run -p lattice-bridge -- \
  --host 0.0.0.0 --port 8787 \
  --root "${LATTICE_DEV_HOME}/Workspaces/First Look"
```

CORS allows `http://localhost:5173` and `http://127.0.0.1:5173` only. Port
forwarding and local dev use those origins. If you expose Vite via Tailscale
Serve on a non-localhost hostname, CORS will block bridge requests — use port
forward to `localhost:5173` or run bridge + web on the same host origin.

See [environment.md](./environment.md) for `LATTICE_DEV_HOME` and `LATTICE_HOME`.

## Ports

| Port | Process | Start command | Notes |
| --- | --- | --- | --- |
| **5173** | Vite (`@lattice/desktop`) | `./scripts/devcontainer/web` | Browser shell (demo or bridge mode) |
| **4321** | Astro (`@lattice/site`) | `./scripts/devcontainer/site` | Marketing + docs |
| **8787** | `lattice-bridge` | `./scripts/devcontainer/bridge` | HTTP handler backend for bridge mode |

Expose them from the cell (DevCell published ports or Tailscale Serve). Do not
assume Mac `localhost` is the cell. The browser reaches bridge and Vite via
forwarded ports on the host (`127.0.0.1`), even though processes bind `0.0.0.0`
inside the container.

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
- Docker Compose multi-process orchestration (use the scripts above instead)

## Architecture notes

- Prefer **amd64/x86_64** for the DevCell appliance lab; arm64 images may work
  for local Docker on Apple Silicon but are not the dogfood target.
- Cold image build needs network for `rustup` and apt; first `cargo test` and
  `pnpm install` also pull caches (volume mounts for cargo registry/git and the
  pnpm store are declared in `devcontainer.json`).

See also: [nix-workflows.md](./nix-workflows.md) for the three desktop/web
surfaces and when to use each.
