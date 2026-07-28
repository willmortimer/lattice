#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/release/_common.sh
source "$(cd "$(dirname "$0")" && pwd)/_common.sh"
lattice_release_ensure_root

missing=0
require_env() {
  local name="$1"
  if [ -z "${!name:-}" ]; then
    echo "release-env-validate: missing required env: $name" >&2
    missing=1
  fi
}
require_env APPLE_SIGNING_IDENTITY
require_env APPLE_ID
require_env APPLE_PASSWORD
require_env APPLE_TEAM_ID
if [ "$missing" -ne 0 ]; then
  echo "release-env-validate: load Apple secrets first, e.g.:" >&2
  echo "  ./scripts/with-secrets.sh apple -- nxr task release-env-validate" >&2
  echo "  # from ecosystem: ./scripts/with-secrets.sh apple -- nxr -f ./lattice --cwd lattice task release-env-validate" >&2
  exit 1
fi

case "$APPLE_SIGNING_IDENTITY" in
  *"Developer ID Application"*) ;;
  *"Apple Development"*)
    echo "release-env-validate: APPLE_SIGNING_IDENTITY looks like Apple Development." >&2
    echo "  Notarization needs Developer ID Application (paid Apple Developer Program)." >&2
    exit 1
    ;;
  *)
    echo "release-env-validate: warning: identity is not 'Developer ID Application: …'" >&2
    echo "  continuing with: $APPLE_SIGNING_IDENTITY" >&2
    ;;
esac

if ! command -v xcrun >/dev/null 2>&1; then
  echo "release-env-validate: xcrun not found (need Xcode or CLT for notarytool/stapler)" >&2
  exit 1
fi
if ! xcrun --find notarytool >/dev/null 2>&1; then
  echo "release-env-validate: notarytool missing — install full Xcode Command Line Tools" >&2
  exit 1
fi

echo "release-env-validate: Apple env + notarytool OK"
if [ "${LATTICE_RELEASE_VALIDATE_ONLY:-}" = "1" ] || [ "${LATTICE_RELEASE_VALIDATE_ONLY:-}" = "true" ]; then
  echo "release-env-validate: LATTICE_RELEASE_VALIDATE_ONLY set — stop here (do not run full desktop-release DAG)."
fi
