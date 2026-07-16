# Lattice

A fast, local-first, open-native workspace for documents, relational data
applications, analytical datasets, notebooks, canvases, drawings, dashboards,
and automations — built from inspectable files and one coherent command model.

> The workspace is a real directory. Offline is the normal state. Every
> important GUI action is also a semantic command. AI is an interchangeable
> client, not a bundled brain.

Full product and architecture specification: [docs/00-overview.md](docs/00-overview.md)

## Repository layout

```text
lattice/
├── apps/
│   ├── cli/          # `lattice` headless CLI (init, validate, ls)
│   └── desktop/      # Tauri 2 desktop shell (React 19 + Vite + TypeScript)
├── crates/
│   └── lattice-core/ # workspace discovery, manifests, resource model, validation
├── docs/             # architecture specification and ADRs
├── site/             # Astro marketing + documentation site
└── flake.nix         # Nix dev shell
```

## Status

Early scaffold implementing the start of [roadmap](docs/29-roadmap.md)
Phase 0 (headless core + CLI) and Phase 1 (minimal desktop shell). The
specification in `docs/` is intentionally far broader than the current code.

## Development

With Nix (recommended):

```sh
nix develop        # rust toolchain, node, pnpm, tauri prerequisites
```

Requires flakes; if not enabled globally, add
`experimental-features = nix-command flakes` to `~/.config/nix/nix.conf`
or pass `--extra-experimental-features 'nix-command flakes'`. Direnv users
can `direnv allow` to load the shell automatically via `.envrc`.

Common tasks are exposed as flake apps (self-contained — they bring their
own toolchain, so they work outside the dev shell too):

Inside the dev shell the same tasks are plain commands (`lattice-test`,
`lattice-check`, …). Full workflow guide: [docs/dev/nix-workflows.md](docs/dev/nix-workflows.md).
Environment variables (currently none required): [docs/dev/environment.md](docs/dev/environment.md).

```sh
nix run .#test           # cargo test --workspace
nix run .#lint           # clippy -D warnings + rustfmt check
nix run .#fmt            # rustfmt
nix run .#check          # everything CI would run (rust + both frontends)
nix run .#site-dev       # Astro marketing/docs site (not the app)
nix run .#site-build     # static site build
nix run .#docs-sync      # regenerate site docs content from docs/
nix run .#desktop-dev    # Tauri native window + Vite HMR on :5173
nix run .#desktop-build  # tauri release build, unbundled
```

`desktop-dev` starts **both** the native app and Vite on :5173 (Tauri loads the UI from Vite in dev). Opening :5173 in a browser is a demo-only shell — see [docs/dev/nix-workflows.md](docs/dev/nix-workflows.md).

First-run home: `lattice home ensure` creates `~/Lattice/{Workspaces,Settings}` and seeds `Workspaces/Personal`. New workspaces: `lattice init --template personal|team|demo|blank`.

```sh
nix run .#desktop-dev    # native + Vite HMR
nix run .#desktop-web    # browser-only demo UI
nix run .#desktop        # native without Vite (reuses dist)
```

Or bring your own toolchain: stable Rust, Node.js ≥ 22, pnpm ≥ 9, and Xcode
Command Line Tools on macOS (required for Tauri bundling either way).

```sh
# core + CLI
cargo build
cargo test
cargo run -p lattice-cli -- init my-workspace

# desktop shell
pnpm install
pnpm --filter @lattice/desktop tauri dev

# marketing/docs site
pnpm --filter @lattice/site dev
```

## License

Not yet licensed for redistribution; a licensing split (permissive specs,
copyleft client) is planned — see
[docs/35-licensing-governance-and-sustainability.md](docs/35-licensing-governance-and-sustainability.md).
