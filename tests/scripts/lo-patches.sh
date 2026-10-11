#!/usr/bin/env bash
# The series applies from a prefix, then as a no-op.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
src=$root/engines/lo-src
[ -d "$src/.git" ] || { echo "SKIP: no engines/lo-src"; exit 0; }
patches=("$root"/patches/libreoffice/*.patch)
files=$(sed -n 's|^+++ b/||p' "${patches[@]}" | sort -u)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Just the touched files at the pinned commit.
tree=$work/tree
mkdir -p "$tree"
git -C "$tree" init -q
# LF, as every tree here; Windows defaults convert.
git -C "$tree" config core.autocrlf false
for f in $files; do
  if git -C "$src" cat-file -e "HEAD:$f" 2>/dev/null; then
    mkdir -p "$tree/$(dirname "$f")"
    git -C "$src" show "HEAD:$f" >"$tree/$f"
  fi
done
git -C "$tree" add -A
git -C "$tree" -c user.name=t -c user.email=t@t commit -qm pin

half=$((${#patches[@]} / 2))
for ((i = 0; i < half; i++)); do
  git -C "$tree" apply "${patches[$i]}"
done
out=$("$root/scripts/apply-lo-patches.sh" "$tree")
grep -q "^apply-lo-patches.sh: $half of ${#patches[@]} patches already" <<<"$out" || {
  echo "FAIL: the prefix was not found: $out"
  exit 1
}
out=$("$root/scripts/apply-lo-patches.sh" "$tree")
grep -q "${#patches[@]} of ${#patches[@]} patches already" <<<"$out" || {
  echo "FAIL: a second run was not a no-op: $out"
  exit 1
}
for f in $files; do
  [ ! -e "$tree/$f" ] || cmp -s "$tree/$f" "$src/$f" || {
    echo "FAIL: $f differs from engines/lo-src"
    exit 1
  }
done
echo "lo-patches: the series applies from patch $half and again as a no-op"
