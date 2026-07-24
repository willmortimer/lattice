# Thursday Gate A baseline

Recorded baseline for the Thursday integration branch (`thursday/integration`) so later
regressions are attributable to specific packets (T1–T8).

## Snapshot

| Field | Value |
| --- | --- |
| Branch | `thursday/integration` (detached at baseline commit) |
| Commit | `e3b5243da71eb18431e2cefd2d4118ba14b853fa` |
| Commit message | Merge feat/f2-attachment-fields. |
| Recorded | 2026-07-24 |
| Gate | **A** — post-T0 canonical check ledger |

## Toolchain

| Tool | Version |
| --- | --- |
| rustc | 1.91.1 (ed61e7d7e 2025-11-07) |
| cargo | 1.91.1 (ea2d97820 2025-10-10) |
| node | v24.18.0 |
| pnpm | 11.11.0 |
| nix | 2.34.8 (Determinate Nix 3.21.8) |
| nxr | 2.4.1 |

Checks were run from the T0 worktree using host toolchain (`nxr` / `cargo` / `pnpm`), not
inside a Nix dev shell, unless noted.

## Canonical check results

Gate A canonical command: `nxr task check` (flake `check` app: fmt, clippy, workspace tests,
desktop + site builds).

| Step | Command | Result | Elapsed |
| --- | --- | --- | --- |
| Format | `cargo fmt --all -- --check` | **PASS** (after T0 fmt fix) | ~1s |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** (after T0 clippy fixes) | ~9s |
| Rust tests | `cargo test --workspace` | **FAIL** — blocked by `lattice-publish` compile | ~297s |
| Frontend install | `pnpm install --frozen-lockfile` | **PASS** | ~10s |
| Desktop build | `pnpm --filter @lattice/desktop build` | **FAIL** — `DataTableView.tsx` type errors | ~2s |
| Site build | `pnpm --filter @lattice/site build` | **PASS** | ~7s |
| Daemon tests | `cargo test -p lattice-daemon` | **FAIL** — `spawn_helper_launches_binary` | ~288s |
| Python tests | `cd packages/lattice-py && uv run --with pytest --with duckdb --with pyarrow pytest` | **PASS** (13 tests) | ~5s |
| Theme outputs | `pnpm compile-theme` + `git diff` on generated theme files | **PASS** (no drift) | ~1s |
| Template outputs | `pnpm compile-templates` + `git diff` on generated template files | **PASS** (no drift) | ~1s |

**Gate A verdict: RED.** Trivial fmt/clippy drift was fixed in T0; remaining failures are
architectural gaps from incomplete attachment-field integration and daemon spawn reliability.

## T0 fixes applied (trivial drift only)

- `cargo fmt --all` across the workspace (132+ files; rustfmt drift on `thursday/integration`).
- Mechanical Clippy fixes in `lattice-profile`, `lattice-data`, `lattice-datasets`,
  `lattice-core`, `lattice-env`, `lattice-commands`, and `lattice-index` (derivable `Default`,
  loop style, redundant closures/guards, `io::Error::other`, targeted `#[allow]` for
  `too_many_arguments` / `only_used_in_recursion`).

No feature or behavioral changes beyond fmt/clippy hygiene.

## Failure ledger (route to packets)

| Failure | Owner packet | Repro command | Blocking? |
| --- | --- | --- | --- |
| `lattice-publish`: non-exhaustive `CellValue` match — `MultiEnum` and `Attachment` arms missing in `snapshot.rs` | **T1** (attachment staging / field integration) | `cargo test -p lattice-publish` | **Yes** — blocks `cargo test --workspace` |
| Desktop `DataTableView.tsx`: `GridCell` union does not cover attachment/multi-enum column types (`Type '"data"' is not assignable to type 'never'`, `data: string` vs `string[]`) | **T1** (attachment field editors in grid) | `pnpm --filter @lattice/desktop build` | **Yes** — blocks Gate A desktop build |
| Daemon contract: `spawn_helper_launches_binary` times out waiting for spawned socket | **T6** (daemon lifecycle / spawn helper) | `cargo test -p lattice-daemon --test contract spawn_helper_launches_binary` | **Yes** for daemon CI; workspace `cargo test` may skip if publish fails first |
| Python tests require explicit pytest dep in bare `uv run pytest` | **T8** (Python SDK / dev ergonomics) | `cd packages/lattice-py && uv run pytest` (fails); use `uv run --with pytest --with duckdb --with pyarrow pytest` | **No** — tests pass with documented command; dev-env/docs gap |

### Non-blocking / informational

| Item | Notes |
| --- | --- |
| Generated theme/template catalogs | Clean after `pnpm compile-theme` and `pnpm compile-templates` |
| Site static build | Passes independently of desktop attachment grid work |
| `lattice-py` | 13/13 tests pass with `uv run --with pytest --with duckdb --with pyarrow pytest` |

