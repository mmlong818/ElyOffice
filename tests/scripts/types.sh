#!/usr/bin/env bash
# packaging/types.tsv reaches Linux, Windows and macOS alike.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
types=$root/packaging/types.tsv
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
count=0
for slug in $(cut -f1 "$types" | sort -u); do
  desktop=$("$root/packaging/linux/package.sh" --desktop "$slug")
  windows=$("$root/packaging/windows/types.sh" "$slug")
  rows=$(awk -F'\t' -v slug="$slug" '$1 == slug' "$types")
  while IFS=$'\t' read -r _ ext mime; do
    grep -q "^MimeType=.*$mime;" <<<"$desktop" || fail "$slug: Linux misses $mime"
    if [ "$ext" = / ]; then
      grep -q 'Directory\\shell' <<<"$windows" || fail "$slug: Windows misses folders"
    else
      grep -q "\\\\.$ext\\\\OpenWithProgids" <<<"$windows" || fail "$slug: Windows misses .$ext"
      grep -q "DeleteRegValue.*\\\\.$ext\\\\OpenWithProgids" <<<"$windows" || fail "$slug: uninstall keeps .$ext"
    fi
    count=$((count + 1))
  done <<<"$rows"
  # One macOS type per MIME type; md and markdown share.
  utis=$("$root/packaging/macos/documents.sh" "$slug" | grep -c '^      <string>')
  mimes=$(cut -f3 <<<"$rows" | sort -u | wc -l | tr -d ' ')
  [ "$utis" -eq "$mimes" ] || fail "$slug: macOS declares $utis types, the table $mimes"
done
# Never the default: Windows fills only a handlerless type.
for s in $(cut -f1 "$types" | sort -u); do
  "$root/packaging/windows/types.sh" "$s" | awk -v slug="$s" '
    /WriteRegStr HKCU "Software\\Classes\\\.[a-z]*" ""/ && (prev !~ /\$\{OrIf\} \$\{Errors\}$/ || prev2 !~ /\$\{If\} \$0 == ""$/) { print slug ": " $0; bad = 1 }
    { prev2 = prev; prev = $0 } END { exit bad }' || fail "Windows sets a default handler"
done
# Explorer's New makes docx, xlsx, pptx and md.
fresh=0
for ext in docx xlsx pptx md; do
  slug=$(awk -F'\t' -v ext="$ext" '$2 == ext { print $1 }' "$types")
  windows=$("$root/packaging/windows/types.sh" "$slug")
  grep -qF "\\.$ext\\ShellNew\" \"FileName\" \"\$INSTDIR\\new.$ext\"" <<<"$windows" ||
    fail "$slug: New misses .$ext"
  fresh=$((fresh + 1))
done
echo "ok: types ($count file types on three systems, $fresh in New)"
