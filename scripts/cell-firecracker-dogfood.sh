#!/usr/bin/env bash
# Firecracker dogfood: CELLD_BASE_URL → hydrate/run/collect → ≥1 Lattice proposal.
# Default (--dry-run): mocked celld + latticed; no live celld required (CI-safe).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

usage() {
  cat <<'EOF'
usage: scripts/cell-firecracker-dogfood.sh [--dry-run | --live] [live options]

End-to-end Lattice ↔ celld (Firecracker / lattice-runtime) dogfood loop with
propose_resource for collected /output files.

Modes:
  --dry-run (default)  Run mocked integration tests (no celld or latticed).
  --live               Run against live celld + latticed (see env below).

Live options (passed to cell-firecracker-dogfood binary):
  --workspace PATH     Workspace root (default: $CELL_DOGFOOD_WORKSPACE or temp dir)
  --cell-id ID         Cell id (default: cell_dogfood)
  --projection-id ID   Projection id (default: proj_dogfood)
  --output-target DIR  Proposal path prefix (default: Reports)
  --hydrate REL        Workspace-relative file to hydrate under input/ (repeatable)
  --                  Remaining args become guest argv (default: copy input → output)

Live environment (required):
  CELLD_BASE_URL           celld Connect/HTTP origin (no trailing slash)
  LATTICE_API_BASE_URL     latticed HTTP API (e.g. http://127.0.0.1:18787)
  LATTICE_AUTH_TOKEN       Bearer token for propose_resource

Firecracker lab (celld guest media — see cell/scripts/lattice-cell-loop.sh):
  CELL_FC_KERNEL / DEVCELL_FC_KERNEL       Guest kernel (vmlinux)
  CELL_FC_ROOTFS / DEVCELL_FC_ROOTFS       Guest rootfs (cellos.ext4)
  CELL_FC_INITRD / DEVCELL_FC_INITRD       Optional initrd
  CELL_FC_BIN / DEVCELL_FC_BIN             firecracker binary override
  CELL_FC_JAILER_BIN / DEVCELL_FC_JAILER_BIN
  CELL_FC_JAILER_LAUNCH / DEVCELL_FC_JAILER_LAUNCH
  CELL_FC_VSOCK_UDS_ROOT / DEVCELL_FC_VSOCK_UDS_ROOT
  CELL_FC_SLICE_TMPDIR / DEVCELL_FC_SLICE_TMPDIR

Start celld with --backend=firecracker and lattice-runtime profile before --live.
Cell repo: scripts/lattice-cell-loop.sh documents a full apply/start/invoke loop.

Examples:
  scripts/cell-firecracker-dogfood.sh
  scripts/cell-firecracker-dogfood.sh --dry-run
  export CELLD_BASE_URL=http://127.0.0.1:8080
  export LATTICE_API_BASE_URL=http://127.0.0.1:18787 LATTICE_AUTH_TOKEN=...
  scripts/cell-firecracker-dogfood.sh --live --workspace /path/to/ws --hydrate input/hello.txt
EOF
}

mode="dry-run"
live_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      mode="dry-run"
      shift
      ;;
    --live)
      mode="live"
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      if [[ "$mode" != "live" ]]; then
        echo "unknown argument (use --live first): $1" >&2
        usage >&2
        exit 2
      fi
      live_args+=("$1")
      shift
      ;;
  esac
done

if [[ "$mode" == "dry-run" ]]; then
  echo "==> cell firecracker dogfood (mocked celld + propose; no live services)"
  cargo test -p lattice-agentd --test cell_propose
  echo "cell-firecracker-dogfood dry-run ok (>=1 proposal in mocked loop)"
  exit 0
fi

missing=()
[[ -z "${CELLD_BASE_URL:-}" ]] && missing+=("CELLD_BASE_URL")
[[ -z "${LATTICE_API_BASE_URL:-}" ]] && missing+=("LATTICE_API_BASE_URL")
[[ -z "${LATTICE_AUTH_TOKEN:-}" ]] && missing+=("LATTICE_AUTH_TOKEN")
if ((${#missing[@]} > 0)); then
  echo "live mode requires: ${missing[*]}" >&2
  usage >&2
  exit 1
fi

echo "==> cell firecracker dogfood (live celld at $CELLD_BASE_URL)"
exec cargo run -q -p lattice-agentd --bin cell-firecracker-dogfood -- "${live_args[@]}"
