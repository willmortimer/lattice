# Nix workflows

Everything automatable in this repo runs through the flake. This page is the
complete inventory: setup, the dev shell, every task, and how the desktop /
site / browser surfaces relate.

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
| `site-dev` | Astro **marketing/docs** site (usually <http://localhost:4321>) |
| `site-build` | static site build (syncs docs content first via `prebuild`) |
| `docs-sync` | regenerate `site/src/content/docs/` from `docs/` |
| `compile-theme` | compile `themes/*.theme.yaml` → desktop/site CSS (+ Pixi) tokens |
| `compile-templates` | validate template packages → embedded Rust and browser catalogs |
| `desktop-dev` | Native window **+ Vite HMR** on :5173 (frontend hot-reload); seeds **First Look** in `target/dev-home` |
| `desktop-web` | Browser-only React UI on :5173 (demo workspace; no Tauri) |
| `desktop` | Native window **without Vite** — reuses `apps/desktop/dist` if present, else builds once |
| `desktop-build` | release binary, unbundled (`tauri build --no-bundle`) |

CI should run exactly one thing: `nix run .#check`.

### Three different “web” surfaces

| Surface | How to run | URL (typical) | What it is |
| --- | --- | --- | --- |
| **Desktop app (native, HMR)** | `nix run .#desktop-dev` | Tauri window ← Vite :5173 | Real Lattice shell with hot reload |
| **Desktop app (native, no Vite)** | `nix run .#desktop` | Tauri window ← static `dist` | Real shell; rebuild UI with `pnpm --filter @lattice/desktop build` when needed |
| **Desktop frontend only (browser)** | `nix run .#desktop-web` | <http://localhost:5173> | Same React UI, **demo fixture**, no filesystem |
| **Marketing / docs site** | `nix run .#site-dev` | Astro (often :4321) | Public site + Starlight docs |
| **Cell / Dev Container demo** | `./scripts/devcontainer/web` (+ `site`) | :5173 / :4321 on `0.0.0.0` | Same browser + site surfaces without Nix; see [devcontainer.md](./devcontainer.md) |

### Why `desktop-dev` also starts :5173

`tauri dev` is two processes by design:

1. **Vite** — serves the React UI with HMR on port **5173**.
2. **Rust / Tauri** — native WebView pointed at that Vite URL.

Seeing Vite on 5173 alongside the native window is expected for `desktop-dev`. Prefer `nix run .#desktop` when you want the native app without a Vite process.

### Lattice home directory

Production first-run (**Create Lattice home**) creates:

```text
~/Lattice/                 # Lattice home (user-level, not a workspace)
├── Workspaces/
│   └── Personal/          # first workspace (personal template)
│       ├── lattice.yaml
│       ├── Home.md        # landing page inside the workspace
│       ├── Inbox/
│       ├── Projects/
│       ├── Product/
│       └── …
├── Settings/              # versioned human-editable preferences
└── State/
    └── desktop.sqlite     # recents, sessions, and shell state
```

`nix run .#desktop-dev` (and `pnpm --filter @lattice/desktop tauri:dev`) set
`LATTICE_DEV_HOME` to an absolute `target/dev-home` under the repo root so local
Tauri development uses an isolated profile instead of `~/Lattice`. **Debug**
builds launched without any profile env vars also default to `target/dev-home`
(relative to the process cwd) and seed First Look on first run; set
`LATTICE_HOME` or `LATTICE_FORCE_PROD_HOME=1` to opt into real `~/Lattice`
Personal seeding instead. Release builds always use `~/Lattice` unless overridden.
First-run in dev-home mode seeds the **First Look** demo template:

```text
target/dev-home/
├── Workspaces/
│   └── First Look/        # demo / kitchen-sink template
│       ├── lattice.yaml
│       ├── Home.md
│       ├── CRM.data
│       ├── Product/
│       └── …
├── Settings/
└── State/
```

Delete `target/dev-home` to regenerate the dev profile from scratch. Your real
`~/Lattice` profile is untouched.

`Personal` is the production workspace folder; `Home.md` is just a page inside
it. Run `lattice templates list` for the current template catalog (gallery,
sample, and legacy ids). Examples: `personal`, `project`, `blank`, `demo`
(First Look sample), and `team` (legacy).

See [environment.md](./environment.md) for `LATTICE_DEV_HOME` and `LATTICE_HOME`.

## Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| `experimental Nix feature 'nix-command' is disabled` | Flakes not enabled for the bare CLI. Do the one-time setup above. (direnv succeeding while this fails is expected — see note.) |
| `Path 'X' ... is not tracked by Git` | Flakes only see git-tracked files. `git add` the new file (staged is enough, no commit needed). |
| `.envrc is blocked` | `direnv allow` after reviewing the file. |
| `Git tree ... is dirty` warning | Harmless; nix is telling you the working tree has uncommitted changes. |
| Task can't find `site/scripts/...` or workspace packages | Run tasks from the repo root. |
| `tauri build` (bundled) fails outside the shell | macOS bundling needs Xcode CLT; the nix shell doesn't provide it. |
| Browser on :5173 shows “Engineering Workspace” | That is the **demo fixture** (`demoWorkspace.generated.ts` from the `demo` template), not your disk. Use the Tauri window to open a real folder. |
| Want Astro but ran `desktop-dev` | Use `nix run .#site-dev` instead. |

## How it fits together

- [flake.nix](../../flake.nix) — toolchain list, `tasks` (name → script),
  exposed as both flake `apps` and dev-shell `lattice-*` commands.
- [.envrc](../../.envrc) — `use flake`, for direnv users.
- `flake.lock` — pinned nixpkgs; update deliberately with `nix flake update`.
- [environment.md](./environment.md) — env vars (`LATTICE_DEV_HOME`, `LATTICE_HOME`, etc.).
