#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root
lattice_release_prefer_xcode

: "${APPLE_ID:?}"
: "${APPLE_PASSWORD:?}"
: "${APPLE_TEAM_ID:?}"

app_src="$(lattice_release_app_path)"
version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app_src/Contents/Info.plist" 2>/dev/null || echo "0.0.0")"
out_dir="${LATTICE_RELEASE_DIR:-target/release/bundle/dmg}"
mkdir -p "$out_dir"
zip_path="$out_dir/Lattice-$version-notarize.zip"

echo "notarize-app: packing → $zip_path"
rm -f "$zip_path"
ditto -c -k --keepParent "$app_src" "$zip_path"

echo "notarize-app: submitting to Apple notary service (can take several minutes)"
if ! xcrun notarytool submit "$zip_path" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait; then
  echo "notarize-app: notarytool submit failed." >&2
  echo "  Check APPLE_ID / APPLE_PASSWORD (app-specific) / APPLE_TEAM_ID." >&2
  exit 1
fi
# Keep zip for staple debugging; build-dmg / staple leave cleanup to verify step.
echo "notarize-app: ok (zip retained at $zip_path)"
