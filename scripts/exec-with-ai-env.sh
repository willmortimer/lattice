#!/usr/bin/env bash
# Load ecosystem secrets/ai.env when agent provider keys are missing, then exec.
#
# Prefer launching desktop via this wrapper (nxr desktop-dev) so Tauri sees
# PIONEER_API_KEY / OPENAI_API_KEY without a manual `with-secrets` prefix.
# NXR contexts still use provider=env — they only forward keys already present.
#
# Private demo overlays, First Look reset knobs, and other ecosystem-only
# runtime config belong in lattice-ecosystem `scripts/exec-for-dev.sh` — not here.
set -euo pipefail

lattice_root="$(cd "$(dirname "$0")/.." && pwd)"

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <command>..." >&2
  exit 2
fi

if [[ -n "${OPENAI_API_KEY:-}" || -n "${PIONEER_API_KEY:-}" ]]; then
  exec "$@"
fi

eco="${LATTICE_ECOSYSTEM_ROOT:-}"
if [[ -z "$eco" && -f "$lattice_root/../secrets/ai.env" ]]; then
  eco="$(cd "$lattice_root/.." && pwd)"
fi

if [[ -z "$eco" || ! -f "$eco/secrets/ai.env" ]]; then
  echo "exec-with-ai-env: no provider keys and no secrets/ai.env — agent will use fake" >&2
  exec "$@"
fi

if [[ ! -x "$eco/scripts/with-secrets.sh" ]]; then
  echo "exec-with-ai-env: missing $eco/scripts/with-secrets.sh — agent will use fake" >&2
  exec "$@"
fi

if ! command -v sops >/dev/null 2>&1; then
  echo "exec-with-ai-env: sops not on PATH — cannot decrypt ai.env; agent will use fake" >&2
  exec "$@"
fi

echo "exec-with-ai-env: injecting secrets/ai.env for desktop/agent" >&2

# Decrypt into this shell, set defaults, then exec the real command.
# shellcheck disable=SC1090
eval "$(sops -d "$eco/secrets/ai.env" | python3 "$eco/scripts/sops-dotenv-exports.py")"

if [[ -z "${LATTICE_AGENT_PROVIDER:-}" ]]; then
  if [[ -n "${OPENAI_API_KEY:-}" ]]; then
    export LATTICE_AGENT_PROVIDER=openai
  elif [[ -n "${PIONEER_API_KEY:-}" ]]; then
    export LATTICE_AGENT_PROVIDER=pioneer
  fi
fi

if [[ -z "${LATTICE_AGENT_MODEL:-}" ]]; then
  case "${LATTICE_AGENT_PROVIDER:-}" in
    openai) export LATTICE_AGENT_MODEL=gpt-5-nano ;;
    pioneer) export LATTICE_AGENT_MODEL=gpt-5.6-luna ;;
  esac
fi

exec "$@"
