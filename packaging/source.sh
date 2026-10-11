#!/usr/bin/env bash
# Usage: source.sh <dist>; the source of what this commit builds.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dist=${1:?the folder to write into}
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
# Binaries match a commit only when the tree is clean.
changed=$(git -C "$root" status --porcelain --untracked-files=all)
[ -z "$changed" ] || {
  echo "source.sh: uncommitted changes; commit before a release:" >&2
  echo "$changed" >&2
  exit 1
}
commit=$(git -C "$root" rev-parse --short=12 HEAD)
mkdir -p "$dist"
# Our source as published: the committed tree.
ours=$dist/ElyOffice-$version-$commit-source.tar.gz
git -C "$root" archive --format=tar.gz --prefix="ElyOffice-$version/" -o "$ours" HEAD
echo "$ours"
# Copyleft upstream as shipped: patched MuPDF (AGPL), Git (GPLv2).
upstream=$dist/ElyOffice-$version-$commit-upstream-source.tar.gz
[ -f "$root/engines/mupdf-src/COPYING" ] || {
  echo "source.sh: no engines/mupdf-src; run scripts/build-mupdf.sh" >&2
  exit 1
}
[ -f "$root/reference/git/COPYING" ] || {
  echo "source.sh: no reference/git; run scripts/fetch-references.sh" >&2
  exit 1
}
tar -czf "$upstream" --exclude .git --exclude .DS_Store -C "$root/engines" mupdf-src -C "$root/reference" git
echo "$upstream"
