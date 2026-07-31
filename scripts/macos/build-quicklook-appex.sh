#!/usr/bin/env bash
# Build LatticeQuickLook.appex (scripted, no Xcode project required).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$ROOT/apps/desktop/macos/LatticeQuickLook"
OUT_DIR="${1:-$ROOT/target/macos/LatticeQuickLook.appex}"
ENTITLEMENTS="$SRC/LatticeQuickLook.entitlements"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "build-quicklook-appex: skipping on non-macOS" >&2
  exit 0
fi

if [[ ! -f "$SRC/PreviewViewController.swift" ]]; then
  echo "build-quicklook-appex: missing sources in $SRC" >&2
  exit 1
fi

DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
if [[ ! -d "$DEVELOPER_DIR" ]]; then
  echo "build-quicklook-appex: Xcode not found at $DEVELOPER_DIR" >&2
  exit 1
fi
export DEVELOPER_DIR
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
SWIFTC="$(xcrun --find swiftc)"
ARCH="$(uname -m)"
TARGET="${ARCH}-apple-macosx14.0"

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/Contents/MacOS"

# Compile as an appex executable (entry point is PlugInKit's _NSExtensionMain).
"$SWIFTC" \
  -parse-as-library \
  -target "$TARGET" \
  -sdk "$SDKROOT" \
  -O \
  -framework Cocoa \
  -framework Quartz \
  -framework UniformTypeIdentifiers \
  -emit-executable \
  -Xlinker -e -Xlinker _NSExtensionMain \
  -module-name LatticeQuickLook \
  -o "$OUT_DIR/Contents/MacOS/LatticeQuickLook" \
  "$SRC/PreviewViewController.swift"

# Info.plist with concrete executable name (no Xcode substitutions).
cat >"$OUT_DIR/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>Lattice Quick Look</string>
	<key>CFBundleExecutable</key>
	<string>LatticeQuickLook</string>
	<key>CFBundleIdentifier</key>
	<string>dev.lattice.desktop.quicklook</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>LatticeQuickLook</string>
	<key>CFBundlePackageType</key>
	<string>XPC!</string>
	<key>CFBundleShortVersionString</key>
	<string>0.1.0</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>NSExtension</key>
	<dict>
		<key>NSExtensionAttributes</key>
		<dict>
			<key>QLSupportedContentTypes</key>
			<array>
				<string>net.daringfireball.markdown</string>
				<string>public.plain-text</string>
				<string>public.json</string>
				<string>dev.lattice.page</string>
			</array>
			<key>QLSupportsSearchableItems</key>
			<false/>
		</dict>
		<key>NSExtensionPointIdentifier</key>
		<string>com.apple.quicklook.preview</string>
		<key>NSExtensionPrincipalClass</key>
		<string>PreviewViewController</string>
	</dict>
</dict>
</plist>
PLIST

# Entitlements stay outside the bundle: codesign treats files under Contents/
# (other than Info.plist / MacOS / Resources / _CodeSignature) as nested code
# and fails Developer ID signing with "code object is not signed at all".

# Ad-hoc sign so the bundle is loadable in local installs; release re-signs with Developer ID.
codesign --force --sign - \
  --entitlements "$ENTITLEMENTS" \
  "$OUT_DIR" 2>/dev/null || true

echo "build-quicklook-appex: ok → $OUT_DIR"
