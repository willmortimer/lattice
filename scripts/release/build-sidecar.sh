#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

pkg="${1:?package name required}"
bin="${2:?bin name required}"
features="${3:-}"

echo "build-sidecar: cargo build --release -p $pkg --bin $bin ${features:+--features $features}"
if [ -n "$features" ]; then
  cargo build --release -p "$pkg" --bin "$bin" --features "$features" || {
    # voice-host may fall back without fluidaudio
    if [ "$pkg" = "lattice-voice-host" ]; then
      cargo build --release -p "$pkg" --bin "$bin"
    else
      exit 1
    fi
  }
else
  cargo build --release -p "$pkg" --bin "$bin"
fi
test -f "target/release/$bin"
echo "build-sidecar: ok → target/release/$bin"
