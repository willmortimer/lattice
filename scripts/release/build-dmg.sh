#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

app_src="$(lattice_release_app_path)"
version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app_src/Contents/Info.plist" 2>/dev/null || echo "0.0.0")"
out_dir="${LATTICE_RELEASE_DIR:-target/release/bundle/dmg}"
mkdir -p "$out_dir"
dmg_path="$out_dir/Lattice-$version.dmg"
zip_path="$out_dir/Lattice-$version-notarize.zip"

echo "build-dmg: → $dmg_path"
rm -f "$dmg_path"
hdiutil create \
  -volname "Lattice" \
  -srcfolder "$app_src" \
  -ov \
  -format UDZO \
  "$dmg_path"
rm -f "$zip_path"
echo "build-dmg: ok → $dmg_path"
