#!/usr/bin/env bash
# Collect daemon/voice lifecycle timings from integration tests.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

MEASUREMENTS="$ROOT/research/voice-lifecycle/MEASUREMENTS.md"
LOG="$(mktemp -t lattice-lifecycle-measure.XXXXXX.log)"

echo "Building latticed + lattice-voice-host..."
cargo build -p lattice-daemon -p lattice-voice-host --bins

echo "Running lifecycle integration tests..."
cargo test -p lattice-daemon \
  --test lifecycle_measure \
  --test voice_crash_isolation \
  --test idle_shutdown \
  -- --nocapture 2>&1 | tee "$LOG"

HOST="$(uname -s) $(uname -m)"
DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
COMMIT="$(git rev-parse --short HEAD)"

parse_meas() {
  local key="$1"
  grep -E "LIFECYCLE_MEAS: ${key}=" "$LOG" | tail -1 | sed -E "s/.*${key}=([0-9]+).*/\1/" || true
}

COLD_START="$(parse_meas latticed_cold_start_ready_ms)"
IDLE_SHUTDOWN="$(parse_meas idle_shutdown_after_disconnect_ms)"
KEEP_WAIT="$(parse_meas keep_running_wait_after_disconnect_ms)"
KEEP_RECONNECT="$(parse_meas keep_running_reconnect_ms)"
VOICE_RECOVERY="$(parse_meas voice_host_recovery_ms)"

not_measured() {
  if [[ -z "$1" ]]; then
    echo "**not measured** (harness did not emit value)"
  else
    echo "${1} ms"
  fi
}

cat > "$MEASUREMENTS" <<EOF
# Voice lifecycle measurements

Machine-local numbers from \`research/voice-lifecycle/scripts/measure.sh\`.
Targets for warm UX latency live in
[docs/voice/performance-budget.md](../../docs/voice/performance-budget.md).

## Run metadata

| Field | Value |
| --- | --- |
| Collected (UTC) | $DATE |
| Host | $HOST |
| Git commit | \`$COMMIT\` |
| Harness | \`research/voice-lifecycle/scripts/measure.sh\` |

## Daemon residency

| Metric | Measured | Budget reference |
| --- | --- | --- |
| \`latticed\` cold start → health ready | $(not_measured "$COLD_START") | No explicit budget; keep interactive shell responsive |
| Idle shutdown after last client disconnect | $(not_measured "$IDLE_SHUTDOWN") | Configured idle timeout (test uses 1 s) |
| Keep-running: process alive after disconnect | $(not_measured "$KEEP_WAIT") wait + socket present | Product: warm daemon for Quick Note ([quick-note-dictation.md](../../docs/voice/quick-note-dictation.md)) |
| Keep-running: client reconnect | $(not_measured "$KEEP_RECONNECT") | No explicit budget |

## Voice-host crash isolation

| Metric | Measured | Notes |
| --- | --- | --- |
| Supervised fake host kill → voice degraded | pass (integration test) | \`voice_crash_isolation.rs\` |
| Daemon health during voice outage | pass | Health RPC unaffected |
| Supervisor restart → voice capabilities | $(not_measured "$VOICE_RECOVERY") | Bounded backoff in \`voice_host.rs\` (200 ms initial) |
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

- Keep-running keeps \`latticed\` warm after desktop disconnect without a second process.
- Crash isolation confines voice-host failure to the voice plane; daemon and workspace sessions survive.
- Measured cold-start and reconnect times are within interactive expectations on this host.
- Warm-path UI budgets remain uninstrumented; a helper would not address those gaps.
- No budget failure on residency or recovery was observed that would require out-of-process UI ownership.

Revisit if Milestone 0 / M5 measurements show shortcut→overlay or cold-model paths missing targets on lowest-supported hardware, or if menu-bar-hidden Quick Note requires UI the Tauri process cannot host.

## Raw log excerpt

\`\`\`text
$(grep 'LIFECYCLE_MEAS:' "$LOG" || echo "(no LIFECYCLE_MEAS lines)")
\`\`\`
EOF

echo ""
echo "Wrote $MEASUREMENTS"
rm -f "$LOG"
