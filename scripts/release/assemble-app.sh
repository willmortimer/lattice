#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

app_src="$(lattice_release_app_path)"
macos_dir="$app_src/Contents/MacOS"

for dylib in libLatticeVoiceBridge.dylib libLatticeAudioBridge.dylib; do
  src="target/release/$dylib"
  if [ -f "$src" ]; then
    cp -f "$src" "$macos_dir/$dylib"
    echo "assemble-app: bundled $dylib"
  else
    echo "assemble-app: warning: missing $src (voice may fail at runtime)" >&2
  fi
done

for bin in latticed lattice-agentd lattice-wasi-seatbelt lattice-embed-host lattice-voice-host; do
  src="target/release/$bin"
  if [ ! -f "$src" ]; then
    echo "assemble-app: missing $src (required production sidecar)" >&2
    exit 1
  fi
  cp -f "$src" "$macos_dir/$bin"
  chmod +x "$macos_dir/$bin"
  echo "assemble-app: bundled $bin"
done

echo "assemble-app: ok → $app_src"
