#!/bin/bash
# Writes the GitHub release notes for a version to a file and prints the release
# title. Uses docs/releases/v<version>.md when there is one (title on its first
# line as "<!-- title: … -->"), else that version's part of CHANGELOG.md
# followed by .github/release-install.md.
#
#   ./scripts/release-notes.sh 0.1.1 notes.md
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$1"
OUT="$2"
TITLE="NeedleX $VERSION"
if [ -f "$ROOT/docs/releases/v$VERSION.md" ]; then
  cp "$ROOT/docs/releases/v$VERSION.md" "$OUT"
  T="$(sed -n '1s/^<!-- title: \(.*\) -->$/\1/p' "$OUT")"
  if [ -n "$T" ]; then TITLE="$T"; fi
else
  awk -v v="$VERSION" '$0 ~ "^## " v "( |$)" {on=1; next} /^## / {on=0} on' "$ROOT/CHANGELOG.md" > "$OUT"
  cat "$ROOT/.github/release-install.md" >> "$OUT"
fi
echo "$TITLE"
