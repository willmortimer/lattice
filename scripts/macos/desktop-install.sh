#!/usr/bin/env bash
# Build, sign, and install Lattice.app for local dogfood or a local distribution-shaped install.
#
# Usage:
#   scripts/macos/desktop-install.sh --profile development   # SIWA (Apple Development)
#   scripts/macos/desktop-install.sh --profile distribution  # Developer ID + optional notarize
#
# development requires:
#   - Apple Development signing identity
#   - Mac Development provisioning profile with Sign in with Apple
# distribution uses Developer ID + Entitlements.plist (no SIWA) and notarizes when APPLE_ID set.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

profile=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      profile="${2:?--profile requires development|distribution}"
      shift 2
      ;;
    -h | --help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
    *)
      echo "desktop-install: unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$profile" ]]; then
  echo "desktop-install: pass --profile development|distribution" >&2
  exit 2
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "desktop-install: macOS only" >&2
  exit 1
fi

resolve_development_identity() {
  if [[ -n "${APPLE_DEVELOPMENT_SIGNING_IDENTITY:-}" ]]; then
    printf '%s\n' "$APPLE_DEVELOPMENT_SIGNING_IDENTITY"
    return 0
  fi
  # Prefer an explicit Apple Development line from the keychain.
  local line
  line="$(security find-identity -v -p codesigning 2>/dev/null | grep 'Apple Development:' | head -1 || true)"
  if [[ -n "$line" ]]; then
    sed -E 's/.*"(.+)"/\1/' <<<"$line"
    return 0
  fi
  return 1
}

resolve_development_profile() {
  if [[ -n "${LATTICE_DEVELOPMENT_PROVISION_PROFILE:-}" ]]; then
    printf '%s\n' "$LATTICE_DEVELOPMENT_PROVISION_PROFILE"
    return 0
  fi
  local candidates=(
    "$ROOT/secrets/apple/dev.lattice.desktop.development.provisionprofile"
    "$HOME/Downloads/Lattice_App_Development_Profile.provisionprofile"
  )
  local path
  for path in "${candidates[@]}"; do
    if [[ -f "$path" ]]; then
      printf '%s\n' "$path"
      return 0
    fi
  done
  return 1
}

case "$profile" in
  development)
    if ! identity="$(resolve_development_identity)"; then
      echo "desktop-install: no Apple Development identity found." >&2
      echo "  Set APPLE_DEVELOPMENT_SIGNING_IDENTITY or install an Apple Development cert." >&2
      exit 1
    fi
    if ! provision="$(resolve_development_profile)"; then
      echo "desktop-install: missing Mac Development provisioning profile." >&2
      echo "  Place it at secrets/apple/dev.lattice.desktop.development.provisionprofile" >&2
      echo "  or set LATTICE_DEVELOPMENT_PROVISION_PROFILE." >&2
      exit 1
    fi
    export APPLE_SIGNING_IDENTITY="$identity"
    export LATTICE_CODESIGN_PROFILE=development
    export LATTICE_EMBEDDED_PROVISION_PROFILE="$provision"
    echo "desktop-install: profile=development"
    echo "desktop-install: identity=$APPLE_SIGNING_IDENTITY"
    echo "desktop-install: provision=$LATTICE_EMBEDDED_PROVISION_PROFILE"
    ;;
  distribution)
    : "${APPLE_SIGNING_IDENTITY:?Set APPLE_SIGNING_IDENTITY (Developer ID Application)}"
    if [[ "$APPLE_SIGNING_IDENTITY" != Developer\ ID* ]]; then
      echo "desktop-install: distribution profile expects Developer ID Application identity" >&2
      echo "  got: $APPLE_SIGNING_IDENTITY" >&2
      exit 1
    fi
    export LATTICE_CODESIGN_PROFILE=release
    unset LATTICE_EMBEDDED_PROVISION_PROFILE || true
    echo "desktop-install: profile=distribution (Developer ID; native SIWA disabled)"
    echo "desktop-install: identity=$APPLE_SIGNING_IDENTITY"
    ;;
  *)
    echo "desktop-install: unknown --profile $profile" >&2
    exit 2
    ;;
esac

pnpm install --frozen-lockfile --prefer-offline
# Keep the Nix apple-sdk DEVELOPER_DIR/SDKROOT for the Cargo build.
#
# Tauri auto-notarizes when APPLE_ID/APPLE_PASSWORD are present. That is wrong for
# both profiles here: Development certs cannot be notarized, and distribution
# notarization must run *after* we bundle sidecars and re-sign (codesign-app.sh).
echo "desktop-install: building tauri app (notarize deferred)"
env -u APPLE_ID -u APPLE_PASSWORD -u APPLE_API_KEY -u APPLE_API_ISSUER -u APPLE_API_KEY_PATH \
  pnpm --filter @lattice/desktop exec tauri build --bundles app --features voice-embedded

