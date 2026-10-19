#!/usr/bin/env bash
# Builds dist/ElyOffice-<version>-windows-x86_64-setup.exe from the apps' setups.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
dist=${ELY_DIST:-$root/dist}
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
makensis=${MAKENSIS:-"/c/Program Files (x86)/NSIS/makensis.exe"}
[ -x "$makensis" ] || {
  echo "bundle.sh: no makensis at $makensis; install NSIS" >&2
  exit 1
}
apps=$root/target/stage/bundle-apps.nsh
mkdir -p "$(dirname "$apps")"
: >"$apps"
for name in $(awk -F'\t' '$4 == 1 { print $2 }' "$root/packaging/apps.tsv"); do
  setup=$dist/$name-$version-windows-x86_64-setup.exe
  [ -f "$setup" ] || {
    echo "bundle.sh: no $setup; run installer.sh first" >&2
    exit 1
  }
  printf '!insertmacro ElyApp "%s" "%s"\n' "$name" "$(cygpath -w "$setup")" >>"$apps"
done
out=$dist/ElyOffice-$version-windows-x86_64-setup.exe
"$makensis" -V2 -INPUTCHARSET UTF8 -DVERSION="$version" -DAPPS="$(cygpath -w "$apps")" \
  -DOUTFILE="$(cygpath -w "$out")" "$(cygpath -w "$root/packaging/windows/bundle.nsi")"
echo "$out"
