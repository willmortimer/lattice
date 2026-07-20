# Voice lifecycle measurements

Machine-local numbers from `research/voice-lifecycle/scripts/measure.sh`.
Targets for warm UX latency live in
[docs/voice/performance-budget.md](../../docs/voice/performance-budget.md).

## Run metadata

| Field | Value |
| --- | --- |
| Collected (UTC) | 2026-07-20T00:35:10Z |
| Host | Darwin arm64 |
| Git commit | `203a267` |
| Harness | `research/voice-lifecycle/scripts/measure.sh` |

## Daemon residency

| Metric | Measured | Budget reference |
| --- | --- | --- |
| `latticed` cold start → health ready | 33 ms | No explicit budget; keep interactive shell responsive |
| Idle shutdown after last client disconnect | 1023 ms | Configured idle timeout (test uses 1 s) |
| Keep-running: process alive after disconnect | 1502 ms wait + socket present | Product: warm daemon for Quick Note ([quick-note-dictation.md](../../docs/voice/quick-note-dictation.md)) |
| Keep-running: client reconnect | 0 ms | No explicit budget |

## Voice-host crash isolation

| Metric | Measured | Notes |
| --- | --- | --- |
| Supervised fake host kill → voice degraded | pass (integration test) | `voice_crash_isolation.rs` |
| Daemon health during voice outage | pass | Health RPC unaffected |
| Supervisor restart → voice capabilities | 420 ms | Bounded backoff in `voice_host.rs` (200 ms initial) |
| Workspace lease after recovery | pass | Re-open succeeds; no corruption |

## UI / warm-path (not measured here)

Per [performance-budget.md](../../docs/voice/performance-budget.md):

| Metric | Target | Status |
| --- | --- | --- |
| Shortcut → capture start | < 50 ms | **not measured** — no Playwright/Tauri instrumentation in this harness |
| Shortcut → visible overlay | < 100 ms | **not measured** |
| Speech → first provisional text | < 500 ms preferred | **not measured** |
| Push-to-talk release → final text | < 300 ms preferred | **not measured** |

## Helper necessity (Tauri vs login-item agent)

**Recommendation: Tauri menu-bar residency is sufficient; a separate login-item helper is not justified yet.**

Evidence:

- Keep-running keeps `latticed` warm after desktop disconnect without a second process.
- Crash isolation confines voice-host failure to the voice plane; daemon and workspace sessions survive.
- Measured cold-start and reconnect times are within interactive expectations on this host.
- Warm-path UI budgets remain uninstrumented; a helper would not address those gaps.
- No budget failure on residency or recovery was observed that would require out-of-process UI ownership.

Revisit if Milestone 0 / M5 measurements show shortcut→overlay or cold-model paths missing targets on lowest-supported hardware, or if menu-bar-hidden Quick Note requires UI the Tauri process cannot host.

## Raw log excerpt

```text
LIFECYCLE_MEAS: latticed_cold_start_ready_ms=33
LIFECYCLE_MEAS: idle_shutdown_after_disconnect_ms=1023
LIFECYCLE_MEAS: keep_running_wait_after_disconnect_ms=1502
LIFECYCLE_MEAS: keep_running_reconnect_ms=0
LIFECYCLE_MEAS: voice_host_recovery_ms=420
```
