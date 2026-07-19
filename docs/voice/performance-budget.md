# Performance Budget

## Scope

Explicit product targets for voice dictation. Performance budgets are product
requirements ([docs/02](../02-principles-and-invariants.md) #25).

## Initial targets

### Warm model

| Metric | Target |
|--------|--------|
| Shortcut to capture start | < 50 ms |
| Shortcut to visible overlay | < 100 ms |
| Speech to first provisional text | < 500 ms preferred |
| Provisional update interval | 100–300 ms |
| Push-to-talk release to final text | < 300 ms preferred |
| Audio frame loss | 0 under normal load |
| UI-thread blocking | 0 |

### Cold model

- App remains interactive throughout loading
- Visible preparation state appears immediately
- Model preparation never occurs silently
- Quick Note may capture audio while the model finishes loading, within a
  bounded buffer limit

### Quality

- No provisional transcript enters persistent state
- Final transcript replaces the complete provisional range
- One finalized utterance creates one undoable transaction
- False destructive command execution rate: effectively zero

## Benchmark matrix

Test at least:

- Lowest-supported Apple Silicon Mac
- Current base MacBook Air
- Higher-end MacBook Pro
- Battery mode
- Thermal throttling
- Simultaneous indexing or sync activity
- Multiple Lattice windows
- Model cold and warm states

## Testing requirements

Automated micro-benchmarks where feasible; hardware matrix in CI or scheduled
dogfood harness ([docs/dev/perf-harness.md](../dev/perf-harness.md) patterns).

## Open questions

- First-partial latency on oldest supported M-series (research Q5)
- Whether offline re-decode stays within finalization budget (research Q6)

## Acceptance criteria

- [ ] Warm-path targets measured on lowest-supported device
- [ ] UI-thread blocking is zero in instrumentation
- [ ] Cold path never blocks interaction without visible state
