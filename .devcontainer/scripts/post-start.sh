#!/usr/bin/env bash
# Print cell demo pointers. Do not auto-start servers or Tauri.
set -euo pipefail

cat <<'EOF'
Lattice Dev Container ready.

Cell demo (HTTP, Tailscale Serve–friendly):
  ./scripts/devcontainer/web     # browser UI on 0.0.0.0:5173
  ./scripts/devcontainer/site    # docs/marketing on 0.0.0.0:4321
  ./scripts/devcontainer/test    # cargo + desktop vitest + CLI smoke

Headless CLI + real core in the browser:
  ./scripts/devcontainer/cli     # build lattice → target/debug/lattice
  ./scripts/devcontainer/seed    # LATTICE_DEV_HOME=target/cell-home + First Look
  ./scripts/devcontainer/bridge  # lattice-bridge on 0.0.0.0:8787 (published)
  ./scripts/devcontainer/up      # seed + bridge (background) + web instructions

Real Lattice UI against Rust handlers (two terminals):
  ./scripts/devcontainer/seed && ./scripts/devcontainer/bridge
  ./scripts/devcontainer/web     # sets VITE_LATTICE_BRIDGE_URL → bridge mode

Demo fixture only (no bridge): unset VITE_LATTICE_BRIDGE_URL before ./scripts/devcontainer/web

Native Tauri (`desktop-dev`) is out of scope here — use Nix on your Mac.
See docs/dev/devcontainer.md.
EOF
