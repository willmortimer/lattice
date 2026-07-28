#!/usr/bin/env bash
# Skip pnpm install when lockfile/workspace inputs and pnpm version are unchanged.
#
# Stamp: node_modules/.lattice-js-deps.stamp
#   --dev        prefer-offline only (hot desktop launches)
#   --bootstrap  always install + refresh stamp (nxr task js-deps)
set -euo pipefail

lattice_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$lattice_root"

STAMP_FILE="node_modules/.lattice-js-deps.stamp"
DEV_MODE=0
BOOTSTRAP=0
PNPM_EXTRA=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dev)
      DEV_MODE=1
      shift
      ;;
    --bootstrap)
      BOOTSTRAP=1
      shift
      ;;
    --)
      shift
      PNPM_EXTRA+=("$@")
      break
      ;;
    *)
      PNPM_EXTRA+=("$1")
      shift
      ;;
  esac
done

compute_stamp_key() {
  local hasher
  if command -v sha256sum >/dev/null 2>&1; then
    hasher=(sha256sum)
  elif command -v shasum >/dev/null 2>&1; then
    hasher=(shasum -a 256)
  else
    echo "ensure-js-deps: need sha256sum or shasum on PATH" >&2
    exit 1
  fi

  {
    pnpm --version
    for f in pnpm-lock.yaml package.json pnpm-workspace.yaml apps/desktop/package.json; do
      if [[ -f "$f" ]]; then
        printf '%s\0' "$f"
        cat "$f"
      fi
    done
    shopt -s nullglob
    local pkg
    for pkg in packages/*/package.json; do
      printf '%s\0' "$pkg"
      cat "$pkg"
    done
    shopt -u nullglob
  } | "${hasher[@]}" | awk '{print $1}'
}

run_install() {
  if [[ "$DEV_MODE" -eq 1 ]]; then
    pnpm install --prefer-offline "${PNPM_EXTRA[@]}"
  else
    pnpm install --frozen-lockfile --prefer-offline "${PNPM_EXTRA[@]}"
  fi
}

write_stamp() {
  mkdir -p node_modules
  compute_stamp_key >"$STAMP_FILE"
}

key="$(compute_stamp_key)"

if [[ "$BOOTSTRAP" -eq 0 && -d node_modules && -f "$STAMP_FILE" ]]; then
  if [[ "$(cat "$STAMP_FILE")" == "$key" ]]; then
    exit 0
  fi
fi

run_install
write_stamp
