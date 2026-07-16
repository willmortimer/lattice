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
│   ├── cli/              # `lattice` headless CLI
│   └── desktop/          # Tauri 2 shell (React 19 + Vite + TypeScript)
├── crates/
│   ├── lattice-core/     # workspace model, manifest, watcher, home layout
│   ├── lattice-storage/  # filesystem store + recovery journal
│   ├── lattice-commands/ # semantic command / transaction engine
│   ├── lattice-index/    # FTS5 search + backlinks
│   ├── lattice-data/     # `.data` table packages (SQLite + views)
│   └── lattice-theme/    # theme YAML, appearance settings, CSS flattening
├── themes/               # built-in themes (Lattice Slate, Lattice Paper)
├── scripts/              # compile-theme and related generators
├── docs/                 # architecture specification and ADRs
├── design/               # brand mark / visual identity notes
├── site/                 # Astro marketing + Starlight docs
└── flake.nix             # Nix dev shell and task runners
```

## Status

Active Phase 0–1 scaffold: headless core + CLI, desktop shell (pages, search,
data tables, theming), and the marketing/docs site. The specification in
`docs/` is intentionally broader than the current code — see
[roadmap](docs/29-roadmap.md).

## Development

With Nix (recommended):

```sh
nix develop        # rust toolchain, node, pnpm, tauri prerequisites
```

Requires flakes; if not enabled globally, add
`experimental-features = nix-command flakes` to `~/.config/nix/nix.conf`
or pass `--extra-experimental-features 'nix-command flakes'`. Direnv users
can `direnv allow` to load the shell automatically via `.envrc`.

Common tasks are flake apps (and `lattice-<task>` inside the dev shell).
Full guide: [docs/dev/nix-workflows.md](docs/dev/nix-workflows.md).
Environment variables: [docs/dev/environment.md](docs/dev/environment.md).

```sh
nix run .#test            # cargo test --workspace
nix run .#lint            # clippy -D warnings + rustfmt check
nix run .#fmt             # rustfmt
nix run .#check           # everything CI would run (rust + both frontends)
nix run .#compile-theme   # themes/*.theme.yaml → CSS/TS tokens
nix run .#site-dev        # Astro marketing/docs site
nix run .#site-build      # static site build
nix run .#docs-sync       # regenerate site docs content from docs/
nix run .#desktop-dev     # Tauri native window + Vite HMR on :5173
nix run .#desktop-web     # browser-only demo UI
nix run .#desktop         # native without Vite (reuses dist)
nix run .#desktop-build   # tauri release build, unbundled
```

`desktop-dev` starts **both** the native app and Vite on :5173. Opening :5173
in a browser is a demo-only shell — see the nix workflows doc.

First-run home: `lattice home ensure` creates `~/Lattice/{Workspaces,Settings}`
and seeds `Workspaces/Personal`. New workspaces:
`lattice init --template personal|team|demo|blank`.

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

# theme tokens (also runs on desktop/site predev)
pnpm compile-theme
```

## License

Not yet licensed for redistribution; a licensing split (permissive specs,
copyleft client/server) is planned — see
[docs/35-licensing-governance-and-sustainability.md](docs/35-licensing-governance-and-sustainability.md).