echo "desktop-install: building latticed / lattice-agentd / lattice-wasi-seatbelt / lattice-embed-host / lattice-voice-host"
cargo build --release -p lattice-daemon --bin latticed
cargo build --release -p lattice-agentd --bin lattice-agentd
cargo build --release -p lattice-agentd --bin lattice-wasi-seatbelt
# shellcheck disable=SC1091
. scripts/macos/llama-cpp-nix-sdk.sh
cargo build --release -p lattice-embed-host --bin lattice-embed-host --features llama-cpp
cargo build --release -p lattice-voice-host --bin lattice-voice-host --features fluidaudio || \
  cargo build --release -p lattice-voice-host --bin lattice-voice-host

echo "desktop-install: verifying production sidecars"
for bin in latticed lattice-agentd lattice-wasi-seatbelt lattice-embed-host lattice-voice-host; do
  if [[ ! -f "target/release/$bin" ]]; then
    echo "desktop-install: missing target/release/$bin after build" >&2
    exit 1
  fi
done
backends="$(target/release/lattice-embed-host backends || true)"
echo "desktop-install: lattice-embed-host backends:"$'\n'"$backends"
if ! printf '%s\n' "$backends" | grep -qx 'llama-cpp'; then
  echo "desktop-install: lattice-embed-host must list llama-cpp (build with --features llama-cpp)" >&2
  exit 1
fi

if [[ -d /Applications/Xcode.app/Contents/Developer ]]; then
  export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
elif [[ -d /Library/Developer/CommandLineTools ]]; then
  export DEVELOPER_DIR=/Library/Developer/CommandLineTools
fi

app_src="target/release/bundle/macos/Lattice.app"
if [[ ! -d "$app_src" ]]; then
  alt_src="apps/desktop/src-tauri/target/release/bundle/macos/Lattice.app"
  if [[ -d "$alt_src" ]]; then
    app_src="$alt_src"
  else
    echo "desktop-install: missing bundle at $app_src" >&2
    exit 1
  fi
fi

macos_dir="$app_src/Contents/MacOS"
for dylib in libLatticeVoiceBridge.dylib libLatticeAudioBridge.dylib libLatticeApprovalBridge.dylib libLatticeAppleSignInBridge.dylib; do
  src="target/release/$dylib"
  if [[ -f "$src" ]]; then
    cp -f "$src" "$macos_dir/$dylib"
    echo "desktop-install: bundled $dylib"
  else
    echo "desktop-install: warning: missing $src" >&2
  fi
done

appex_out="$PWD/target/macos/LatticeQuickLook.appex"
if bash scripts/macos/build-quicklook-appex.sh "$appex_out"; then
  mkdir -p "$app_src/Contents/PlugIns"
  rm -rf "$app_src/Contents/PlugIns/LatticeQuickLook.appex"
  cp -R "$appex_out" "$app_src/Contents/PlugIns/LatticeQuickLook.appex"
  echo "desktop-install: bundled LatticeQuickLook.appex"
fi

for bin in latticed lattice-agentd lattice-wasi-seatbelt lattice-embed-host lattice-voice-host; do
  src="target/release/$bin"
  cp -f "$src" "$macos_dir/$bin"
  chmod +x "$macos_dir/$bin"
  echo "desktop-install: bundled $bin"
done

bash scripts/release/codesign-app.sh

if [[ "$profile" = "distribution" ]]; then
  if [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
    bash scripts/release/notarize-app.sh
    bash scripts/release/staple-app.sh
  else
    echo "desktop-install: warning: APPLE_ID/PASSWORD/TEAM_ID unset — skipping notarize" >&2
  fi
else
  echo "desktop-install: skipping notarize (development profile; local machine only)"
fi

dest="${LATTICE_INSTALL_DIR:-/Applications}/Lattice.app"
echo "desktop-install: installing → $dest"
rm -rf "$dest"
ditto "$app_src" "$dest"
codesign -dv --verbose=2 "$dest" || true
if command -v spctl >/dev/null 2>&1; then
  spctl --assess --verbose=4 --type execute "$dest" || true
fi
echo "desktop-install: done. Open with: open \"$dest\""
echo "desktop-install: for agent env, from ecosystem root:"
echo "  ./scripts/exec-for-dev.sh -- \"$dest/Contents/MacOS/lattice-desktop\""