## Packet ownership reference (T1–T8)

| Packet | Scope |
| --- | --- |
| T0 | Baseline ledger (this document) |
| T1 | Attachment staging / reference-only remove |
| T2 | Retry idempotency + proposal dedupe |
| T3 | Canonical e2e smoke (depends T1+T2+T4+T5) |
| T4 | Derived output integrity + atomic promote |
| T5 | Rich proposal previews + subset validation |
| T6 | Daemon job-status unification + spawn reliability |
| T7a | Embedded saved data-view |
| T7b | Embedded forms |
| T8 | Python/MCP parity + First Look |

## Next gates

- **Gate B** (post-T1/T2/T4/T5 + T3): `nxr task check` + governed-loop smoke + attachment/proposal/derived smokes.

### Governed-loop smoke (T3)

Canonical backend integration test for form → workflow → proposal → approve →
derived → refresh → undo on the First Look `demo` template:

```sh
cargo test -p lattice-commands --test governed_loop_smoke -- --nocapture
```

The test provisions a temp workspace from the `demo` template, inserts a
`ContactIntake` row, runs `Automations/Contact intake.workflow.yaml`, previews
and applies the workflow proposal, rebuilds `Derived/ContactBrief.derived.yaml`
(stale → current), checks workflow relationship edges, and undoes the apply.
Boundary diagnostics print to stderr as `[governed-loop] …` lines.

## Gate C — Thursday finish line (2026-07-24)

| Field | Value |
| --- | --- |
| Integration SHA | `52b54103db278fcc3d6042162cdd5fa757389cb5` (+ follow-up T7a casing fix commit) |
| Packets merged | T0–T8, T7a, T7b |

### Spot checks run on integration tip

| Check | Result |
| --- | --- |
| `cargo test -p lattice-commands --test governed_loop_smoke` | **PASS** (~11s) |
| `cargo check -p lattice-commands` | **PASS** |
| `cargo test -p lattice-daemon --test contract spawn_helper_launches_binary` | **PASS** (after T6) |
| `pnpm --filter @lattice/desktop build` | **PASS** (after T7a casing/`BubbleCell` fix) |
| `cargo test -p lattice-publish` | **PASS** (after T1) |

Full `nxr task check` / `cargo test --workspace` was **not** re-run end-to-end in this session; use Gate A commands above for a fresh full suite before demo freeze.

### Known limitations (honest shipped boundaries)

- **Scheduler:** open-session interval only; cron parsed but not executed; no durable closed-desktop registry (Phase 5 / T9).
- **Attachments:** staged until commit; orphan cleanup is explicit (CLI/Tauri), no TTL sweep for abandoned staging dirs; UX polish (open/reveal/drag-drop) deferred.
- **Proposal inbox:** rich previews + subset validation shipped; full filtering/archive lifecycle deferred (P2-3).
- **Interfaces:** embedded forms + saved views shipped; form submit bumps host snapshot revision so sibling embedded data-views refresh (F0).
- **Python SDK:** schema/profile parity for datasets; full read/search/propose Phase 4 surface deferred.
- **Daemon jobs:** list/get/cancel + tray merge for schedule runs; not full durable job queue/recovery.

### Friday-ready demo paths

1. First Look: prepare workspace → agent task schema/profile → proposal → rich review (T5) → approve → interface.
2. OpsDashboard: embedded form submit → Board/data-view refresh (F0); workflow → proposal optional.
3. Derived ContactBrief: stale reasons + atomic rebuild.
4. Tray: desktop + daemon schedule job visibility/cancel (open session).

Friday rehearsal checklist: [friday-demo.md](./friday-demo.md).

### Friday continuation packets

- Wire interface form submit → host snapshot revision refresh for embedded views.
- T9 scheduler durability increments (known-workspace registry, lease vs idle shutdown).
- Proposal inbox filters + open-result actions.
- Attachment inventory UI + staging TTL cleanup.
- Interface builder insert/bind/reorder (P1-14).
- Full Python read/search/context + revision-aware propose helpers.
- Publishing dependency closure; relationship impact analysis UI.

## Re-run baseline

```sh
# From repo root on thursday/integration
nxr task check

# Or decomposed
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm install --frozen-lockfile
pnpm --filter @lattice/desktop build
pnpm --filter @lattice/site build
cargo test -p lattice-daemon
cd packages/lattice-py && uv run --with pytest --with duckdb --with pyarrow pytest
pnpm compile-theme && pnpm compile-templates

# Governed loop
cargo test -p lattice-commands --test governed_loop_smoke -- --nocapture
```
