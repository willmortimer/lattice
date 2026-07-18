# Browser performance harness

Lattice ships a **browser-mode** Playwright harness that measures frontend
performance budgets against the Vite desktop demo (`inBrowser` fixture). It
runs in Chromium against `pnpm --filter @lattice/desktop dev` on
<http://localhost:5173> — the same surface as `nix run .#desktop-web`.

Plain Playwright cannot drive WKWebView on macOS, so this harness does **not**
block on native Tauri E2E. A future path is documented below.

## Why browser-first

| Approach | Status |
| --- | --- |
| **Vite demo in Chromium** (this harness) | Implemented — repeatable, CI-friendly |
| **Tauri + `tauri-plugin-playwright` / WebDriver** | Follow-up — needed for WKWebView parity on macOS |

The demo fixture exercises the real React shell, resource tree, and page editor
without filesystem or Tauri IPC. Budget regressions in shell render and page
open are meaningful even before native WebView automation exists.

## Run

From the repo root (Nix dev shell or plain Node 22 + pnpm):

```sh
pnpm install
pnpm --filter @lattice/desktop exec playwright install chromium
pnpm --filter @lattice/desktop test:perf
```

Equivalent Nix task:

```sh
nix run .#desktop-perf
```

The Playwright config starts the Vite dev server automatically unless one is
already listening on port 5173 (local iteration). Set `CI=1` to always spawn a
fresh server.

HTML report (on failure or retry): `apps/desktop/playwright-report/index.html`.

## What is measured

Specs live under `apps/desktop/e2e/perf/`:

| Spec | Scenario |
| --- | --- |
| `shell.perf.spec.ts` | **Cold** first navigation to workspace chrome (title, resource tree, activity rail) |
| `shell.perf.spec.ts` | **Warm** reload to the same chrome |
| `page.perf.spec.ts` | Open `Home.md` from the resource tree until the ProseMirror editor is visible |
| `page.perf.spec.ts` | Scroll smoke on an open page (demo pages are short) |

Each run records wall-clock time plus Navigation Timing (`domContentLoaded`,
`load`) and, on Chromium, JS heap via CDP (`Runtime.getHeapUsage`).

## Budgets

Product targets are in
[Frontend, rendering, and performance](../23-frontend-rendering-and-performance.md):

- **Warm shell visible:** 300–500 ms on representative hardware.

The harness asserts **soft** ceilings so CI stays stable on shared runners.
Tighten locally when profiling on your machine.

| Variable | Default (CI-friendly) | Local profiling suggestion |
| --- | --- | --- |
| `LATTICE_PERF_SHELL_COLD_MS` | `8000` | `3000` |
| `LATTICE_PERF_SHELL_WARM_MS` | `3000` | `500` (doc target) |
| `LATTICE_PERF_PAGE_OPEN_MS` | `4000` | `1000` |
| `LATTICE_PERF_PAGE_SCROLL_MS` | `2000` | `500` |

Example — assert doc-level warm shell on a fast laptop:

```sh
LATTICE_PERF_SHELL_WARM_MS=500 pnpm --filter @lattice/desktop test:perf
```

Failures print the measured value and the active budget. Annotations in the
Playwright report include navigation and heap snapshots for triage.

## Vitest boundary

Unit tests remain `pnpm --filter @lattice/desktop test` (Vitest, `src/**/*.test.ts`).
Perf specs are excluded from Vitest; only `test:perf` runs Playwright.

## CI

The harness is **optional** for repo CI (`nix run .#check` does not run it yet).
Add a dedicated job when runner time allows:

```sh
pnpm --filter @lattice/desktop exec playwright install --with-deps chromium
pnpm --filter @lattice/desktop test:perf
```

## Future: native Tauri perf

When WKWebView automation is available:

1. **`tauri-plugin-playwright`** or platform WebDriver against the embedded
   webview.
2. Reuse the same budget env vars and spec structure; swap `webServer` for a
   `tauri dev` (or packaged app) lifecycle.
3. Add IPC-heavy scenarios (large tables, canvas) that the browser demo cannot
   represent.

Until then, treat this harness as the canonical frontend perf gate for shell
and page surfaces.
