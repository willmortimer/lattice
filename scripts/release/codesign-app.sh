#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root
lattice_release_prefer_xcode

: "${APPLE_SIGNING_IDENTITY:?Set APPLE_SIGNING_IDENTITY}"
app_src="$(lattice_release_app_path)"
macos_dir="$app_src/Contents/MacOS"

echo "codesign-app: Developer ID, hardened runtime → $app_src"
sign_bin() {
  local path="$1"
  if ! codesign --force --options runtime --timestamp \
    --sign "$APPLE_SIGNING_IDENTITY" "$path"; then
    echo "codesign-app: codesign failed: $path" >&2
    echo "  identity: $APPLE_SIGNING_IDENTITY" >&2
    exit 1
  fi
}

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
