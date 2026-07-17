# Shared Dev Container / DevCell environment for scripts in this directory.
# Expects ROOT to be set to the repository root before sourcing.
#
# Bind-mounting the repo from macOS leaves Darwin node_modules and Mach-O
# binaries under target/. Isolate cargo outputs and allow non-TTY pnpm purge.
if [[ -z "${ROOT:-}" ]]; then
  echo "error: ROOT must be set before sourcing _env.sh" >&2
  exit 1
fi

if [[ "${DEVCONTAINER:-}" == "1" || "${DEV_CELL:-}" == "true" ]]; then
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT}/target/devcontainer}"
  # pnpm refuses to replace a foreign modules dir without a TTY unless CI=true.
  export CI="${CI:-true}"
fi

lattice_cli_bin() {
  local profile="${1:-debug}"
  local target_root="${CARGO_TARGET_DIR:-${ROOT}/target}"
  echo "${target_root}/${profile}/lattice"
}
