#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

app_src="$(lattice_release_app_path)"
version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app_src/Contents/Info.plist" 2>/dev/null || echo "0.0.0")"
out_dir="${LATTICE_RELEASE_DIR:-target/release/bundle/dmg}"
dmg_path="$out_dir/Lattice-$version.dmg"

echo "verify-gatekeeper: spctl + codesign on $app_src"
spctl -a -vv --type execute "$app_src"
codesign --verify --deep --strict --verbose=2 "$app_src"
if [ -f "$dmg_path" ]; then
  echo "verify-gatekeeper: dmg present → $dmg_path"
else
  echo "verify-gatekeeper: warning: missing $dmg_path" >&2
fi
echo "verify-gatekeeper: ok"
echo "  app: $app_src"
echo "  dmg: $dmg_path"
