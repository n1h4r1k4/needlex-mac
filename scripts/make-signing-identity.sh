#!/bin/sh
# Creates a free, self-signed code-signing identity for local builds, in its own
# keychain under ~/.needlex-signing (not your login keychain). Run once.
# With it, scripts/sign-local.sh gives every build the same signature, so macOS
# keeps the permissions you gave NeedleX instead of asking again after each build.
set -e
D="${NEEDLEX_SIGNING_DIR:-$HOME/.needlex-signing}"
KC="$D/signing.keychain-db"
[ -f "$KC" ] && { echo "already there: $KC"; exit 0; }
mkdir -p "$D"; chmod 700 "$D"; cd "$D"
openssl rand -hex 24 > keychain-password; chmod 600 keychain-password
PW=$(cat keychain-password)
openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes -keyout key.pem -out cert.pem \
  -subj "/CN=NeedleX Local Signing" \
  -addext "basicConstraints=critical,CA:false" -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning" 2>/dev/null
openssl pkcs12 -export -inkey key.pem -in cert.pem -out identity.p12 -passout pass:needlex \
  -certpbe PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1
security create-keychain -p "$PW" "$KC"
security set-keychain-settings "$KC"            # no auto-lock
security unlock-keychain -p "$PW" "$KC"
security import identity.p12 -k "$KC" -P needlex -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$PW" "$KC" >/dev/null
rm -f key.pem identity.p12
echo "created the NeedleX Local Signing identity in $KC"
