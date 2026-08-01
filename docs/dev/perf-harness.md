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

## Warm-shell critical path

Warm reload budgets measure time until workspace title, resource tree, and
activity rail are visible. On adopt, Lattice must not await wiki-link catalog
refresh or index rebuild before painting chrome — those run in the background
(`refresh_resource_catalog` / `rebuild_index`). Theme catalog load also starts
after snapshot adopt and should not gate the chrome selectors above.

Default CI budgets (overridable via env):

| Metric | Default |
| --- | --- |
| Cold shell | 5000 ms (`LATTICE_PERF_SHELL_COLD_MS`) |
| Warm shell | 1500 ms (`LATTICE_PERF_SHELL_WARM_MS`) |
| Page open | 2500 ms (`LATTICE_PERF_PAGE_OPEN_MS`) |
| Page scroll | 1000 ms (`LATTICE_PERF_PAGE_SCROLL_MS`) |

Documented product target remains warm shell in 300–500 ms on representative
hardware (`documentedTargets.shellWarmMs`). Current CI warm-shell (1.5 s) and
page-open (2.5 s) budgets are **intermediate smoke gates**, not evidence that
the product target is met.

### Measurement roadmap (beyond smoke budgets)

Add harness coverage for:

| Signal | Why |
| --- | --- |
| Keystroke-to-paint p50/p95 | Proves editor hot path, not just open latency |
| React commits per 100 characters | Catches shell re-render leaks |
| Long tasks over 50 ms | Main-thread jank |
| File-tree scroll at 10k and 100k entries | Catalog/virtualization scale |
| Agent thread at 1k messages | Transcript/virtualization scale |
| Heap retained after closing editors | Suspension / leak |
| Initial JS, CSS, font bytes by entry | Startup payload |
| Cold process launch vs warm process launch | Separate process vs reload |

When profiling regressions, prefer First Look tree virtualization, theme
resolve IPC, font-pack CSS (only the active pack loads at startup), and
`ensure_home` / `open_workspace` / `prepare_quick_note` scan cost over expanding
the Playwright harness itself.

### Sprint stubs (hotpath + agent workbench)

Explicit harness stubs and planned coverage from the desktop hotpath and agent
workbench sprints. **Unit** = Vitest today; **Playwright** = perf or smoke stub
not yet automated (no new CI budget claimed).

| Signal | Automation today | Notes |
| --- | --- | --- |
| Serialized save failure / `retry()` (no retry-spin) | Unit (`serializedSave.test.ts`) | Playwright stub: inject save fault, assert latch + Cmd+S recovery |
| Per-session save chrome (`saveStatusBySessionId`) | Unit (`desktopUiStore.test.ts`) | Covered at store boundary; no Playwright stub yet |
| Agent hydration / composer gate | Not yet automated | Playwright stub: open thread, assert composer disabled until hydration ready |
| Agent thread at 1k messages | Playwright stub | Transcript / assistant-ui virtualization scale (sharpen when harness exists) |
| File-tree scroll at 10k entries | Playwright stub | Catalog / First Look virtualization scale (100k remains roadmap) |
| Workbench layout resize smoke | Playwright stub (optional) | Agent panel `react-resizable-panels` drag without jank regression |
| Query-backed shell panes (threads, settings) | Playwright stub | `@tanstack/react-query` adoption — stub only, not in CI |

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

Related (not a perf budget / not CI):

- CRM Wave 2 Tauri smoke: `e2e/data/crm.smoke.tauri.spec.ts` —
  `pnpm --filter @lattice/desktop test:crm:tauri`
- Analytics Tauri smoke: `e2e/data/analytics.smoke.tauri.spec.ts` —
  `pnpm --filter @lattice/desktop test:analytics:tauri`
  (Orders Preview / Vega chart / canvas Fit; First Look via
  `LATTICE_DEV_RESET_DEMO`)
- Proposal inbox Tauri smoke: `e2e/data/proposal.smoke.tauri.spec.ts` —
  `pnpm --filter @lattice/desktop test:proposal:tauri`
  (demo `create_demo_proposal` seed → approve → ⌘Z undo)

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

Browser harness is optional for `nix run .#check`. GitHub Actions runs it on
push and pull requests to `main` via
[`.github/workflows/desktop-perf.yml`](../../.github/workflows/desktop-perf.yml):

- **Runner:** `ubuntu-latest`
- **Command:** `pnpm --filter @lattice/desktop test:perf` (Chromium + Vite demo)
- **Blocking:** no — the perf step uses `continue-on-error: true` so soft budget
  regressions surface in the check without failing the workflow
- **Tauri:** not in CI — native WebView perf stays local (`test:perf:tauri` or
  `nix run .#desktop-perf-tauri`)

Local reproduction of the CI job:

```sh
pnpm install --frozen-lockfile
pnpm --filter @lattice/desktop exec playwright install --with-deps chromium
pnpm --filter @lattice/desktop test:perf
```

## Dependencies

| Package | License | Role | Removal |
| --- | --- | --- | --- |
| `tauri-plugin-playwright` `0.4.1` | MIT | Rust socket bridge (initialized only with `e2e-testing`) | Drop feature + capability + dep |
| `@srsholmes/tauri-playwright` `0.4.1` | MIT | Node fixture / `TauriPage` API | Drop fixtures + tauri project |
| `@playwright/test` | Apache-2.0 | Test runner | Keep for browser harness |
