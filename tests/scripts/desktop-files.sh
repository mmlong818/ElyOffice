#!/usr/bin/env bash
# Each desktop entry is valid and matches its window class.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

command -v desktop-file-validate >/dev/null || {
  echo "SKIP desktop-files: desktop-file-validate is not installed"
  exit 0
}
count=0
while IFS=$'\t' read -r slug name _; do
  file=$work/$slug.desktop
  "$root/packaging/linux/package.sh" --desktop "$slug" >"$file"
  report=$(desktop-file-validate "$file" 2>&1) || fail "$slug.desktop is not valid: $report"
  [ -z "$report" ] || fail "$slug.desktop has hints: $report"
  grep -qx "Name=$name" "$file" || fail "$slug.desktop has the wrong name"
  grep -qx "StartupWMClass=$slug" "$file" || fail "$slug.desktop does not match the window app id"
  grep -qx "Exec=$slug %F" "$file" || fail "$slug.desktop starts the wrong binary"
  count=$((count + 1))
done <"$root/packaging/apps.tsv"
[ "$count" -eq 8 ] || fail "checked $count desktop files, expected 8"
echo "desktop-files: 8 valid"
