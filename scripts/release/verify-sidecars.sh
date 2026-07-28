#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

for bin in latticed lattice-agentd lattice-embed-host lattice-voice-host; do
  if [ ! -f "target/release/$bin" ]; then
    echo "verify-sidecars: missing target/release/$bin after build" >&2
    exit 1
  fi
done
backends="$(target/release/lattice-embed-host backends || true)"
echo "verify-sidecars: lattice-embed-host backends:"$'\n'"$backends"
if ! printf '%s\n' "$backends" | grep -qx 'llama-cpp'; then
  echo "verify-sidecars: lattice-embed-host must list llama-cpp (build with --features llama-cpp)" >&2
  exit 1
fi
echo "verify-sidecars: ok"
