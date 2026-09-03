#!/usr/bin/env bash
set -euo pipefail
# One Cargo invocation for all macOS release sidecars so shared crates
# (Wasmtime, Arrow, Tokio, …) compile once. Individual build-sidecar.sh
# remains for rebuilding a single binary.
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

# llama-cpp Metal must compile against the same Nix apple-sdk rustc links.
# shellcheck source=scripts/macos/llama-cpp-nix-sdk.sh
source "$(cd "$(dirname "$0")/.." && pwd)/macos/llama-cpp-nix-sdk.sh"

features="lattice-embed-host/llama-cpp,lattice-voice-host/fluidaudio"

echo "build-sidecars: cargo build --release (cohort) --features $features"
if ! cargo build --release \
  -p lattice-daemon --bin latticed \
  -p lattice-agentd --bin lattice-agentd --bin lattice-wasi-seatbelt \
  -p lattice-embed-host --bin lattice-embed-host \
  -p lattice-voice-host --bin lattice-voice-host \
  --features "$features"; then
  echo "build-sidecars: fluidaudio failed; retrying voice-host without it" >&2
  cargo build --release \
    -p lattice-daemon --bin latticed \
    -p lattice-agentd --bin lattice-agentd --bin lattice-wasi-seatbelt \
    -p lattice-embed-host --bin lattice-embed-host \
    -p lattice-voice-host --bin lattice-voice-host \
    --features lattice-embed-host/llama-cpp
fi

for bin in latticed lattice-agentd lattice-wasi-seatbelt lattice-embed-host lattice-voice-host; do
  test -f "target/release/$bin"
  echo "build-sidecars: ok → target/release/$bin"
done
