#!/usr/bin/env bash
# Prints NSIS macros registering <slug>'s types, never as default.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
slug=${1:?app slug, such as ely-doc}
name=$(awk -F'\t' -v slug="$slug" '$1 == slug { print $2 }' "$root/packaging/apps.tsv")
[ -n "$name" ] || { echo "types.sh: unknown app $slug" >&2; exit 1; }
progid="ElyOffice.$name"
classes='Software\Classes'
exts=$(awk -F'\t' -v slug="$slug" '$1 == slug && $2 != "/" { print $2 }' "$root/packaging/types.tsv")
folder=$(awk -F'\t' -v slug="$slug" '$1 == slug && $2 == "/" { print "yes" }' "$root/packaging/types.tsv")
# Types Explorer's New menu makes, from packaging/windows/new.
fresh=""
for ext in $exts; do
  [ ! -e "$root/packaging/windows/new/new.$ext" ] || fresh="$fresh $ext"
done

# Open With lists the app; the user's default stays theirs.
echo '!macro ElyTypesInstall'
if [ -n "$exts" ]; then
  echo "  WriteRegStr HKCU \"$classes\\$progid\" \"\" \"$name Document\""
  echo "  WriteRegStr HKCU \"$classes\\$progid\\DefaultIcon\" \"\" \"\$INSTDIR\\$slug.ico\""
  echo "  WriteRegStr HKCU \"$classes\\$progid\\shell\\open\\command\" \"\" '\"\$INSTDIR\\$slug.exe\" \"%1\"'"
  echo "  WriteRegStr HKCU \"$classes\\Applications\\$slug.exe\" \"FriendlyAppName\" \"$name\""
  for ext in $exts; do
    echo "  WriteRegStr HKCU \"$classes\\.$ext\\OpenWithProgids\" \"$progid\" \"\""
    echo "  WriteRegStr HKCU \"$classes\\Applications\\$slug.exe\\SupportedTypes\" \".$ext\" \"\""
  done
fi
for ext in $fresh; do
  echo "  WriteRegStr HKCU \"$classes\\.$ext\\ShellNew\" \"FileName\" \"\$INSTDIR\\new.$ext\""
  # New needs a class; take only a handlerless type.
  echo "  ReadRegStr \$0 HKCR \".$ext\" \"\""
  echo "  ClearErrors"
  # A default naming no class handles nothing either.
  echo "  \${If} \$0 != \"\""
  echo "    EnumRegKey \$1 HKCR \"\$0\" 0"
  echo "  \${EndIf}"
  echo "  \${If} \$0 == \"\""
  echo "  \${OrIf} \${Errors}"
  echo "    WriteRegStr HKCU \"$classes\\.$ext\" \"\" \"$progid\""
  echo "  \${EndIf}"
done
if [ -n "$folder" ]; then
  echo "  WriteRegStr HKCU \"$classes\\Directory\\shell\\$progid\" \"\" \"Open in $name\""
  echo "  WriteRegStr HKCU \"$classes\\Directory\\shell\\$progid\" \"Icon\" \"\$INSTDIR\\$slug.ico\""
  echo "  WriteRegStr HKCU \"$classes\\Directory\\shell\\$progid\\command\" \"\" '\"\$INSTDIR\\$slug.exe\" \"%1\"'"
fi
echo '  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"'
echo '!macroend'

echo '!macro ElyTypesUninstall'
if [ -n "$exts" ]; then
  echo "  DeleteRegKey HKCU \"$classes\\$progid\""
  echo "  DeleteRegKey HKCU \"$classes\\Applications\\$slug.exe\""
  for ext in $exts; do
    echo "  DeleteRegValue HKCU \"$classes\\.$ext\\OpenWithProgids\" \"$progid\""
  done
fi
# Only what this install wrote.
for ext in $fresh; do
  echo "  ReadRegStr \$0 HKCU \"$classes\\.$ext\\ShellNew\" \"FileName\""
  echo "  \${If} \$0 == \"\$INSTDIR\\new.$ext\""
  echo "    DeleteRegKey HKCU \"$classes\\.$ext\\ShellNew\""
  echo "  \${EndIf}"
  echo "  ReadRegStr \$0 HKCU \"$classes\\.$ext\" \"\""
  echo "  \${If} \$0 == \"$progid\""
  echo "    DeleteRegValue HKCU \"$classes\\.$ext\" \"\""
  echo "  \${EndIf}"
done
if [ -n "$folder" ]; then
  echo "  DeleteRegKey HKCU \"$classes\\Directory\\shell\\$progid\""
fi
echo '  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"'
echo '!macroend'
