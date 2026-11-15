#!/bin/sh
# Re-signs the built app with the local identity from make-signing-identity.sh.
# Ad-hoc signatures change with every build, and macOS ties permissions (screen
# recording, system audio, automation, location, calendars) to the signature –
# a stable one keeps them across rebuilds. Without the identity: does nothing.
set -e
APP="${1:-src-tauri/target/release/bundle/macos/NeedleX.app}"
D="${NEEDLEX_SIGNING_DIR:-$HOME/.needlex-signing}"
KC="$D/signing.keychain-db"
ID="NeedleX Local Signing"
if [ ! -f "$KC" ]; then
  echo "sign-local: no local identity (run scripts/make-signing-identity.sh) – keeping the ad-hoc signature"
  exit 0
fi
security unlock-keychain -p "$(cat "$D/keychain-password")" "$KC"
codesign --force --keychain "$KC" -s "$ID" "$APP/Contents/MacOS/needlex-helper"
codesign --force --keychain "$KC" -s "$ID" "$APP"
codesign --verify --strict "$APP"
codesign -dr - "$APP" 2>&1 | tail -1
