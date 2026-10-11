#!/usr/bin/env bash
# Builds dist/<Name>-<version>-windows-x86_64-setup.exe. Usage: installer.sh <slug>
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
slug=${1:?app slug, such as ely-doc}
dist=${ELY_DIST:-$root/dist}
name=$(awk -F'\t' -v slug="$slug" '$1 == slug { print $2 }' "$root/packaging/apps.tsv")
[ -n "$name" ] || {
  echo "installer.sh: unknown app $slug" >&2
  exit 1
}
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
makensis=${MAKENSIS:-"/c/Program Files (x86)/NSIS/makensis.exe"}
[ -x "$makensis" ] || {
  echo "installer.sh: no makensis at $makensis; install NSIS" >&2
  exit 1
}
stage=$root/target/stage/$slug
"$root/packaging/stage.sh" "$slug" "$stage"
types=$root/target/stage/$slug-types.nsh
"$root/packaging/windows/types.sh" "$slug" >"$types"
mkdir -p "$dist"
out=$dist/$name-$version-windows-x86_64-setup.exe
"$makensis" -V2 -INPUTCHARSET UTF8 -DAPP_NAME="$name" -DAPP_SLUG="$slug" -DVERSION="$version" \
  -DSTAGE="$(cygpath -w "$stage")" -DICON="$(cygpath -w "$root/assets/icons/$slug.ico")" \
  -DTYPES="$(cygpath -w "$types")" -DOUTFILE="$(cygpath -w "$out")" \
  "$(cygpath -w "$root/packaging/windows/installer.nsi")"
echo "$out"
