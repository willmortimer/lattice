#!/usr/bin/env bash
# Wrapper for desktop sidecar launches via LATTICE_AGENTD_BIN.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$APP_ROOT/../.." && pwd)"
cd "$REPO_ROOT"
exec pnpm --filter @lattice/agentd exec tsx "$APP_ROOT/src/index.ts" "$@"
