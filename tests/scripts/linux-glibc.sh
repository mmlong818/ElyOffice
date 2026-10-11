#!/usr/bin/env bash
# Linux builds need glibc 2.43 at most, as Ubuntu 26.04.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
limit=2.43
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
for tool in objdump readelf; do
  command -v "$tool" >/dev/null || {
    echo "SKIP: no $tool"
    exit 0
  }
done
release=$root/target/ubuntu/release
lo=$root/engines/lo-src/instdir/program
required=("$release"/ely-{doc,table,show,pdf,tree,mark,lok-worker,pdf-worker} "$lo/libmergedlo.so"
  "$root/engines/git/bin/git" "$root/engines/cef/libcef.so")
for file in "${required[@]}"; do
  [ -f "$file" ] || fail "no $file; build in packaging/linux/container.sh"
  magic=$(head -c 4 "$file" | od -An -c | tr -d ' ') || fail "cannot read $file"
  [ "$magic" = '177ELF' ] || fail "$file is not an ELF file"
done
list=$(mktemp)
trap 'rm -f "$list"' EXIT
find -L "$release"/ely-{doc,table,show,pdf,tree,mark,lok-worker,pdf-worker} "$lo" "$root/engines/git" \
  "$root/engines/cef" -type f -print0 >"$list" || fail "cannot list every file"
checked=0
while IFS= read -r -d '' file; do
  magic=$(head -c 4 "$file" | od -An -c | tr -d ' ') || fail "cannot read $file"
  [ "$magic" = '177ELF' ] || continue
  checked=$((checked + 1))
  headers=$(readelf -lW "$file") || fail "readelf cannot read $file"
  # Static binaries, like git-lfs, need no glibc.
  grep -q ' DYNAMIC ' <<<"$headers" || continue
  symbols=$(objdump -T "$file") || fail "objdump cannot read $file"
  need=$(sed -n 's/.*GLIBC_\([0-9.]*\).*/\1/p' <<<"$symbols" | sort -V | tail -n 1)
  [ -z "$need" ] && continue
  [ "$(printf '%s\n%s\n' "$need" "$limit" | sort -V | tail -n 1)" = "$limit" ] ||
    fail "${file#"$root"/} needs glibc $need, above $limit"
done <"$list"
echo "linux: $checked ELF files need glibc $limit at most"
