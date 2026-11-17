#!/bin/bash
# Builds the downloadable NeedleX: one app for Apple silicon and Intel Macs,
# packed as a disk image (drag to Applications) and a zip, plus the signed
# update for the in-app updater, in dist/.
#
#   ./scripts/release.sh             build only
#   ./scripts/release.sh --publish   build, then publish it as the GitHub
#                                    release v<version> (npm run release)
#
# The update is signed with the updater key: TAURI_SIGNING_PRIVATE_KEY and
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD, or else ~/.needlex-signing/updater.key and
# updater-password. Installed copies only accept updates signed with it.
#
# The app is ad-hoc signed, not notarized (that needs a paid Apple Developer
# account), so on first launch macOS asks you to confirm it (see the README).
# Needs: Xcode Command Line Tools, Rust with the aarch64 and x86_64 Apple
# targets, Node.js (npm install); for --publish also the GitHub CLI (gh).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
DIST="$ROOT/dist"
APP_NAME="NeedleX"
TAG="v$VERSION"
PUBLISH=false
[ "${1:-}" = "--publish" ] && PUBLISH=true
echo "NeedleX $VERSION"

# the updater key from this Mac when it isn't in the environment (CI secrets)
KEYS="${NEEDLEX_SIGNING_DIR:-$HOME/.needlex-signing}"
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -f "$KEYS/updater.key" ]; then
  TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEYS/updater.key")"
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$KEYS/updater-password" 2>/dev/null || true)"
  export TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
fi

# before the long build: everything publishing needs
if $PUBLISH; then
  fail() { echo "release: $*" >&2; exit 1; }
  [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || fail "no updater key (see the top of this script); a release without a signed update would stop in-app updates"
  gh auth status >/dev/null 2>&1 || fail "the GitHub CLI isn't logged in (gh auth login)"
  [ -z "$(git status --porcelain)" ] || fail "commit your changes first"
  HEAD="$(git rev-parse HEAD)"
  git fetch -q origin
  [ "$(git rev-parse origin/main)" = "$HEAD" ] || fail "push main first: the release is made from the commit on GitHub"
  ! git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null || fail "$TAG already exists; bump the version in tauri.conf.json, Cargo.toml and package.json"
  grep -q "^## $VERSION " CHANGELOG.md || fail "CHANGELOG.md has no '## $VERSION – <date>' section (it becomes the update's notes)"
fi

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

# For the in-app updater: the app as a .tar.gz (NeedleX.app at its top, no
# AppleDouble files), its signature, and latest.json, which the app downloads
# from the latest release to see if there is a newer version. Signing needs the
# updater key (see the top).
TGZ="$DIST/$APP_NAME-$VERSION.app.tar.gz"
COPYFILE_DISABLE=1 tar -czf "$TGZ" -C "$(dirname "$APP")" "$APP_NAME.app"
if [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  npx tauri signer sign "$TGZ" >/dev/null
  REPO="${GITHUB_REPOSITORY:-n1h4r1k4/needlex-mac}"
  awk -v v="$VERSION" '$0 ~ "^## " v "( |$)" {on=1; next} /^## / {on=0} on' CHANGELOG.md > "$STAGE/notes.md"
  node -e '
    const fs = require("fs");
    const [version, url, sig, notes, out] = process.argv.slice(1);
    const p = { signature: fs.readFileSync(sig, "utf8").trim(), url };
    fs.writeFileSync(out, JSON.stringify({
      version,
      notes: fs.readFileSync(notes, "utf8").trim(),
      pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
      // one universal app for both kinds of Mac
      platforms: { "darwin-aarch64": p, "darwin-x86_64": p },
    }, null, 2) + "\n");
  ' "$VERSION" "https://github.com/$REPO/releases/download/v$VERSION/$(basename "$TGZ")" \
    "$TGZ.sig" "$STAGE/notes.md" "$DIST/latest.json"
else
  echo "no TAURI_SIGNING_PRIVATE_KEY: not signing the update, so no latest.json"
  rm "$TGZ"
fi
(cd "$DIST" && shasum -a 256 $(ls *.dmg *.zip *.tar.gz 2>/dev/null) > SHA256SUMS.txt)
ls -lh "$DIST"
cat "$DIST/SHA256SUMS.txt"

if $PUBLISH; then
  echo "publishing $TAG…"
  TITLE="$(./scripts/release-notes.sh "$VERSION" "$STAGE/release-notes.md")"
  # creates the tag on GitHub too; the Release workflow then sees the release
  # is already there and leaves it alone
  gh release create "$TAG" --target "$HEAD" --title "$TITLE" --notes-file "$STAGE/release-notes.md" --latest \
    "$DIST"/*.dmg "$DIST"/*.zip "$DIST"/*.app.tar.gz "$DIST"/*.app.tar.gz.sig "$DIST/latest.json" "$DIST/SHA256SUMS.txt"
  echo "installed copies of NeedleX will offer $VERSION within a few hours (or at once with Check for Updates…)"
fi
