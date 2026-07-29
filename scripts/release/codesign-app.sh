#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root
lattice_release_prefer_xcode

: "${APPLE_SIGNING_IDENTITY:?Set APPLE_SIGNING_IDENTITY}"
app_src="$(lattice_release_app_path)"
macos_dir="$app_src/Contents/MacOS"
plugins_dir="$app_src/Contents/PlugIns"
root="$(cd "$(dirname "$0")/../.." && pwd)"
entitlements="$root/apps/desktop/src-tauri/Entitlements.plist"
ql_entitlements="$root/apps/desktop/macos/LatticeQuickLook/LatticeQuickLook.entitlements"
if [ ! -f "$entitlements" ]; then
  echo "codesign-app: missing entitlements: $entitlements" >&2
  exit 1
fi

echo "codesign-app: Developer ID, hardened runtime + entitlements → $app_src"
echo "codesign-app: entitlements=$entitlements"

sign_bin() {
  local path="$1"
  local ents="${2:-$entitlements}"
  if ! codesign --force --options runtime --timestamp \
    --entitlements "$ents" \
    --sign "$APPLE_SIGNING_IDENTITY" "$path"; then
    echo "codesign-app: codesign failed: $path" >&2
    echo "  identity: $APPLE_SIGNING_IDENTITY" >&2
    exit 1
  fi
}

# Nested PlugIns first (Quick Look appex uses its own entitlements).
if [ -d "$plugins_dir" ]; then
  find "$plugins_dir" -name '*.appex' -print0 |
    while IFS= read -r -d '' appex; do
      exe="$appex/Contents/MacOS"
      if [ -d "$exe" ]; then
        find "$exe" -type f -print0 |
          while IFS= read -r -d '' bin; do
            sign_bin "$bin" "$ql_entitlements"
          done
      fi
      sign_bin "$appex" "$ql_entitlements"
      echo "codesign-app: signed appex $(basename "$appex")"
    done
fi

for path in "$macos_dir"/*; do
  if [ -f "$path" ] || [ -L "$path" ]; then
    sign_bin "$path"
  fi
done
if [ -d "$app_src/Contents/Frameworks" ]; then
  find "$app_src/Contents/Frameworks" -type f \( -perm -111 -o -name '*.dylib' -o -name '*.so' \) -print0 |
    while IFS= read -r -d '' path; do
      sign_bin "$path"
    done
fi
sign_bin "$app_src"
codesign --verify --deep --strict --verbose=2 "$app_src"
echo "codesign-app: ok"
