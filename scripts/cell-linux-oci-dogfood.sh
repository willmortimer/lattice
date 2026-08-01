#!/usr/bin/env bash
# Linux OCI Lattice dogfood: CELLD_BASE_URL → hydrate/run/collect → ≥1 proposal.
# Places KernelFS role dirs under /run/kernelfs or $XDG_RUNTIME_DIR/kernelfs via
# kernelfs_linux::export_live (no CELL_VZ_RUNTIME_DIR / VirtioFS agent-share).
# Default (--dry-run): mocked celld + latticed; no live celld / runsc (CI-safe).
#
# Live product path is GuestSessionService.Invoke → lattice.runtime.v1 (not cellctl
# exec). Requires celld with native Linux gVisor backend (runsc) and a prepared
# OCI bundle. See docs/dev/celld-client.md § Linux OCI.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

usage() {
  cat <<'EOF'
usage: scripts/cell-linux-oci-dogfood.sh [--dry-run | --live] [options]

Linux OCI beat: same hydrate → run → collect → propose loop as
scripts/cell-firecracker-dogfood.sh, forced to --execution-mode=oci with
KernelFS role volume sources from kernelfs export:

  ${XDG_RUNTIME_DIR:-/run}/kernelfs/{run_id}/{input,output[,work]}

No CELL_VZ_RUNTIME_DIR — Linux export uses kernelfs_linux under the runtime
parent. Contract: Cell docs/28-oci-agent-mount-contract.md (gVisor bind mounts).

Modes:
  --dry-run (default)  Mocked integration tests (no celld, no runsc).
  --live               Live celld + latticed (Linux lab host).

Live options (forwarded; --execution-mode=oci is always set):
  --workspace PATH
  --cell-id ID              (default: cell_dogfood)
  --projection-id ID
  --output-target DIR
  --hydrate REL             (repeatable)
  --oci-bundle-path PATH    required for --live
  --with-work               also export/mount {run_id}/work
  --allow-network           with_network_deny_all(false) when OCI egress is OK
  --shared-cell-id          reuse --cell-id (no {cell_id}_{projection_id} suffix)
  --                        guest argv (default: copy input → output)

Live environment (required):
  CELLD_BASE_URL
  LATTICE_API_BASE_URL
  LATTICE_AUTH_TOKEN

Live celld / runsc (Linux lab — not run by this script):
  # celld with Linux gVisor OCI backend; runsc on PATH
  # Optional: export XDG_RUNTIME_DIR=/run/user/$UID for per-user export parent
  celld --backend=oci --http-dev
  Do NOT set CELL_OCI_AGENT_MOUNT_COPY=1 (hides live-bind).
  Note: cellctl exec live-bind PASS ≠ RunTask; this script exercises Invoke.

Secrets stay opt-in via existing agentd env (LATTICE_WASI_SECRET_HANDLES /
secretHandlesJson); this dogfood does not inject secrets or ambient network.

Examples:
  scripts/cell-linux-oci-dogfood.sh
  scripts/cell-linux-oci-dogfood.sh --dry-run

  # Live hardware (document-only proof in CI; run on Linux lab e.g. optiprox3):
  export CELLD_BASE_URL=http://127.0.0.1:8080
  export LATTICE_API_BASE_URL=http://127.0.0.1:18787 LATTICE_AUTH_TOKEN=…
  scripts/cell-linux-oci-dogfood.sh --live \
    --oci-bundle-path /path/to/oci-bundle \
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
      echo "cell-linux-oci-dogfood always uses --execution-mode=oci (omit $1)" >&2
      exit 2
      ;;
    --vz-runtime-dir | --vz-runtime-dir=*)
      echo "cell-linux-oci-dogfood does not use CELL_VZ_RUNTIME_DIR (omit $1)" >&2
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
  echo "==> cell Linux OCI dogfood (mocked; CI-safe — no celld / runsc)"
  exec "$root/scripts/cell-firecracker-dogfood.sh" --dry-run
fi

missing=()
[[ -z "${CELLD_BASE_URL:-}" ]] && missing+=("CELLD_BASE_URL")
[[ -z "${LATTICE_API_BASE_URL:-}" ]] && missing+=("LATTICE_API_BASE_URL")
[[ -z "${LATTICE_AUTH_TOKEN:-}" ]] && missing+=("LATTICE_AUTH_TOKEN")
if [[ "$has_oci_bundle" -ne 1 ]]; then
  missing+=("--oci-bundle-path")
fi
if ((${#missing[@]} > 0)); then
  echo "live Linux OCI dogfood requires: ${missing[*]}" >&2
  usage >&2
  exit 1
fi

echo "==> cell Linux OCI dogfood (live celld at $CELLD_BASE_URL; kernelfs export under XDG_RUNTIME_DIR or /run/kernelfs)"
exec "$root/scripts/cell-firecracker-dogfood.sh" --live \
  --execution-mode=oci \
  "${live_args[@]}"
