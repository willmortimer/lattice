#!/usr/bin/env bash
# Align llama-cpp-sys cmake with the Nix apple-sdk that rustc links against.
#
# Without this, cmake often discovers Xcode's MacOSX.sdk (host 26.x), compiles
# Metal residency APIs (macOS 15+ / MTLResidencySetDescriptor), then rustc links
# with apple-sdk-14.4 + MACOSX_DEPLOYMENT_TARGET=14.0 and fails undefined.
#
# shellcheck shell=bash
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
  return 0 2>/dev/null || exit 0
fi

if [ -z "${SDKROOT:-}" ]; then
  echo "llama-cpp-nix-sdk: SDKROOT unset; leave cmake defaults alone" >&2
  return 0 2>/dev/null || exit 0
fi

: "${MACOSX_DEPLOYMENT_TARGET:=14.0}"
export MACOSX_DEPLOYMENT_TARGET
export CMAKE_OSX_SYSROOT="$SDKROOT"
export CMAKE_OSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET"

# cmake-rs does not always forward env into an existing cache; scrub stale
# llama-cpp-sys builds that targeted a different SDK / deployment min.
_lattice_llama_sysroot_mismatch() {
  local cache
  for cache in target/*/build/llama-cpp-sys-2-*/out/build/CMakeCache.txt; do
    [ -f "$cache" ] || continue
    if grep -Eq 'mmacosx-version-min=2[5-9]\.|/Applications/Xcode\.app/.*MacOSX\.sdk' "$cache"; then
      return 0
    fi
    if grep -q "CMAKE_OSX_SYSROOT:STRING=$SDKROOT" "$cache"; then
      continue
    fi
    # Empty or non-Nix sysroot while we require Nix SDKROOT.
    if grep -Eq '^CMAKE_OSX_SYSROOT:STRING=$' "$cache" \
      || ! grep -Fq "CMAKE_OSX_SYSROOT:STRING=$SDKROOT" "$cache"; then
      return 0
    fi
  done
  return 1
}

if _lattice_llama_sysroot_mismatch; then
  echo "llama-cpp-nix-sdk: clearing stale llama-cpp-sys-2 build dirs (SDK/deployment mismatch)" >&2
  # shellcheck disable=SC2086
  rm -rf target/*/build/llama-cpp-sys-2-* \
    target/*/deps/libllama_cpp_sys_2-* \
    target/*/deps/llama_cpp_sys_2-*
fi

echo "llama-cpp-nix-sdk: CMAKE_OSX_SYSROOT=$CMAKE_OSX_SYSROOT" >&2
echo "llama-cpp-nix-sdk: CMAKE_OSX_DEPLOYMENT_TARGET=$CMAKE_OSX_DEPLOYMENT_TARGET" >&2
