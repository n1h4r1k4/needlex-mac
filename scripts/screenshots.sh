#!/bin/bash
# Regenerates the README screenshots in docs/images from the pages' browser
# demo. Needs the Xcode Command Line Tools.
#
# By default every shot plays the made-up song in src/demo.js. To use real
# songs, make song files with scripts/screenshots/song.py and pass them:
#   SONG_A=a.js SONG_B=b.js ./scripts/screenshots.sh
# Shots alternate between song A and song B, as listed below. The `t=` times
# (song position in ms) suit the songs used for the README; change them for
# other songs.
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

song() { if [ "$1" = a ]; then echo "${SONG_A:-}"; else echo "${SONG_B:-}"; fi; }
# page, query, name, width, height, delay (s), song (a | b)
shot() {
  local s; s="$(song "$7")"
  if [ -n "$s" ]; then "$TMP/snap" "$SRC/$1" "$2" "$TMP/$3.png" "$4" "$5" "$6" "$s"
  else "$TMP/snap" "$SRC/$1" "$2" "$TMP/$3.png" "$4" "$5" "$6"; fi
}
# full-screen shots: 1440 px wide JPEGs
full() { # query, name, song, [delay]
  shot full.html "snap=1&$1" "$2" 1440 900 "${4:-6.5}" "$3"
  sips -s format jpeg -s formatOptions 78 -Z 1440 "$TMP/$2.png" --out "$OUT/$2.jpg" >/dev/null
  echo "  $2.jpg"
}
# island shots: the top of its 560x520 window
island() { # query, name, crop height (px), song
  shot index.html "$1" "$2" 560 520 3.5 "$4"
  "$TMP/crop" "$TMP/$2.png" "$OUT/$2.jpg" "$3"
  echo "  $2.jpg"
}
desk() { # look, song
  shot desk.html "look=$1" "desk-$1" 1440 900 4 "$2"
  sips -s format jpeg -s formatOptions 78 -Z 1440 "$TMP/desk-$1.png" --out "$OUT/desk-$1.jpg" >/dev/null
  echo "  desk-$1.jpg"
}

A="t=58500" # song A: a chorus line
B="t=62500" # song B: a verse line

echo "full screen and effects…"
full "" cover a 4
full "mode=lyrics&style=words&$A" fx-words a
full "mode=lyrics&style=lens&$B" fx-lens b
full "mode=lyrics&style=stars&$A" fx-stars a
full "mode=lyrics&style=terminal&$B" fx-terminal b
full "mode=lyrics&style=aura&$A" fx-aura a
full "mode=lyrics&style=lines&$B" fx-lines b
full "mode=lyrics&style=words&bg=glass&$B" bg-glass b
full "mode=lyrics&style=stars&bg=liquid&lt=9&$A" bg-liquid a
full "mode=lyrics&style=lens&bg=foggy&$B" bg-foggy b
full "mode=lyrics&style=terminal&cover=1&$A" with-cover a

echo "lock screen and desktop…"
full "lock=1&lk=record" lock-record a 5
full "lock=1&lk=cover" lock-cover b 5
full "lock=1&lk=lyrics&style=stars&t=58000" lock-lyrics a 7
full "wall=1&mode=lyrics&style=words&t=62000" wall-words b 7
desk turntable a
desk sleeve b
desk aura a

echo "island…"
island "view=expanded" isl-player 440 a
island "view=lyrics" isl-lyrics 1000 a
island "page=weather" isl-weather 440 b
island "page=calendar" isl-calendar 440 a
island "view=peek" isl-peek 200 b

echo "done: $OUT"
