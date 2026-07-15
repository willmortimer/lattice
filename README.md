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
