#!/usr/bin/env bash
# Load repo-root .env into the environment, then exec the remaining args.
# Used by desktop tauri:dev* so Pioneer/agent keys reach lattice-desktop even
# when the parent shell's direnv snapshot is stale.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
env_file="${root}/.env"
if [[ -f "${env_file}" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "${env_file}"
  set +a
fi
exec "$@"
