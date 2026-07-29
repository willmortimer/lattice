#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

pkg="${1:?package name required}"
bin="${2:?bin name required}"
features="${3:-}"

# llama-cpp Metal must compile against the same Nix apple-sdk rustc links.
if [ "$features" = "llama-cpp" ] || [[ "$features" == *llama-cpp* ]]; then
  # shellcheck source=scripts/macos/llama-cpp-nix-sdk.sh
  source "$(cd "$(dirname "$0")/.." && pwd)/macos/llama-cpp-nix-sdk.sh"
fi

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
