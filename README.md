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
└── flake.nix             # Nix dev shell, flake apps, and nxr tasks
```

## Status

Local desktop substrate is shipping: headless core + CLI, Tauri shell,
pages/canvas/search/voice, SQLite data apps (views, forms, interfaces),
DuckDB `.dataset` analytics (Perspective, Vega-Lite, Profile, Plan, MapLibre
maps), notebooks (native `ipykernel` + Pyodide), tasks/workflows/proposals/
artifacts, and the `latticed` daemon — plus the marketing/docs site. The
specification in `docs/` is intentionally broader than what is polished today —
see [roadmap](docs/29-roadmap.md) for residual gaps and later phases.

## Development

With Nix (recommended):

```sh
nix develop        # rust toolchain, node, pnpm, tauri prerequisites
```

Requires flakes; if not enabled globally, add
`experimental-features = nix-command flakes` to `~/.config/nix/nix.conf`
or pass `--extra-experimental-features 'nix-command flakes'`. Direnv users
can `direnv allow` to load the shell automatically via `.envrc`.

Common tasks are flake apps. Prefer [nxr](https://github.com/willmortimer/nxr)
(`nxr list`, `nxr <app>`, `nxr task <name>`); `nix run .#<app>` and
`lattice-<app>` inside the shell still work.
Full guide: [docs/dev/nix-workflows.md](docs/dev/nix-workflows.md).
Environment variables: [docs/dev/environment.md](docs/dev/environment.md).

For a Linux Dev Container / DevCell cell (browser demo + docs site, no Tauri):
[docs/dev/devcontainer.md](docs/dev/devcontainer.md).

```sh
nxr list                  # discover apps + tasks
nxr test                  # cargo test --workspace
nxr lint                  # clippy -D warnings + rustfmt check
nxr fmt                   # rustfmt
nxr task check            # everything CI would run (alias: nxr task ci)
nxr task codegen -j 2     # theme ∥ templates
nxr compile-theme         # themes/*.theme.yaml → CSS/TS tokens
nxr compile-templates     # template packages → embedded catalogs
nxr site-dev              # Astro marketing/docs site
nxr site-build            # static site build
nxr docs-sync             # regenerate site docs content from docs/
nxr desktop-dev           # Tauri native window + Vite HMR on :5173
nxr desktop-web           # browser-only demo UI
nxr desktop               # native without Vite (reuses dist)
nxr desktop-build         # tauri release build, unbundled
nxr desktop-install       # macOS: sign + install to /Applications
nxr desktop-release       # macOS: Developer ID + notarytool + DMG
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

## Notable desktop analytics dependencies

| Package | License | Role | Size (approx.) |
| --- | --- | --- | --- |
| `@finos/perspective` + `@finos/perspective-viewer` + datagrid | Apache-2.0 | Analytical grid for `.dataset` Arrow IPC | ~15 MB unpacked (WASM); loaded only with the dataset renderer chunk |
| `@glideapps/glide-data-grid` | MIT | Mutable `.data` app grid — **not** replaced by Perspective | existing |

## License

Lattice is licensed under the [GNU Affero General Public License v3.0 or
later](LICENSE) (`AGPL-3.0-or-later`). See
[ADR 0031](docs/decisions/0031-agpl-3-or-later.md) and
[docs/35-licensing-governance-and-sustainability.md](docs/35-licensing-governance-and-sustainability.md).
