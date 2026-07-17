#!/usr/bin/env bash
# Install workspace JS deps after the image is created.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT}"

corepack prepare pnpm@11.11.0 --activate
pnpm install

echo "Lattice post-create done. Start demos with:"
echo "  ./scripts/devcontainer/web"
echo "  ./scripts/devcontainer/site"
echo "  ./scripts/devcontainer/test"
