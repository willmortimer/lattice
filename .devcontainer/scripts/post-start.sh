#!/usr/bin/env bash
# Print cell demo pointers. Do not auto-start servers or Tauri.
set -euo pipefail

cat <<'EOF'
Lattice Dev Container ready.

Cell demo (HTTP, Tailscale Serve–friendly):
  ./scripts/devcontainer/web   # browser UI on 0.0.0.0:5173
  ./scripts/devcontainer/site  # docs/marketing on 0.0.0.0:4321
  ./scripts/devcontainer/test  # cargo + desktop vitest + CLI smoke

Headless CLI (real workspace on disk):
  ./scripts/devcontainer/cli   # build lattice → target/debug/lattice
  ./scripts/devcontainer/seed  # LATTICE_DEV_HOME=target/cell-home + demo workspace

Native Tauri (`desktop-dev`) is out of scope here — use Nix on your Mac.
See docs/dev/devcontainer.md.
EOF
