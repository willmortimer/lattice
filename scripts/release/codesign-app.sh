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

# Nix-shell builds link libiconv from the store; rewrite before signing or
# Gatekeeper SIGKILLs the notarized app (posix spawn 163).
bash "$root/scripts/macos/rewrite-nix-libiconv.sh" "$app_src"

# Sidecars/dylibs only need a non-sandboxed hardened-runtime baseline.
sidecar_entitlements="$(mktemp -t lattice-sidecar-ents.XXXXXX.plist)"
cleanup_ents() { rm -f "$sidecar_entitlements"; }
trap cleanup_ents EXIT
cat >"$sidecar_entitlements" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
	<key>com.apple.security.app-sandbox</key>
	<false/>
</dict></plist>
EOF

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
    base="$(basename "$path")"
    if [ "$base" = "lattice-desktop" ]; then
      sign_bin "$path" "$entitlements"
    else
      sign_bin "$path" "$sidecar_entitlements"
    fi
  fi
done
if [ -d "$app_src/Contents/Frameworks" ]; then
  find "$app_src/Contents/Frameworks" -type f \( -perm -111 -o -name '*.dylib' -o -name '*.so' \) -print0 |
    while IFS= read -r -d '' path; do
      sign_bin "$path" "$sidecar_entitlements"
    done
fi
sign_bin "$app_src" "$entitlements"
codesign --verify --deep --strict --verbose=2 "$app_src"
echo "codesign-app: ok"
