# Nix workflows

Everything automatable in this repo runs through the flake. This page is the
complete inventory: setup, the dev shell, every app, nxr tasks, and how the
desktop / site / browser surfaces relate.

## One-time setup

The `nix` CLI ships with flakes disabled. Enable them once per machine:

```sh
mkdir -p ~/.config/nix
echo 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf
```

Without this, every `nix run`/`nix develop` invocation needs
`--extra-experimental-features 'nix-command flakes'` prepended.

Optional but recommended: [direnv](https://direnv.net). The repo's `.envrc`
loads the dev shell and, when present, a local `.env` (gitignored):

```sh
direnv allow
```

> direnv works even when flakes are *not* enabled globally, because direnv's
> flake support passes the experimental-feature flags itself. Bare `nix run`
> does not — which is why the shell can load fine while `nix run` fails.
> Enable flakes globally (above) and both work.

Optional: install [nxr](https://github.com/willmortimer/nxr) on your user
profile so `nxr` works outside `nix develop` (the flake also pins it and puts
it on `PATH` inside the shell):

```sh
nix profile install .#nxr
```

## The dev shell

```sh
nix develop        # or just cd in, with direnv
```

Provides: rustc, cargo, rustfmt, clippy, rust-analyzer, node 22, pnpm,
pkg-config, **nxr** (plus Tauri's GTK/WebKit stack on Linux). macOS app
bundling additionally needs Xcode Command Line Tools / Xcode (outside nix).

Session-local nxr completion is enabled via `nxr.shellIntegration` (no global
dotfile writes). After `direnv allow`, `nxr` should tab-complete in zsh/bash
inside the shell.

## Dev shells

| Shell | Activate | For |
| --- | --- | --- |
| **default** | `direnv` / `nix develop` | Day-to-day Rust, desktop, notebooks, local `desktop-install` |
| **ops** | `nix develop .#ops` | Cloudflare / site publish only (node, pnpm, wrangler, sops) |

**Do not** put `desktop-install` or Apple notarization tooling in `ops`. That
path needs the full Rust/Tauri toolchain plus Xcode `codesign` — the opposite
of a light publish shell. Apple values in `secrets/apple.env` decrypt via
direnv into the **default** shell; use them from there (`nxr desktop-install`).

`ops` exists so every direnv reload does not pull wrangler/`npx` into the
default environment (and so we avoid nixpkgs’ broken multi‑GiB wrangler
derivation on Darwin).

## Ops shell (Cloudflare / site publish)

Separate from the heavy default shell. Activate only when debugging wrangler
interactively:

```sh
nix develop .#ops
```

Provides: node 22, pnpm, **wrangler** (thin `npx wrangler@4` wrapper — not
nixpkgs’ workers-sdk build), plus `lattice-site-build`, `lattice-site-deploy`,
and `lattice-docs-sync`.

direnv keeps loading `.#default` via `use flake`. Do **not** change `.envrc`
for day-to-day work — open an ops shell in a second terminal when you need
Cloudflare CLI (`whoami`, `login`, project list).

For a normal deploy you do **not** need ops if the API token is loaded:

```sh
nix run .#site-deploy
```

### How `wrangler login` works with Nix

The ops shell puts a small `wrangler` shim on `PATH` that runs
`npx --yes wrangler@4`. OAuth tokens are **not** stored in the Nix store:

1. Run `wrangler login` inside `nix develop .#ops` (needs a browser; interactive).
2. Wrangler writes credentials under your home directory
   (`~/Library/Preferences/.wrangler/` on macOS).
3. Later `wrangler whoami` / `nix run .#site-deploy` reuse that login until it
   expires.
4. For CI or non-interactive shells, set `CLOUDFLARE_API_TOKEN` instead (see
   [environment.md](./environment.md)).

```sh
nix develop .#ops
wrangler login
wrangler whoami
# build + deploy to https://lattice-dop.pages.dev/
lattice-site-deploy
# or from any shell after login / with sops token:
nix run .#site-deploy
```

`site-deploy` builds the site, then `cd site` and runs `wrangler pages deploy`
so [`site/wrangler.toml`](../../site/wrangler.toml) (`pages_build_output_dir =
dist`) applies.

Prefer an API token via sops ([secrets/README.md](../../secrets/README.md)):

```sh
# direnv decrypts secrets/cloudflare.env into the environment
nix run .#site-deploy

# one-shot without direnv
sops exec-env secrets/cloudflare.env -- nix run .#site-deploy
```

### Tag-only CI deploy

Pushing to `main` does **not** deploy the site (avoids burning Actions minutes
on high-frequency pushes). Creating a version tag does:

```sh
git tag v0.1.0
git push origin v0.1.0
```

Workflow: [`.github/workflows/site-deploy.yml`](../../.github/workflows/site-deploy.yml).
Requires GitHub Actions secrets `CLOUDFLARE_API_TOKEN` and optional
`CLOUDFLARE_ACCOUNT_ID` (same values as sops; CI does not use the age key).
`workflow_dispatch` is also enabled for a manual one-off from the Actions UI.

Desktop notarized releases can share the same tag trigger later; they are not
wired yet.

> We intentionally avoid `nixpkgs#wrangler`: it rebuilds the Cloudflare
> workers-sdk monorepo (multi‑GiB) and has been failing on Darwin (`EBADF`
> during tsup). The npm-published CLI is enough for Pages. First `wrangler`
> invocation needs network to populate the npx cache; after that it is local.

## Runners

Prefer **nxr** for day-to-day work. Every leaf is still a normal flake app, so
`nix run` remains a first-class escape hatch.

| Form | Example |
| --- | --- |
| nxr app | `nxr desktop-dev` |
| nxr task DAG | `nxr task codegen -j 2` |
| nix run | `nix run .#desktop-dev` |
| legacy shell command | `lattice-desktop-dev` (inside `nix develop`) |

```sh
nxr list                 # apps + tasks
nxr graph codegen        # mermaid/text/dot via --format
nxr task validate -j 4   # parallel ready-set scheduling
nxr task check           # monolithic CI gate (alias: nxr task ci)
nxr desktop-install      # macOS local signed install (needs .env)
```

## Apps

Each app exists in three equivalent forms:

- `nxr <app>` / `nix run .#<app>` — from anywhere (nxr from profile or shell)
- `lattice-<app>` — plain command inside the dev shell

Run them from the repo root (they use relative paths).

| App | What it does |
| --- | --- |
| `test` | `cargo test --workspace` |
| `lint` | clippy with `-D warnings` + `rustfmt --check` |
| `fmt` | format all Rust code |
| `check` | everything CI runs: fmt check, clippy, tests, `pnpm install --frozen-lockfile`, desktop + site builds |
| `site-dev` | Astro **marketing/docs** site (usually <http://localhost:4321>) |
| `site-build` | static site build (syncs docs content first via `prebuild`) |
| `site-deploy` | build + `wrangler pages deploy` to Cloudflare Pages (`lattice-dop`) |
| `docs-sync` | regenerate `site/src/content/docs/` from `docs/` |
| `compile-theme` | compile `themes/*.theme.yaml` → desktop/site CSS (+ Pixi) tokens |
| `compile-templates` | validate template packages → embedded Rust and browser catalogs |
| `desktop-dev` | Native window **+ Vite HMR** on :5173 (frontend hot-reload); seeds **First Look** in `target/dev-home` |
| `desktop-web` | Browser-only React UI on :5173 (demo workspace; no Tauri) |
| `desktop-perf` | Playwright browser perf harness against the Vite demo (see [perf-harness.md](./perf-harness.md)) |
| `desktop-perf-tauri` | Native WebView perf via `tauri-plugin-playwright` (see [perf-harness.md](./perf-harness.md)) |
| `desktop` | Native window **without Vite** — reuses `apps/desktop/dist` if present, else builds once |
| `desktop-build` | release binary, unbundled (`tauri build --no-bundle`; macOS adds `--features voice-embedded`) |
| `desktop-ui-build` | Vite production build for `@lattice/desktop` only |
| `desktop-install` | macOS: `tauri build --bundles app --features voice-embedded`, codesign, bundle Swift voice/audio dylibs, install to `/Applications/Lattice.app` |
| `ok` | no-op success (join node for nxr task DAGs) |

### Notable tasks (orchestration)

Tasks coordinate apps; they do not replace them. Useful graphs:

| Task | What it runs |
| --- | --- |
| `codegen` (alias `compile`) | `compile-theme` ∥ `compile-templates` |
| `validate` | `lint` ∥ `test` ∥ `desktop-ui-build` ∥ `site-build` |
| `check` (alias `ci`) | monolithic `apps.check` (what CI should keep calling) |
| `desktop-install` (alias `install`) | local signed macOS install |

CI should run exactly one **blocking** thing: `nix run .#check` (or
`nxr task check`). Browser perf runs separately as a non-blocking GitHub
Action on `main` ([`desktop-perf.yml`](../../.github/workflows/desktop-perf.yml));
see [perf-harness.md](./perf-harness.md). Tauri perf is not in CI.

### Local macOS install

`desktop-install` is for **your** Mac (Apple Development identity). It is not
Developer ID + notarization — other machines will still Gatekeeper-block until
you have a paid Apple account and notarize.

Requires (via `.env` + direnv, or exported in the shell):

- `APPLE_SIGNING_IDENTITY` — e.g. `Apple Development: you@example.com (…)`
- `APPLE_TEAM_ID` — recommended; required later for notarization

Optional: `LATTICE_INSTALL_DIR` (default `/Applications`).

```sh
nxr desktop-install
# or: nix run .#desktop-install
```

macOS installs enable `--features voice-embedded` (same as `nxr desktop-dev`) and
copy `libLatticeVoiceBridge.dylib` / `libLatticeAudioBridge.dylib` into the
`.app` so Settings → Voice works. Re-run install after pulling voice changes.

### Three different “web” surfaces

| Surface | How to run | URL (typical) | What it is |
| --- | --- | --- | --- |
| **Desktop app (native, HMR)** | `nxr desktop-dev` | Tauri window ← Vite :5173 | Real Lattice shell with hot reload |
| **Desktop app (native, no Vite)** | `nxr desktop` | Tauri window ← static `dist` | Real shell; rebuild UI with `pnpm --filter @lattice/desktop build` when needed |
| **Desktop frontend only (browser)** | `nxr desktop-web` | <http://localhost:5173> | Same React UI, **demo fixture**, no filesystem |
| **Marketing / docs site** | `nxr site-dev` | Astro (often :4321) | Public site + Starlight docs |
| **Cell / Dev Container demo** | `./scripts/devcontainer/web` (+ `site`) | :5173 / :4321 on `0.0.0.0` | Same browser + site surfaces without Nix; see [devcontainer.md](./devcontainer.md) |

### Why `desktop-dev` also starts :5173

`tauri dev` is two processes by design:

1. **Vite** — serves the React UI with HMR on port **5173**.
2. **Rust / Tauri** — native WebView pointed at that Vite URL.

Seeing Vite on 5173 alongside the native window is expected for `desktop-dev`. Prefer `nxr desktop` when you want the native app without a Vite process.

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

`nxr desktop-dev` (and `pnpm --filter @lattice/desktop tauri:dev`) set
`LATTICE_DEV_HOME` to an absolute `target/dev-home` under the repo root so local
Tauri development uses an isolated profile instead of `~/Lattice`. They also set
`LATTICE_DEV_RESET_DEMO=1` so **First Look** is wiped and re-seeded from the
current `demo` template on every launch (use `tauri:dev:keep` to preserve edits).
**Debug** builds launched without any profile env vars also default to
`target/dev-home` (relative to the process cwd) and seed First Look on first run;
set `LATTICE_HOME` or `LATTICE_FORCE_PROD_HOME=1` to opt into real `~/Lattice`
Personal seeding instead. Release builds always use `~/Lattice` unless overridden.
First-run / reset in dev-home mode seeds the **First Look** demo template:

```text
target/dev-home/
├── Workspaces/
│   └── First Look/        # demo / kitchen-sink template
│       ├── lattice.yaml
│       ├── Home.md
│       ├── CRM.data
│       ├── Projects/Delivery.data
│       ├── Data/Metrics.data
│       ├── OKRs.data
│       ├── Product/
│       └── …
├── Settings/
└── State/
```

Delete `target/dev-home` (or rely on `LATTICE_DEV_RESET_DEMO`) to regenerate.
Your real `~/Lattice` profile is untouched.

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
| `tauri build` (bundled) / `desktop-install` fails | Needs real Xcode or CLT for **codesign**; the script sets `DEVELOPER_DIR` only after the Cargo build. |
| `libduckdb-sys` fails with `uint8_t` / `intmax_t` / `_CTYPE_*` under `desktop-install` | Do not export Xcode’s `DEVELOPER_DIR` for the Cargo step — it mixes Xcode SDK headers with Nix libcxx. Keep the flake’s Nix apple-sdk for compile; Xcode only for codesign. Wipe the broken cache with `cargo clean -p libduckdb-sys` if a failed release build left junk under `target/release/build/`. |
| `APPLE_SIGNING_IDENTITY: unbound variable` | Load `.env` via direnv (`dotenv_if_exists`) or export the var before `desktop-install`. |
| Browser on :5173 shows “Engineering Workspace” | That is the **demo fixture** (`demoWorkspace.generated.ts` from the `demo` template), not your disk. Use the Tauri window to open a real folder. |
| Want Astro but ran `desktop-dev` | Use `nxr site-dev` instead. |

## How it fits together

- [flake.nix](../../flake.nix) — flake-parts + [nxr](https://github.com/willmortimer/nxr) module; toolchain, `nxr.apps`, `nxr.tasks`, shell integration.
- [.envrc](../../.envrc) — `use flake` + `dotenv_if_exists .env`.
- `flake.lock` — pinned nixpkgs and nxr; update deliberately with `nix flake update`.
- [environment.md](./environment.md) — env vars (`LATTICE_DEV_HOME`, Apple signing, etc.).
