#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

app_src="$(lattice_release_app_path)"
macos_dir="$app_src/Contents/MacOS"
plugins_dir="$app_src/Contents/PlugIns"
root="$(cd "$(dirname "$0")/../.." && pwd)"

for dylib in libLatticeVoiceBridge.dylib libLatticeAudioBridge.dylib libLatticeApprovalBridge.dylib; do
  src="target/release/$dylib"
  if [ -f "$src" ]; then
    cp -f "$src" "$macos_dir/$dylib"
    echo "assemble-app: bundled $dylib"
  else
    echo "assemble-app: warning: missing $src" >&2
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

# Quick Look appex (optional if Xcode/SDK unavailable).
appex_out="$root/target/macos/LatticeQuickLook.appex"
if bash "$root/scripts/macos/build-quicklook-appex.sh" "$appex_out"; then
  mkdir -p "$plugins_dir"
  rm -rf "$plugins_dir/LatticeQuickLook.appex"
  cp -R "$appex_out" "$plugins_dir/LatticeQuickLook.appex"
  echo "assemble-app: bundled LatticeQuickLook.appex"
else
  echo "assemble-app: warning: Quick Look appex build skipped/failed" >&2
fi

echo "assemble-app: ok → $app_src"
