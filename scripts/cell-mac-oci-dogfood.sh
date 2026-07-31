#!/usr/bin/env bash
# Mac OCI Lattice dogfood: CELLD_BASE_URL → hydrate/run/collect → ≥1 proposal.
# Places KernelFS role dirs under ivisor agent-share (Cell mac-live-bind contract).
# Default (--dry-run): mocked celld + latticed; no live celld / hardware (CI-safe).
#
# Live product path is GuestSessionService.Invoke → lattice.runtime.v1 (not cellctl
# exec). Requires staged CellOS *lattice* artifacts (cell-agent) under
# CELL_VZ_IMAGES_DIR; busybox OCI bundles are only the container rootfs.
# Prefer CELL_OCI_IVISOR_SYNC=guest when the image has tar/gzip; use orbctl if
# StartCell fails on guest-channel sync until the image is restaged.
# See docs/dev/celld-client.md § Lattice uses Cells on a Mac.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

usage() {
  cat <<'EOF'
usage: scripts/cell-mac-oci-dogfood.sh [--dry-run | --live] [options]

First “Lattice uses Cells on a Mac” beat: same hydrate → run → collect →
propose loop as scripts/cell-firecracker-dogfood.sh, forced to
--execution-mode=oci with KernelFS role volume sources from kernelfs export:

  ${CELL_VZ_RUNTIME_DIR}/ivisor-worker-<cell-id>/agent-share/{run_id}/{input,output[,work]}

Materialize lands under agent-share/.kernelfs-runs/{run_id}/; export symlinks
role dirs at agent-share/{run_id}/… (run_id defaults to --projection-id).
Contract: Cell docs/mac-live-bind-demo.md (agent-share + VirtioFS live-bind).

Modes:
  --dry-run (default)  Mocked integration tests (no celld, no Apple Silicon).
  --live               Live celld --backend=vz + latticed (hardware lab).

Live options (forwarded; --execution-mode=oci is always set):
  --workspace PATH
  --cell-id ID              (default: cell_dogfood)
  --projection-id ID
  --output-target DIR
  --hydrate REL             (repeatable)
  --oci-bundle-path PATH    required for --live
  --vz-runtime-dir PATH     or set CELL_VZ_RUNTIME_DIR / CELL_OCI_IVISOR_WORKSPACE
  --with-work               also export/mount agent-share/{run_id}/work
  --allow-network           with_network_deny_all(false) when OCI egress is OK
  --                        guest argv (default: copy input → output)

Live environment (required):
  CELLD_BASE_URL
  LATTICE_API_BASE_URL
  LATTICE_AUTH_TOKEN

Live celld / helper (Apple Silicon lab — not run by this script):
  CELL_OCI_IVISOR_INTERIM=1
  CELL_OCI_IVISOR_WORKSPACE=<parent of OCI bundle>
  CELL_VZ_RUNTIME_DIR=<same runtime as cell-host-macos>   # agent-share parent
  CELL_VZ_HELPER_SOCKET / CELL_VZ_IMAGES_DIR as needed
  # CELL_VZ_IMAGES_DIR must stage lattice profile-manifest (lattice.runtime.v1)
  # CELL_OCI_IVISOR_SYNC=guest|orbctl  # guest needs tar/gzip; orbctl = OrbStack fallback
  celld --backend=vz --http-dev
  Do NOT set CELL_OCI_AGENT_MOUNT_COPY=1 (hides live-bind).
  Note: cellctl exec live-bind PASS ≠ RunTask; this script exercises Invoke.

Secrets stay opt-in via existing agentd env (LATTICE_WASI_SECRET_HANDLES /
secretHandlesJson); this dogfood does not inject secrets or ambient network.

Examples:
  scripts/cell-mac-oci-dogfood.sh
  scripts/cell-mac-oci-dogfood.sh --dry-run

  # Live hardware (document-only proof in CI; run on Apple Silicon lab):
  export CELLD_BASE_URL=http://127.0.0.1:8080
  export LATTICE_API_BASE_URL=http://127.0.0.1:18787 LATTICE_AUTH_TOKEN=…
  export CELL_OCI_IVISOR_INTERIM=1
  export CELL_OCI_IVISOR_WORKSPACE=/tmp/cell-oci-bundles
  export CELL_VZ_RUNTIME_DIR=/tmp/cell-oci-bundles/vz-runtime
  scripts/cell-mac-oci-dogfood.sh --live \
    --oci-bundle-path /tmp/cell-oci-bundles/cell_mac_live_bind \
    --workspace /path/to/ws --hydrate input/hello.txt
EOF
}

mode="dry-run"
live_args=()
has_oci_bundle=0

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
    --execution-mode | --execution-mode=*)
      echo "cell-mac-oci-dogfood always uses --execution-mode=oci (omit $1)" >&2
      exit 2
      ;;
    --oci-bundle-path)
      if [[ "$mode" != "live" ]]; then
        echo "unknown argument (use --live first): $1" >&2
        usage >&2
        exit 2
      fi
      shift
      live_args+=("--oci-bundle-path" "${1:-}")
      has_oci_bundle=1
      shift
      ;;
    --oci-bundle-path=*)
      if [[ "$mode" != "live" ]]; then
        echo "unknown argument (use --live first): $1" >&2
        usage >&2
        exit 2
      fi
      live_args+=("--oci-bundle-path" "${1#*=}")
      has_oci_bundle=1
      shift
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
  echo "==> cell Mac OCI dogfood (mocked; CI-safe — no celld / VZ hardware)"
  exec "$root/scripts/cell-firecracker-dogfood.sh" --dry-run
fi

missing=()
[[ -z "${CELLD_BASE_URL:-}" ]] && missing+=("CELLD_BASE_URL")
[[ -z "${LATTICE_API_BASE_URL:-}" ]] && missing+=("LATTICE_API_BASE_URL")
[[ -z "${LATTICE_AUTH_TOKEN:-}" ]] && missing+=("LATTICE_AUTH_TOKEN")
if [[ "$has_oci_bundle" -ne 1 ]]; then
  missing+=("--oci-bundle-path")
fi
if [[ -z "${CELL_VZ_RUNTIME_DIR:-}" && -z "${CELL_OCI_IVISOR_WORKSPACE:-}" ]]; then
  # binary also accepts --vz-runtime-dir in live_args; check that loosely
  runtime_flag=0
  for ((i = 0; i < ${#live_args[@]}; i++)); do
    if [[ "${live_args[$i]}" == "--vz-runtime-dir" ]]; then
      runtime_flag=1
      break
    fi
  done
  if [[ "$runtime_flag" -ne 1 ]]; then
    missing+=("CELL_VZ_RUNTIME_DIR|CELL_OCI_IVISOR_WORKSPACE|--vz-runtime-dir")
  fi
fi
if ((${#missing[@]} > 0)); then
  echo "live Mac OCI dogfood requires: ${missing[*]}" >&2
  usage >&2
  exit 1
fi

echo "==> cell Mac OCI dogfood (live celld at $CELLD_BASE_URL; agent-share under CELL_VZ_RUNTIME_DIR)"
exec "$root/scripts/cell-firecracker-dogfood.sh" --live \
  --execution-mode=oci \
  "${live_args[@]}"
