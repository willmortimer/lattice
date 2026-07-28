#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root
lattice_release_prefer_xcode

app_src="$(lattice_release_app_path)"
echo "staple-app: stapling ticket onto $app_src"
if ! xcrun stapler staple "$app_src"; then
  echo "staple-app: stapler failed for $app_src" >&2
  exit 1
fi
xcrun stapler validate "$app_src"
echo "staple-app: ok"
