# Nix workflows

Everything automatable in this repo runs through the flake. This page is the
complete inventory: setup, the dev shell, every task, and the failure modes
we have actually hit.

## One-time setup

The `nix` CLI ships with flakes disabled. Enable them once per machine:

```sh
mkdir -p ~/.config/nix
echo 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf
```

Without this, every `nix run`/`nix develop` invocation needs
`--extra-experimental-features 'nix-command flakes'` prepended.

Optional but recommended: [direnv](https://direnv.net). The repo's `.envrc`
loads the dev shell automatically when you `cd` in. First use requires
approval:

```sh
direnv allow
```

> direnv works even when flakes are *not* enabled globally, because direnv's
> flake support passes the experimental-feature flags itself. Bare `nix run`
> does not — which is why the shell can load fine while `nix run` fails.
> Enable flakes globally (above) and both work.

## The dev shell

```sh
nix develop        # or just cd in, with direnv
```

Provides: rustc, cargo, rustfmt, clippy, rust-analyzer, node 22, pnpm,
pkg-config (plus Tauri's GTK/WebKit stack on Linux). macOS app bundling
additionally needs Xcode Command Line Tools (outside nix).

## Tasks

Each task exists in two equivalent forms:

- `nix run .#<task>` — from anywhere, no shell needed, brings its own toolchain
- `lattice-<task>` — plain command available inside the dev shell

Run them from the repo root (they use relative paths).

| Task | What it does |
| --- | --- |
| `test` | `cargo test --workspace` |
| `lint` | clippy with `-D warnings` + `rustfmt --check` |
| `fmt` | format all Rust code |
| `check` | everything CI runs: fmt check, clippy, tests, `pnpm install --frozen-lockfile`, desktop + site builds |
| `site-dev` | Astro dev server for the marketing/docs site |
| `site-build` | static site build (syncs docs content first via `prebuild`) |
| `docs-sync` | regenerate `site/src/content/docs/` from `docs/` |
| `desktop-dev` | `tauri dev` — compiles the Rust shell and opens the native window |
| `desktop-build` | release binary, unbundled (`tauri build --no-bundle`) |

CI should run exactly one thing: `nix run .#check`.

## Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| `experimental Nix feature 'nix-command' is disabled` | Flakes not enabled for the bare CLI. Do the one-time setup above. (direnv succeeding while this fails is expected — see note.) |
| `Path 'X' ... is not tracked by Git` | Flakes only see git-tracked files. `git add` the new file (staged is enough, no commit needed). |
| `.envrc is blocked` | `direnv allow` after reviewing the file. |
| `Git tree ... is dirty` warning | Harmless; nix is telling you the working tree has uncommitted changes. |
| Task can't find `site/scripts/...` or workspace packages | Run tasks from the repo root. |
| `tauri build` (bundled) fails outside the shell | macOS bundling needs Xcode CLT; the nix shell doesn't provide it. |

## How it fits together

- [flake.nix](../../flake.nix) — toolchain list, `tasks` (name → script),
  exposed as both flake `apps` and dev-shell `lattice-*` commands.
- [.envrc](../../.envrc) — `use flake`, for direnv users.
- `flake.lock` — pinned nixpkgs; update deliberately with `nix flake update`.
