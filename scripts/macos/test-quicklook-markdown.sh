#!/usr/bin/env bash
# Unit-test MarkdownHTML without an Xcode project / XCTest target.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$ROOT/apps/desktop/macos/LatticeQuickLook"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "test-quicklook-markdown: skipping on non-macOS" >&2
  exit 0
fi

if [[ ! -f "$SRC/MarkdownHTML.swift" || ! -f "$SRC/tests/markdown_html_test.swift" ]]; then
  echo "test-quicklook-markdown: missing sources in $SRC" >&2
  exit 1
fi

if ! command -v xcrun >/dev/null 2>&1; then
  echo "test-quicklook-markdown: skipping (xcrun not found)" >&2
  exit 0
fi

SWIFTC="$(xcrun --find swiftc 2>/dev/null || true)"
if [[ -z "$SWIFTC" ]]; then
  echo "test-quicklook-markdown: skipping (swiftc not found)" >&2
  exit 0
fi

DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
export DEVELOPER_DIR
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
ARCH="$(uname -m)"
TARGET="${ARCH}-apple-macosx14.0"

OUT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/lattice-ql-md.XXXXXX")"
trap 'rm -rf "$OUT_DIR"' EXIT
OUT="$OUT_DIR/markdown-html-test"

"$SWIFTC" \
  -parse-as-library \
  -target "$TARGET" \
  -sdk "$SDKROOT" \
  -O \
  -o "$OUT" \
  "$SRC/MarkdownHTML.swift" \
  "$SRC/tests/markdown_html_test.swift"

"$OUT"
echo "test-quicklook-markdown: ok"
