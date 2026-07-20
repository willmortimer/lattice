# Voice lifecycle measurements

Focused harness for **daemon residency**, **keep-running vs idle shutdown**, and
**voice-host crash isolation**. Complements the warm-path UX budgets in
[docs/voice/performance-budget.md](../../docs/voice/performance-budget.md)
(shortcut → overlay, speech → provisional) which are not measured here.

## What this measures

| Area | Evidence |
| --- | --- |
| `latticed` cold start | Time from spawn to health-check ready |
| Idle shutdown | Time from last client disconnect to process exit |
| Keep-running | Socket + process survive disconnect; reconnect latency |
| Voice crash isolation | Supervised fake `lattice-voice-host` kill → degraded → auto-restart; daemon + workspace lease survive |

## Run

From the repository root:

```sh
./research/voice-lifecycle/scripts/measure.sh
```

The script builds `latticed` and `lattice-voice-host`, runs integration tests,
parses `LIFECYCLE_MEAS:` lines, and updates `MEASUREMENTS.md` timestamps.

Focused test targets:

```sh
cargo test -p lattice-daemon --test lifecycle_measure -- --nocapture
cargo test -p lattice-daemon --test voice_crash_isolation -- --nocapture
cargo test -p lattice-daemon --test idle_shutdown
```

## CI

All tests above are headless, use the fake voice backend, and require no models.
They are suitable for `cargo test --workspace` on Linux and macOS.

## Out of scope

- Full hardware matrix (see performance-budget benchmark matrix)
- Playwright / Tauri UI latency (shortcut → overlay)
- Login-item helper or separate menu-bar agent
