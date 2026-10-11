#!/usr/bin/env bash
# Every Linux packaging tool downloaded matches its pinned hash.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script=$root/packaging/linux/package.sh
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

pack=https://github.com/AppImage/appimagetool/releases/download/1.9.1
deploy=https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1
runtime=https://github.com/AppImage/type2-runtime/releases/download/20251108
nfpm=https://github.com/goreleaser/nfpm/releases/download/v2.47.0
curl -fsSI --max-time 20 "$pack/appimagetool-x86_64.AppImage" >/dev/null 2>&1 || {
  echo "SKIP linux-pins: GitHub is not reachable"
  exit 0
}
grep -q "$pack/appimagetool-\$arch.AppImage" "$script" || fail "the script downloads another appimagetool"
grep -q "$deploy/linuxdeploy-\$arch.AppImage" "$script" || fail "the script downloads another linuxdeploy"
grep -q "$runtime/runtime-\$arch" "$script" || fail "the script downloads another runtime"
grep -q "$nfpm/nfpm_2.47.0_Linux_\$nfpm_arch.tar.gz" "$script" || fail "the script downloads another nfpm"
grep -q -- '--runtime-file "\$runtime"' "$script" || fail "appimagetool fetches its own runtime"
for pair in x86_64:x86_64 aarch64:arm64; do
  arch=${pair%:*}
  for url in "$pack/appimagetool-$arch.AppImage" "$deploy/linuxdeploy-$arch.AppImage" "$runtime/runtime-$arch" \
    "$nfpm/nfpm_2.47.0_Linux_${pair#*:}.tar.gz"; do
    sha=$(curl -fsSL "$url" | shasum -a 256 | cut -d' ' -f1)
    grep -q "$sha" "$script" || fail "$url has sha256 $sha, which the script does not pin"
  done
done
echo "linux-pins: 8 downloads match their pins"
