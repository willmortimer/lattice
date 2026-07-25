# Phase C spatial smoke (trail replay)

**Prompt:** `spatial-demo`

Send this exact prompt to the fake agent provider (agentd or daemon
`FakeAgentBackend`) to emit a deterministic spatial sequence:

1. `step_started` (`search`, label “Search demo page”)
2. `overlay_show` with a `markdown-block` anchor (`fake-demo-page` /
   `fake-demo-block`)
3. `step_completed`

## Desktop verification

1. Start the native desktop shell with the fake agent backend.
2. Open the agent panel and send `spatial-demo`.
3. Confirm the **Trail** section lists the search step.
4. Click the trail step to replay highlight/reveal (Guide mode scrolls;
   Quiet mode highlights without forcing viewport moves).

## Automated checks

```sh
pnpm --filter @lattice/desktop test apps/desktop/src/agent/agentStore.test.ts apps/desktop/src/agent/agentTrailReplay.test.ts
pnpm --filter @lattice/agentd test apps/agentd/src/fake-spatial.test.ts
cargo test -p lattice-daemon fake_spatial_prompt_emits_overlay_sequence
```
