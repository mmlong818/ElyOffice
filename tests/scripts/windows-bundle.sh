#!/usr/bin/env bash
# The bundle runs each setup; a refusal stops it.
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
}
trap 'kill $(jobs -p) 2>/dev/null || true; clean' EXIT
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

run() { "$work/bundle.exe" /S && echo 0 || echo $?; }
[ "$(run)" = 0 ] || fail "the bundle failed"
for name in $names; do
  [ -f "$work/app-$name/.ely-installed" ] || fail "$name has no install list"
  reg query "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Ely Office $name" >/dev/null 2>&1 ||
    fail "$name has no uninstall entry"
done

# The second app is refused; the third never runs.
rm -rf "$work/app-ElyProbeThree" "$work/held"
held=$(cygpath -w "$work/app-ElyProbeTwo/ely-probe-two.exe")
powershell -NoProfile -Command "\$f=[IO.File]::Open('$held','Open','Read','Read'); \
  New-Item '$(cygpath -w "$work/held")' | Out-Null; Start-Sleep 60" &
until [ -e "$work/held" ]; do sleep 0.2; done
result=$(run)
kill %% 2>/dev/null; wait 2>/dev/null || true
[ "$result" = 2 ] || fail "the bundle exited $result over a refused app"
grep -q ElyProbeTwo "$work/app-ElyProbeTwo/ely-probe-two.exe" || fail "the refused app changed"
[ -f "$work/app-ElyProbe/.ely-installed" ] || fail "the first app is gone"
[ ! -e "$work/app-ElyProbeThree" ] || fail "the bundle went on after a refusal"
rm -rf "$work"
echo "PASS: windows bundle"
