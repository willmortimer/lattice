# Performance harness

Lattice ships two Playwright surfaces for desktop performance budgets from
[Frontend, rendering, and performance](../23-frontend-rendering-and-performance.md):

| Mode | How | What it measures |
| --- | --- | --- |
| **Browser** (default) | Chromium + Vite demo (`inBrowser`) | React shell, tree, page editor without native IPC |
| **Tauri** | Real app WebView via [`tauri-plugin-playwright`](https://crates.io/crates/tauri-plugin-playwright) | Same UI flows on WKWebView / WebView2 / WebKitGTK with real Rust IPC |

Plain Playwright cannot drive WKWebView (no CDP on macOS/Linux). The Tauri path
embeds a socket bridge (`e2e-testing` feature) that `@srsholmes/tauri-playwright`
speaks.

Browser mode intentionally does **not** use `createTauriTest({ mode: "browser" })`:
that helper injects a mock `__TAURI_INTERNALS__`, which would exit Lattice’s
demo fixture and break the Vite harness.

## Run — browser

```sh
pnpm install
pnpm --filter @lattice/desktop exec playwright install chromium
pnpm --filter @lattice/desktop test:perf
```

Nix: `nix run .#desktop-perf`

## Run — Tauri (native WebView)

```sh
pnpm install
pnpm --filter @lattice/desktop test:perf:tauri
```

The runner starts `tauri dev --features e2e-testing` (with `LATTICE_DEV_HOME`),
waits for `/tmp/tauri-playwright.sock`, runs `--project=tauri`, then stops the
app. Reuses an existing socket if you already have:

```sh
pnpm --filter @lattice/desktop tauri:dev:e2e   # terminal 1
pnpm --filter @lattice/desktop exec playwright test --project=tauri   # terminal 2
```

Override the socket with `TAURI_PLAYWRIGHT_SOCKET`. On macOS, native screenshots
on failure need Screen Recording permission for the terminal/app host.

Nix: `nix run .#desktop-perf-tauri`

## What is measured

| Spec | Scenario |
| --- | --- |
| `shell.perf.spec.ts` / `shell.tauri.perf.spec.ts` | Cold/ready shell chrome + warm reload |
| `page.perf.spec.ts` / `page.tauri.perf.spec.ts` | Open `Home.md` until ProseMirror; scroll smoke |

Annotations record wall time, Navigation Timing, and (browser only) Chromium JS
heap via CDP.

## Budgets

| Variable | Default (CI-friendly) | Local profiling suggestion |
| --- | --- | --- |
| `LATTICE_PERF_SHELL_COLD_MS` | `8000` | `3000` |
| `LATTICE_PERF_SHELL_WARM_MS` | `3000` | `500` (doc target) |
| `LATTICE_PERF_PAGE_OPEN_MS` | `4000` | `1000` |
| `LATTICE_PERF_PAGE_SCROLL_MS` | `2000` | `500` |

```sh
LATTICE_PERF_SHELL_WARM_MS=500 pnpm --filter @lattice/desktop test:perf
```

## Vitest boundary

Unit tests remain `pnpm --filter @lattice/desktop test` (Vitest). Perf specs are
Playwright-only via `test:perf` / `test:perf:tauri`.

## CI

Browser harness is optional for `nix run .#check`. Suggested jobs:

```sh
# Fast / Linux CI
pnpm --filter @lattice/desktop exec playwright install --with-deps chromium
pnpm --filter @lattice/desktop test:perf

# Native WebView (macOS runner recommended)
pnpm --filter @lattice/desktop test:perf:tauri
```

## Dependencies

| Package | License | Role | Removal |
| --- | --- | --- | --- |
| `tauri-plugin-playwright` `0.4.1` | MIT | Rust socket bridge (initialized only with `e2e-testing`) | Drop feature + capability + dep |
| `@srsholmes/tauri-playwright` `0.4.1` | MIT | Node fixture / `TauriPage` API | Drop fixtures + tauri project |
| `@playwright/test` | Apache-2.0 | Test runner | Keep for browser harness |
