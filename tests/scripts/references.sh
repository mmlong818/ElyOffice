#!/usr/bin/env bash
# Every pin in fetch-references.sh matches its checkout.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script=$root/scripts/fetch-references.sh
names="Ely-GPUI-Components zed libreoffice-core mupdf git tesseract marktext colamd magpie agent-client-protocol markion markdown-rs omarchy"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}
pin_of() { awk -v n="$1" '$1 == "fetch" && $2 == n { print $4 }' "$script"; }

set -- $names
[ "$(grep -cE '^fetch ' "$script")" -eq "$#" ] || fail "script pins $(grep -cE '^fetch ' "$script") sources, test names $#"

for name in $names; do
  pin=$(pin_of "$name")
  [ "${#pin}" -eq 40 ] || fail "$name: pin '$pin' is not a full commit"
  head=$(git -C "$root/reference/$name" rev-parse HEAD 2>&1) || fail "$name: no checkout: $head"
  [ "$head" = "$pin" ] || fail "$name: HEAD $head, pin $pin"
done

for header in \
  libreoffice-core/include/LibreOfficeKit/LibreOfficeKit.h \
  mupdf/include/mupdf/fitz.h \
  mupdf/thirdparty/freetype/include/freetype/freetype.h \
  tesseract/include/tesseract/capi.h; do
  [ -s "$root/reference/$header" ] || fail "missing reference/$header"
done

want=$(printf '%s\n' desktop/inc desktop/source/lib include/LibreOfficeKit libreofficekit officecfg/registry/data/org/openoffice/Office/UI | sort)
have=$(git -C "$root/reference/libreoffice-core" sparse-checkout list | sort)
[ "$have" = "$want" ] || fail "libreoffice-core sparse set differs: $have"

status=$(git -C "$root/reference/mupdf" submodule status --recursive)
[ -n "$status" ] || fail "mupdf lists no submodules"
bad=$(grep '^[-+U]' <<<"$status" || true)
[ -z "$bad" ] || fail "mupdf submodules off their pins: $bad"

grep -q "rev = \"$(pin_of Ely-GPUI-Components)\"" "$root/Cargo.toml" ||
  fail "the component library pin differs from the workspace's"
gpui=$(grep -E '^gpui = ' "$root/Cargo.toml")
grep -qxF "$gpui" "$root/reference/Ely-GPUI-Components/Cargo.toml" ||
  fail "the workspace and the component library pin different gpui"

echo "references: $# pins match, sparse set and submodules in place, library and gpui agree"
