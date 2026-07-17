#!/usr/bin/env bash
# Install workspace JS deps after the image is created.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=../../scripts/devcontainer/_env.sh
source "${ROOT}/scripts/devcontainer/_env.sh"
cd "${ROOT}"

corepack prepare pnpm@11.11.0 --activate
pnpm install

echo "Lattice post-create done. Start demos with:"
echo "  ./scripts/devcontainer/up     # seed + bridge"
echo "  ./scripts/devcontainer/web    # Vite against bridge"
echo "  ./scripts/devcontainer/site"
echo "  ./scripts/devcontainer/test"
