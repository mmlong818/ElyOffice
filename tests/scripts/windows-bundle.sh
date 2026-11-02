#!/usr/bin/env bash
# A refusal stops the setups; new apps go where asked.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
makensis=${MAKENSIS:-"/c/Program Files (x86)/NSIS/makensis.exe"}
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) ;; *) echo "SKIP: not Windows"; exit 0 ;; esac
[ -x "$makensis" ] || { echo "SKIP: no makensis"; exit 0; }
export MSYS2_ARG_CONV_EXCL='*'

work=$root/target/bundle-test
names="ElyProbe ElyProbeTwo ElyProbeThree"
clean() {
  for name in $names; do
    reg delete "HKCU\\Software\\Ely Office\\$name" /f >/dev/null 2>&1 || true
    reg delete "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Ely Office $name" /f >/dev/null 2>&1 || true
    rm -f "$APPDATA/Microsoft/Windows/Start Menu/Programs/$name.lnk"
  done
  # Where a misread /D= would put the new app.
  rm -rf "$LOCALAPPDATA/Programs/Ely Office/ElyProbeThree"
}
# The user's own remembered folder comes back afterwards.
kept=$(reg query "HKCU\Software\Ely Office" /v Root 2>/dev/null | awk '/Root/ { sub(/.*REG_SZ +/, ""); print }' || true)
restore() {
  if [ -n "$kept" ]; then
    reg add "HKCU\Software\Ely Office" /v Root /d "$kept" /f >/dev/null
  else
    reg delete "HKCU\Software\Ely Office" /v Root /f >/dev/null 2>&1 || true
  fi
}
trap 'kill $(jobs -p) 2>/dev/null || true; clean; restore' EXIT
clean
rm -rf "$work"
mkdir -p "$work"
fail() { echo "FAIL: $*" >&2; exit 1; }

printf '!macro ElyTypesInstall\n!macroend\n!macro ElyTypesUninstall\n!macroend\n' >"$work/types.nsh"
: >"$work/apps.nsh"
for name in $names; do
  slug=$(echo "$name" | sed 's/\([A-Z]\)/-\L\1/g; s/^-//')
  stage=$work/stage-$name
  mkdir -p "$stage"
  echo "$name" >"$stage/$slug.exe"
  "$root/packaging/windows/files.sh" "$stage" "$slug" >"$work/files-$name.txt"
  "$makensis" -V1 -INPUTCHARSET UTF8 -DAPP_NAME="$name" -DAPP_SLUG="$slug" -DVERSION=1 \
    -DSTAGE="$(cygpath -w "$stage")" -DICON="$(cygpath -w "$root/assets/icons/ely-mark.ico")" \
    -DTYPES="$(cygpath -w "$work/types.nsh")" -DLIST="$(cygpath -w "$work/files-$name.txt")" \
    -DOUTFILE="$(cygpath -w "$work/$name-setup.exe")" "$(cygpath -w "$root/packaging/windows/installer.nsi")"
  printf '!insertmacro ElyApp "%s" "%s"\n' "$name" "$(cygpath -w "$work/$name-setup.exe")" >>"$work/apps.nsh"
  # Each setup installs where its own key points.
  reg add "HKCU\\Software\\Ely Office\\$name" /v InstallDir /d "$(cygpath -w "$work/app-$name")" /f >/dev/null
done
"$makensis" -V1 -INPUTCHARSET UTF8 -DVERSION=1 -DAPPS="$(cygpath -w "$work/apps.nsh")" \
  -DOUTFILE="$(cygpath -w "$work/bundle.exe")" "$(cygpath -w "$root/packaging/windows/bundle.nsi")"

# NSIS wants /D= last and unquoted, spaces and all.
chosen="$work/O'Neil chosen dir"
run() {
  # Paths go through the environment: no quoting to escape.
  ELY_SETUP=$(cygpath -w "$work/bundle.exe") ELY_CHOSEN=$(cygpath -w "$chosen") powershell -NoProfile -Command \
    'exit (Start-Process -FilePath $env:ELY_SETUP -ArgumentList "/S /D=$env:ELY_CHOSEN" -Wait -PassThru).ExitCode' &&
    echo 0 || echo $?
}
# New: no key, so it goes in the chosen folder.
reg delete "HKCU\Software\Ely Office\ElyProbeThree" /f >/dev/null
[ "$(run)" = 0 ] || fail "the bundle failed"
for name in ElyProbe ElyProbeTwo; do
  [ -f "$work/app-$name/.ely-installed" ] || fail "$name did not update in place"
done
[ -f "$chosen/ElyProbeThree/.ely-installed" ] || fail "the new app is not in the chosen folder"
[ ! -e "$work/app-ElyProbeThree" ] || fail "the new app went elsewhere"
for name in $names; do
  reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Ely Office $name" >/dev/null 2>&1 ||
    fail "$name has no uninstall entry"
done
reg query "HKCU\Software\Ely Office" /v Root | grep -qF "$(cygpath -w "$chosen")" ||
  fail "the chosen folder is not remembered"
# Back to its own key, for the refusal below.
rm -rf "$chosen"
reg add "HKCU\Software\Ely Office\ElyProbeThree" /v InstallDir /d "$(cygpath -w "$work/app-ElyProbeThree")" /f >/dev/null

# The second app is refused; the third never runs.
rm -rf "$work/app-ElyProbeThree" "$work/held"
ELY_HELD=$(cygpath -w "$work/app-ElyProbeTwo/ely-probe-two.exe") ELY_MARK=$(cygpath -w "$work/held") \
  powershell -NoProfile -Command \
  '$f=[IO.File]::Open($env:ELY_HELD,"Open","Read","Read"); New-Item $env:ELY_MARK | Out-Null; Start-Sleep 60' &
holder=$!
until [ -e "$work/held" ]; do
  kill -0 "$holder" 2>/dev/null || fail "the file lock did not take"
  sleep 0.2
done
result=$(run)
kill %% 2>/dev/null; wait 2>/dev/null || true
[ "$result" = 2 ] || fail "the bundle exited $result over a refused app"
grep -q ElyProbeTwo "$work/app-ElyProbeTwo/ely-probe-two.exe" || fail "the refused app changed"
[ -f "$work/app-ElyProbe/.ely-installed" ] || fail "the first app is gone"
[ ! -e "$work/app-ElyProbeThree" ] || fail "the bundle went on after a refusal"
rm -rf "$work"
echo "PASS: windows bundle"
