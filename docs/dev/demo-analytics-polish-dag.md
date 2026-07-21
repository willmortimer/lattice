# Demo Analytics Polish DAG

**Status:** Active  
**Created:** 2026-07-20  
**BASE:** `main`  
**Integration branch:** `feat/demo-analytics-polish`  
**Models:** `cursor-grok-4.5-high` only (`best-of-n-runner` worktrees)

Waves:

1. A — P3A01 ‖ P3A05 ‖ P3A06  
2. B — P3A02 ‖ P3A03 (after P01)  
3. C — P3A04 (after P01+P02)  
4. Ship — validate + PR → `main`

See Cursor plan `demo_analytics_polish` for full handoff packets.

## Local verification (not CI)

After P3A01 + P3A02 land on the integration branch, native analytics confidence:

```sh
pnpm --filter @lattice/desktop test:analytics:tauri
```

Spec: `apps/desktop/e2e/data/analytics.smoke.tauri.spec.ts` (Orders Preview /
Revenue by day Vega / Product Strategy canvas Fit). Same
`LATTICE_DEV_RESET_DEMO` First Look seed pattern as `test:crm:tauri`.
