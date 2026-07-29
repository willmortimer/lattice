#!/usr/bin/env bash
# Rewrite absolute Nix store libiconv install names to the macOS system libiconv.
#
# Building inside the Lattice Nix shell links Darwin binaries against
# $NIX_STORE/.../libiconv.2.dylib. Hardened-runtime Developer ID apps then
# SIGKILL on launch (Gatekeeper / AMFI cannot load Nix store paths).
#
# Usage (from lattice repo root):
#   bash scripts/macos/rewrite-nix-libiconv.sh target/release/bundle/macos/Lattice.app
#   bash scripts/macos/rewrite-nix-libiconv.sh target/release/latticed
# shellcheck shell=bash
set -euo pipefail

SYSTEM_LIBICONV="/usr/lib/libiconv.2.dylib"

rewrite_one() {
  local path="$1"
  local deps nix_iconv
  deps="$(otool -L "$path" 2>/dev/null || true)"
  nix_iconv="$(printf '%s\n' "$deps" | awk '/\/nix\/store\/.*libiconv\.2\.dylib/{print $1; exit}')"
  if [ -z "$nix_iconv" ]; then
    return 0
  fi
  echo "rewrite-nix-libiconv: $(basename "$path"): $nix_iconv → $SYSTEM_LIBICONV" >&2
  install_name_tool -change "$nix_iconv" "$SYSTEM_LIBICONV" "$path"
}

if [ $# -lt 1 ]; then
  echo "usage: $0 <Mach-O|Lattice.app>..." >&2
  exit 2
fi

for target in "$@"; do
  if [ -d "$target" ] && [ -d "$target/Contents/MacOS" ]; then
    find "$target/Contents/MacOS" -type f -print0 |
      while IFS= read -r -d '' f; do
        if file "$f" | grep -q 'Mach-O'; then
          rewrite_one "$f"
        fi
      done
  elif [ -f "$target" ]; then
    rewrite_one "$target"
  else
    echo "rewrite-nix-libiconv: skip missing path: $target" >&2
  fi
done
