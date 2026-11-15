#!/bin/bash
# Regenerates the README screenshots in docs/images from the pages' browser
# demo (the made-up song in src/demo.js). Needs the Xcode Command Line Tools.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/src"
OUT="$ROOT/docs/images"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$OUT"

echo "building the snapshot tools…"
xcrun swiftc -O "$ROOT/scripts/screenshots/snap.swift" -o "$TMP/snap"
xcrun swiftc -O "$ROOT/scripts/screenshots/crop.swift" -o "$TMP/crop"

# page, query, name, width, height, delay (s)
shot() {
  "$TMP/snap" "$SRC/$1" "$2" "$TMP/$3.png" "$4" "$5" "$6"
}
# full-screen shots: 1440 px wide JPEGs
full() {
  shot full.html "snap=1&$1" "$2" 1440 900 "${3:-6.5}"
  sips -s format jpeg -s formatOptions 78 -Z 1440 "$TMP/$2.png" --out "$OUT/$2.jpg" >/dev/null
  echo "  $2.jpg"
}
# island shots: the top of its 560x520 window
island() {
  shot index.html "$1" "$2" 560 520 3.5
  "$TMP/crop" "$TMP/$2.png" "$OUT/$2.jpg" "$3"
  echo "  $2.jpg"
}

echo "full screen and effects…"
full "" cover 4
for st in words lens stars terminal aura lines; do full "mode=lyrics&style=$st&t=4500" "fx-$st"; done
full "mode=lyrics&style=words&bg=glass&t=4500" bg-glass
full "mode=lyrics&style=stars&bg=liquid&t=4500&lt=9" bg-liquid
full "mode=lyrics&style=lens&bg=foggy&t=4500" bg-foggy
full "mode=lyrics&style=terminal&cover=1&t=8500" with-cover

echo "lock screen and desktop…"
full "lock=1&lk=record" lock-record 5
full "lock=1&lk=cover" lock-cover 5
full "lock=1&lk=lyrics&style=stars&t=4500" lock-lyrics 7
full "wall=1&mode=lyrics&style=words&t=4500" wall-words 7
for look in turntable sleeve aura; do
  shot desk.html "look=$look" "desk-$look" 1440 900 4
  sips -s format jpeg -s formatOptions 78 -Z 1440 "$TMP/desk-$look.png" --out "$OUT/desk-$look.jpg" >/dev/null
  echo "  desk-$look.jpg"
done

echo "island…"
island "view=expanded" isl-player 440
island "view=lyrics" isl-lyrics 1000
island "page=weather" isl-weather 440
island "page=calendar" isl-calendar 440
island "view=peek" isl-peek 200

echo "done: $OUT"
