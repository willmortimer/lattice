#!/usr/bin/env bash
# Shared helpers for Lattice macOS release leaf scripts.
# shellcheck shell=bash
set -euo pipefail

lattice_release_ensure_root() {
  if [ "$(uname -s)" != "Darwin" ]; then
    echo "desktop-release: macOS only" >&2
    exit 1
  fi
  if [ -f ./lattice/Cargo.toml ] && [ -d ./lattice/apps/daemon ]; then
    cd ./lattice
  elif [ ! -f ./Cargo.toml ] || [ ! -d ./apps/daemon ]; then
    echo "desktop-release: run from lattice repo root (or ecosystem root with ./lattice)" >&2
    exit 1
  fi
}

lattice_release_prefer_xcode() {
  if [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
  elif [ -d /Library/Developer/CommandLineTools ]; then
    export DEVELOPER_DIR=/Library/Developer/CommandLineTools
  fi
}

lattice_release_app_path() {
  local app_src="target/release/bundle/macos/Lattice.app"
  if [ ! -d "$app_src" ]; then
    local alt_src="apps/desktop/src-tauri/target/release/bundle/macos/Lattice.app"
    if [ -d "$alt_src" ]; then
      app_src="$alt_src"
    else
      echo "desktop-release: missing bundle at target/release/bundle/macos/Lattice.app" >&2
      exit 1
    fi
  fi
  printf '%s' "$app_src"
}
