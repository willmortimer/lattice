#!/usr/bin/env bash
# macOS-only: synthesize a spoken English WAV fixture via `say` + afconvert.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$ROOT/Fixtures"
mkdir -p "$FIXTURES"

TEXT="${1:-Lattice voice dictation should preserve CamelCase identifiers like AsrManager, file paths such as /Users/will/Developer/lattice, and punctuation around code.}"
AIFF="$FIXTURES/technical-dictation.aiff"
WAV="$FIXTURES/technical-dictation-16k-mono.wav"

echo "Synthesizing speech with say…"
say -o "$AIFF" "$TEXT"

echo "Converting to 16 kHz mono WAV (Linear PCM)…"
afconvert \
  -f WAVE \
  -d LEF32@16000 \
  -c 1 \
  "$AIFF" \
  "$WAV"

rm -f "$AIFF"
echo "Wrote $WAV"
ls -lh "$WAV"
