#!/usr/bin/env bash
# Writes packaging/windows/new: the blank files Explorer's New makes.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
soffice=$root/engines/lo-src/instdir/program/soffice.com
[ -x "$soffice" ] || {
  echo "templates.sh: no kernel at $soffice; run scripts/build-libreoffice.sh" >&2
  exit 1
}
out=$root/packaging/windows/new
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/in"
# The kernel reads an empty file as a new document.
for ext in docx xlsx pptx; do
  : >"$work/in/new.$ext"
  "$soffice" --headless --norestore "-env:UserInstallation=file:///$(cygpath -m "$work/profile")" \
    --convert-to "$ext" --outdir "$(cygpath -w "$out")" "$(cygpath -w "$work/in/new.$ext")"
  [ -s "$out/new.$ext" ] || {
    echo "templates.sh: the kernel wrote no new.$ext" >&2
    exit 1
  }
done
: >"$out/new.md"
