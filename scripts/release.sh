#!/bin/bash
# Builds the downloadable NeedleX: one app for Apple silicon and Intel Macs,
# packed as a disk image (drag to Applications) and a zip, in dist/.
#
#   ./scripts/release.sh
#
# The app is ad-hoc signed, not notarized (that needs a paid Apple Developer
# account), so on first launch macOS asks you to confirm it (see the README).
# Needs: Xcode Command Line Tools, Rust with the aarch64 and x86_64 Apple
# targets, Node.js (npm install).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
DIST="$ROOT/dist"
APP_NAME="NeedleX"
echo "NeedleX $VERSION"

rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null

# the Swift helper for both kinds of Mac, joined into one binary
echo "building the helper…"
mkdir -p src-tauri/binaries
for arch in arm64 x86_64; do
  xcrun swiftc -O -swift-version 5 -target "$arch-apple-macos13.0" \
    src-tauri/helper/helper.swift -o "src-tauri/binaries/needlex-helper-$arch"
done
lipo -create src-tauri/binaries/needlex-helper-arm64 src-tauri/binaries/needlex-helper-x86_64 \
  -output src-tauri/binaries/needlex-helper-universal-apple-darwin
rm src-tauri/binaries/needlex-helper-arm64 src-tauri/binaries/needlex-helper-x86_64

echo "building the app (Apple silicon + Intel)…"
npx tauri build --target universal-apple-darwin --bundles app

APP="src-tauri/target/universal-apple-darwin/release/bundle/macos/$APP_NAME.app"
lipo -info "$APP/Contents/MacOS/needlex" "$APP/Contents/MacOS/needlex-helper"

# ad-hoc signature, helper first (the app's signature seals it)
codesign --force -s - "$APP/Contents/MacOS/needlex-helper"
codesign --force -s - "$APP"
codesign --verify --strict --deep "$APP"

echo "packing…"
rm -rf "$DIST" && mkdir -p "$DIST"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
ditto "$APP" "$STAGE/$APP_NAME.app"
ln -s /Applications "$STAGE/Applications"
DMG="$DIST/$APP_NAME-$VERSION.dmg"
ZIP="$DIST/$APP_NAME-$VERSION.zip"
hdiutil create -volname "$APP_NAME $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG" >/dev/null
ditto -c -k --keepParent "$APP" "$ZIP"
(cd "$DIST" && shasum -a 256 "$(basename "$DMG")" "$(basename "$ZIP")" > SHA256SUMS.txt)
ls -lh "$DIST"
cat "$DIST/SHA256SUMS.txt"
