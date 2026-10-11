#!/usr/bin/env bash
# Probes LOK on macOS: load, export, tile painting, thread rule.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
here=$root/tests/engines
expect=${ELY_LOK_EXPECT_TILE:-written}
# Never mount a LibreOffice image: macOS would register it.
frameworks=${ELY_LOK_DIR:-$root/engines/lo-src/instdir/LibreOffice.app/Contents/Frameworks}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
run() { SAL_FORCE_HIDPI_SCALING=1 SAL_SKIA=raster perl -e 'alarm 180; exec @ARGV' "$@"; }

case $expect in written | untouched) ;; *) fail "ELY_LOK_EXPECT_TILE is written or untouched, not $expect" ;; esac
[ "$(uname)" = Darwin ] || {
  echo "SKIP lok: macOS only"
  exit 0
}
[ -f "$frameworks/libmergedlo.dylib" ] || {
  echo "SKIP lok: no kernel at $frameworks; run scripts/build-libreoffice.sh"
  exit 0
}

exports=$(nm -gU "$frameworks/libmergedlo.dylib")
for symbol in _libreofficekit_hook _libreofficekit_hook_2 _lok_preinit; do
  grep -qE " T $symbol\$" <<<"$exports" || fail "libmergedlo does not export $symbol"
done
plugins=$(ls "$frameworks" | grep '^libvclplug_' || true)
[ "$plugins" = libvclplug_osxlo.dylib ] || fail "VCL plugins are not only osx: $plugins"

python3 - "$work/hello.docx" <<'PY'
import sys, zipfile
types = '<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'
rels = '<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'
body = '<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:b/><w:sz w:val="48"/></w:rPr><w:t>Ely Office 合同 Contract</w:t></w:r></w:p><w:p><w:r><w:t>The quick brown fox jumps over the lazy dog.</w:t></w:r></w:p></w:body></w:document>'
with zipfile.ZipFile(sys.argv[1], "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", types)
    z.writestr("_rels/.rels", rels)
    z.writestr("word/document.xml", body)
PY
clang -O1 -I "$root/reference/libreoffice-core/include" "$here/lok_probe.c" -o "$work/probe" || fail "probe does not compile"
args=("$frameworks" "file://$work/profile" "file://$work/hello.docx" "file://$work/out.pdf")

out=$(run "$work/probe" "${args[@]}" unipoll 2>"$work/err") || fail "unipoll probe: $(tail -n 5 "$work/err")"
grep -q '"BuildId": "8221e31b3ac356a1623c672912a3d2b492f7e3d1"' <<<"$out" || fail "not the pinned build: $out"
grep -qE '^twips=[1-9][0-9]*x[1-9][0-9]*$' <<<"$out" || fail "document has no size: $out"
grep -qx 'pdf=1' <<<"$out" || fail "PDF export: $out"
[ -s "$work/out.pdf" ] || fail "no PDF written"
grep -qx 'polls=0' <<<"$out" || fail "Aqua called the poll callback: $out"
case $expect in
  untouched) grep -qx 'painted=0' <<<"$out" || fail "tile painted, expected untouched: $out" ;;
  written)
    grep -qE '^painted=[1-9]' <<<"$out" || fail "tile untouched, expected painted: $out"
    grep -qE '^ink=[1-9]' <<<"$out" || fail "tile has no text pixels: $out"
    ;;
esac

if run "$work/probe" "${args[@]}" threaded >/dev/null 2>"$work/err"; then fail "threaded mode did not crash"; fi
grep -q 'NSWindow should only be instantiated on the main thread' "$work/err" || fail "threaded crash has another cause: $(head -n 3 "$work/err")"
echo "lok: load, PDF export, thread rule pass; tile $expect"
