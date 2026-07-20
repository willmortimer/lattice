#!/usr/bin/env bash
# Invoke research/voice-m0-fluidaudio when Swift + FluidAudio models are available.
# Exits non-zero with a clear message when deps/models/fixtures are missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
M0_DIR="$REPO_ROOT/research/voice-m0-fluidaudio"
MODE="${1:-unified}"

usage() {
  cat <<'EOF'
Usage: run_fluidaudio_provider.sh <unified|eou-tdt>

Runs the M0 FluidAudio spike in release mode and prints machine-readable
STREAMING_TEXT / OFFLINE_TEXT lines for the voice-eval harness to parse.

Requires:
  - Apple Silicon macOS + Xcode Swift toolchain
  - Generated fixture: research/voice-m0-fluidaudio/Fixtures/technical-dictation-16k-mono.wav
  - Network on first run (HuggingFace model download into .cache/)

This script is optional. CI should use: python3 scripts/voice_eval.py --dry-run
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "ERROR: FluidAudio provider runner requires macOS (found $(uname -s))." >&2
  echo "HINT: use --dry-run or score --hypothesis-file for offline metric checks." >&2
  exit 3
fi

if [[ ! -d "$M0_DIR" ]]; then
  echo "ERROR: voice-m0-fluidaudio package missing at $M0_DIR" >&2
  exit 3
fi

FIXTURE="$M0_DIR/Fixtures/technical-dictation-16k-mono.wav"
if [[ ! -f "$FIXTURE" ]]; then
  echo "ERROR: fixture WAV missing: $FIXTURE" >&2
  echo "HINT: cd research/voice-m0-fluidaudio && ./scripts/generate-fixture.sh" >&2
  exit 2
fi

if ! command -v /usr/bin/swift >/dev/null 2>&1; then
  echo "ERROR: /usr/bin/swift not found; install Xcode command-line tools." >&2
  exit 3
fi

run_swift() {
  env -i \
    HOME="$HOME" USER="$USER" TMPDIR="${TMPDIR:-/tmp}" \
    PATH="/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin:/Applications/Xcode.app/Contents/Developer/usr/bin:/usr/bin:/bin" \
    DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer" \
    SDKROOT="/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk" \
    /usr/bin/swift "$@"
}

cd "$M0_DIR"
echo "INFO: building voice-m0-fluidaudio (mode=${MODE})..." >&2
if ! run_swift build -c release; then
  echo "ERROR: Swift build failed (SDK mismatch or missing FluidAudio deps)." >&2
  echo "HINT: see research/voice-m0-fluidaudio/README.md for clean-env swift." >&2
  exit 3
fi

echo "INFO: running voice-m0-fluidaudio --mode ${MODE}..." >&2
run_swift run -c release voice-m0-fluidaudio --mode "$MODE"
