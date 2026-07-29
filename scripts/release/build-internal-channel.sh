#!/usr/bin/env bash
# Build the internal channel Lattice.app (side-by-side bundle id).
# Shares Developer ID context with desktop-release; does not require live notarize
# when LATTICE_RELEASE_VALIDATE_ONLY=1.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

if [ -f ./lattice/Cargo.toml ] && [ -d ./lattice/apps/daemon ]; then
  cd ./lattice
fi

export LATTICE_CHANNEL="${LATTICE_CHANNEL:-internal}"
export LATTICE_CLOUD_URL="${LATTICE_CLOUD_URL:-https://staging.cloud.lattice-notes.com}"
# Bake Finder-friendly default into lattice-cloud-client (see build.rs).
export LATTICE_CLOUD_URL_DEFAULT="${LATTICE_CLOUD_URL_DEFAULT:-$LATTICE_CLOUD_URL}"
export LATTICE_PRODUCT_NAME="${LATTICE_PRODUCT_NAME:-Lattice Dev}"
export LATTICE_BUNDLE_ID="${LATTICE_BUNDLE_ID:-dev.lattice.desktop.dev}"

echo "desktop-release-internal: channel=${LATTICE_CHANNEL} cloud=${LATTICE_CLOUD_URL} id=${LATTICE_BUNDLE_ID}" >&2
echo "Apple console: add App ID ${LATTICE_BUNDLE_ID} to the same SIWA group as dev.lattice.desktop" >&2

./scripts/ensure-js-deps.sh
pnpm --filter @lattice/desktop exec tauri build \
  --config src-tauri/tauri.internal.conf.json \
  --bundles app \
  --features voice-embedded \
  "$@"
