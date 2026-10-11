#!/usr/bin/env bash
# No LibreOffice app may appear in Open With.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP lo-unregistered: macOS only"; exit 0; }
fail() { echo "FAIL: $*" >&2; exit 1; }
app=$root/engines/lo-src/instdir/LibreOffice.app
if [ -d "$app" ]; then
  plutil -lint -s "$app/Contents/Info.plist" || fail "$app has no readable Info.plist"
  for key in CFBundleDocumentTypes CFBundleURLTypes UTExportedTypeDeclarations UTImportedTypeDeclarations; do
    ! /usr/libexec/PlistBuddy -c "Print :$key" "$app/Contents/Info.plist" >/dev/null 2>&1 || fail "$app claims $key"
  done
  [ ! -e "$app/Contents/PlugIns" ] || fail "$app keeps its Quick Look plug-ins"
fi
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fake=$work/a/b/Fake.app
mkdir -p "$fake/Contents/PlugIns"
echo "not a plist" >"$fake/Contents/Info.plist"
! "$root/scripts/inert-lo.sh" "$fake" 2>/dev/null || fail "inert-lo.sh accepts an unreadable Info.plist"
plutil -create xml1 "$fake/Contents/Info.plist"
plutil -insert CFBundleDocumentTypes -array "$fake/Contents/Info.plist"
"$root/scripts/inert-lo.sh" "$fake" >/dev/null
! /usr/libexec/PlistBuddy -c "Print :CFBundleDocumentTypes" "$fake/Contents/Info.plist" >/dev/null 2>&1 || fail "inert-lo.sh left a claim"
[ ! -e "$fake/Contents/PlugIns" ] || fail "inert-lo.sh left the plug-ins"
for ext in docx xlsx pptx odt csv; do : >"$work/sample.$ext"; done
cat >"$work/open.swift" <<'SWIFT'
import AppKit
for path in CommandLine.arguments.dropFirst() {
  for app in NSWorkspace.shared.urlsForApplications(toOpen: URL(fileURLWithPath: path)) { print(app.path) }
}
SWIFT
apps=$(swift "$work/open.swift" "$work"/sample.*) || fail "the Open With query failed"
found=$(grep -i libreoffice <<<"$apps" || true)
[ -z "$found" ] || fail "Open With lists LibreOffice: $found"
echo "ok: no LibreOffice in Open With"
