#!/usr/bin/env bash
# The built LibreOffice bundle holds libraries, nothing to register.
set -euo pipefail
app=${1:?usage: scripts/inert-lo.sh path/to/LibreOffice.app}
plist=$app/Contents/Info.plist
plutil -lint -s "$plist" || { echo "inert-lo.sh: $plist is missing or unreadable" >&2; exit 1; }
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
for key in CFBundleDocumentTypes CFBundleURLTypes UTExportedTypeDeclarations UTImportedTypeDeclarations; do
  if /usr/libexec/PlistBuddy -c "Print :$key" "$plist" >/dev/null 2>&1; then
    /usr/libexec/PlistBuddy -c "Delete :$key" "$plist"
  fi
done
rm -rf "$app/Contents/PlugIns" "$app/Contents/Library/Spotlight"
# -10814: not registered, which is the goal.
if ! said=$("$lsregister" -u "$app" 2>&1); then
  grep -q -- "-10814" <<<"$said" || { echo "inert-lo.sh: $said" >&2; exit 1; }
fi
# Spotlight would find the bundle and register it again.
touch "$(cd "$app/../../.." && pwd)/.metadata_never_index"
echo "inert-lo.sh: $app claims no files and is unregistered"
