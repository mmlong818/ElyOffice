#!/usr/bin/env bash
# Every Mach-O in the macOS engines is the named architecture.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
want=${1:?the architecture, such as x86_64}
[ "$(uname)" = Darwin ] || {
  echo "SKIP: engine-arch.sh is for macOS"
  exit 0
}
dirs=()
for dir in lo-src/instdir mupdf-build git openssl; do
  [ -d "$root/engines/$dir" ] && dirs+=("$root/engines/$dir")
done
[ ${#dirs[@]} -gt 0 ] || {
  echo "engine-arch.sh: no engines under $root/engines" >&2
  exit 1
}
checked=0 wrong=0
while IFS= read -r -d '' file; do
  archs=$(lipo -archs "$file" 2>/dev/null) || continue
  checked=$((checked + 1))
  if [ "$archs" != "$want" ]; then
    echo "engine-arch.sh: $file is $archs" >&2
    wrong=$((wrong + 1))
  fi
done < <(find "${dirs[@]}" -type f \( -perm -u+x -o -name '*.dylib' -o -name '*.a' -o -name '*.so' \) -print0)
[ "$checked" -gt 0 ] || {
  echo "engine-arch.sh: no Mach-O files found" >&2
  exit 1
}
[ "$wrong" -eq 0 ] || {
  echo "FAIL: $wrong of $checked Mach-O files are not $want" >&2
  exit 1
}
echo "engine-arch.sh: all $checked Mach-O files are $want"
