#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

pnpm install
# Keep Nix apple-sdk DEVELOPER_DIR/SDKROOT for the Cargo/Tauri build.
pnpm --filter @lattice/desktop exec tauri build --bundles app --features voice-embedded
echo "desktop-tauri-bundle: ok → $(lattice_release_app_path)"
