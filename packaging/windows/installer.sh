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
# Explorer's New copies these; the install list owns them.
for ext in $(awk -F'\t' -v slug="$slug" '$1 == slug { print $2 }' "$root/packaging/types.tsv"); do
  [ ! -e "$root/packaging/windows/new/new.$ext" ] || cp "$root/packaging/windows/new/new.$ext" "$stage/"
done
types=$root/target/stage/$slug-types.nsh
"$root/packaging/windows/types.sh" "$slug" >"$types"
list=$root/target/stage/$slug-files.txt
"$root/packaging/windows/files.sh" "$stage" "$slug" >"$list"
mkdir -p "$dist"
out=$dist/$name-$version-windows-x86_64-setup.exe
"$makensis" -V2 -INPUTCHARSET UTF8 -DAPP_NAME="$name" -DAPP_SLUG="$slug" -DVERSION="$version" \
  -DSTAGE="$(cygpath -w "$stage")" -DICON="$(cygpath -w "$root/assets/icons/$slug.ico")" \
  -DTYPES="$(cygpath -w "$types")" -DLIST="$(cygpath -w "$list")" -DOUTFILE="$(cygpath -w "$out")" \
  "$(cygpath -w "$root/packaging/windows/installer.nsi")"
echo "$out"
