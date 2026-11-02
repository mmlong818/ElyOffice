#!/usr/bin/env bash
# Installer upgrades remove only owned files; busy files change nothing.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
makensis=${MAKENSIS:-"/c/Program Files (x86)/NSIS/makensis.exe"}
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) ;; *) echo "SKIP: not Windows"; exit 0 ;; esac
[ -x "$makensis" ] || { echo "SKIP: no makensis"; exit 0; }
export MSYS2_ARG_CONV_EXCL='*'

work=$root/target/installer-test
key='HKCU\Software\Ely Office\ElyProbe'
uninstall_key='HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Ely Office ElyProbe'
clean() {
  reg delete "$key" /f >/dev/null 2>&1 || true
  reg delete "$uninstall_key" /f >/dev/null 2>&1 || true
  rm -f "$APPDATA/Microsoft/Windows/Start Menu/Programs/ElyProbe.lnk"
}
trap 'kill $(jobs -p) 2>/dev/null || true; clean' EXIT
clean
rm -rf "$work"
mkdir -p "$work"
fail() { echo "FAIL: $*" >&2; exit 1; }

# Two fake releases; v2 adds extra.txt and zz.
build() {
  local version=$1 stage=$work/stage-$1
  mkdir -p "$stage/licenses" "$stage/data" "$stage/bin"
  cp /c/Windows/System32/PING.EXE "$stage/ely-probe.exe"
  cp /c/Windows/System32/PING.EXE "$stage/bin/helper.exe"
  echo "$version" >"$stage/ely-probe-worker.exe"
  echo "$version" >"$stage/licenses/LICENSE"
  echo "$version" >"$stage/data/info.txt"
  [ "$version" = 1 ] || { echo "$version" >"$stage/extra.txt"; mkdir -p "$stage/zz"; echo 2 >"$stage/zz/deep.txt"; }
  printf '!macro ElyTypesInstall\n!macroend\n!macro ElyTypesUninstall\n!macroend\n' >"$work/types.nsh"
  "$root/packaging/windows/files.sh" "$stage" ely-probe >"$work/files-$version.txt"
  "$makensis" -V1 -INPUTCHARSET UTF8 -DAPP_NAME=ElyProbe -DAPP_SLUG=ely-probe -DVERSION="$version" \
    -DSTAGE="$(cygpath -w "$stage")" -DICON="$(cygpath -w "$root/assets/icons/ely-mark.ico")" \
    -DTYPES="$(cygpath -w "$work/types.nsh")" -DLIST="$(cygpath -w "$work/files-$version.txt")" \
    -DOUTFILE="$(cygpath -w "$work/setup-$version.exe")" "$(cygpath -w "$root/packaging/windows/installer.nsi")"
}
build 1
build 2

install() { "$work/setup-$1.exe" /S "/D=$(cygpath -w "$2")" && echo 0 || echo $?; }
snapshot() { (cd "$1" && find . -type f -exec md5sum {} + | sort); }
put() { mkdir -p "$(dirname "$1")" && echo user >"$1"; }
must() { [ -e "$1" ] || fail "$2: $1 is gone"; }
mustnt() { [ ! -e "$1" ] || fail "$2: $1 is still there"; }
# Holds a file open; sharing defaults to read only.
hold() {
  rm -f "$work/held"
  powershell -NoProfile -Command "\$f=[IO.File]::Open('$(cygpath -w "$1")','Open','Read','${2:-Read}'); \
    New-Item '$(cygpath -w "$work/held")' | Out-Null; Start-Sleep 60" &
  until [ -e "$work/held" ]; do sleep 0.2; done
}

# A chosen folder's own files survive install and reinstall.
dir=$work/stranger
own="keep.exe keep.dll keep.json git/keep.txt notes/note.md licenses/my-notes.md"
for f in $own; do put "$dir/$f"; done
[ "$(install 1 "$dir")" = 0 ] || fail "stranger install"
[ "$(install 2 "$dir")" = 0 ] || fail "stranger reinstall"
for f in $own; do must "$dir/$f" stranger; done
must "$dir/extra.txt" "stranger reinstall"
grep -q '^2$' "$dir/licenses/LICENSE" || fail "stranger reinstall kept the old licence"

# Older releases are cleaned only where our key points.
legacy() {
  for f in ely-probe.exe libcef.dll chrome_100.pak old.json git/cmd/git.exe locales/en.pak \
    notes/note.md keep.md; do put "$1/$f"; done
}
# Another install's file in the way: refused, nothing changes.
untouched() {
  local name=$1 dir=$2 before
  before=$(snapshot "$dir")
  [ "$(install 2 "$dir")" = 2 ] || fail "$name: installer did not refuse"
  [ "$(snapshot "$dir")" = "$before" ] || fail "$name: the folder changed"
  mustnt "$dir/.ely-old" "$name"
}
dir=$work/unkeyed
legacy "$dir"
untouched "unkeyed folder" "$dir"
dir=$work/taken
[ "$(install 1 "$dir")" = 0 ] || fail "taken setup"
put "$dir/extra.txt"
untouched "user file in the way" "$dir"
dir=$work/legacy
legacy "$dir"
reg add "$key" /v InstallDir /d "$(cygpath -w "$dir")" /f >/dev/null
[ "$(install 2 "$dir")" = 0 ] || fail "legacy upgrade"
for f in libcef.dll chrome_100.pak old.json git locales; do mustnt "$dir/$f" "legacy upgrade"; done
for f in notes/note.md keep.md extra.txt; do must "$dir/$f" "legacy upgrade"; done

# Busy files: the installer refuses; the old install stays whole.
busy() {
  local name=$1 dir=$2 file=$3 before
  before=$(snapshot "$dir")
  hold "$file"
  [ "$(install 2 "$dir")" = 2 ] || fail "$name: installer did not refuse"
  kill %% 2>/dev/null; wait 2>/dev/null || true
  [ "$(snapshot "$dir")" = "$before" ] || fail "$name: the old install changed"
  mustnt "$dir/.ely-old" "$name"
}
dir=$work/busy
[ "$(install 1 "$dir")" = 0 ] || fail "busy setup"
busy "held program" "$dir" "$dir/ely-probe-worker.exe"
busy "held file in a folder" "$dir" "$dir/licenses/LICENSE"
dir=$work/busy-legacy
legacy "$dir"
reg add "$key" /v InstallDir /d "$(cygpath -w "$dir")" /f >/dev/null
busy "held legacy git" "$dir" "$dir/git/cmd/git.exe"

running() {
  local name=$1 dir=$2 program=$3 before
  before=$(snapshot "$dir")
  "$program" -n 30 127.0.0.1 >/dev/null &
  sleep 1
  [ "$(install 2 "$dir")" = 2 ] || fail "$name: installer did not refuse"
  kill %% 2>/dev/null; wait 2>/dev/null || true
  [ "$(snapshot "$dir")" = "$before" ] || fail "$name: the old install changed"
  mustnt "$dir/.ely-old" "$name"
}
dir=$work/running
[ "$(install 1 "$dir")" = 0 ] || fail "running setup"
running "running program" "$dir" "$dir/ely-probe.exe"
running "running program in a folder" "$dir" "$dir/bin/helper.exe"
dir=$work/running-legacy
legacy "$dir"
cp /c/Windows/System32/PING.EXE "$dir/git/cmd/git.exe"
reg add "$key" /v InstallDir /d "$(cygpath -w "$dir")" /f >/dev/null
running "running legacy git" "$dir" "$dir/git/cmd/git.exe"

# A late copy failure puts the old release back.
dir=$work/rollback
[ "$(install 1 "$dir")" = 0 ] || fail "rollback setup"
put "$dir/zz/user.txt"
icacls "$(cygpath -w "$dir/zz")" /deny '*S-1-1-0:(WD)' >/dev/null
before=$(snapshot "$dir")
result=$(install 2 "$dir")
icacls "$(cygpath -w "$dir/zz")" /remove:d '*S-1-1-0' >/dev/null
[ "$result" != 0 ] || fail "rollback: blocked copy succeeded"
[ "$(snapshot "$dir")" = "$before" ] || fail "rollback: the old install changed"
mustnt "$dir/.ely-old" rollback

# Paths near the limit upgrade and uninstall too.
long=$(cygpath -w "$work/long")
dir=$work/long/$(printf "%$((233 - ${#long} - 1))s" | tr ' ' a)
[ "$(install 1 "$dir")" = 0 ] || fail "long path install"
worker=$(cygpath -w "$dir/ely-probe-worker.exe")
[ ${#worker} = 254 ] || fail "long path is ${#worker} long"
[ "$(install 2 "$dir")" = 0 ] || fail "long path upgrade"
must "$dir/extra.txt" "long path upgrade"
cp "$dir/uninstall.exe" "$work/uninstall.exe"
"$work/uninstall.exe" /S "_?=$(cygpath -w "$dir")" || fail "long path uninstall"
mustnt "$dir/ely-probe-worker.exe" "long path uninstall"

# A held install list stops uninstall; nothing changes.
unheld() {
  local name=$1 dir=$2 share=$3 before result
  before=$(snapshot "$dir")
  hold "$dir/.ely-installed" "$share"
  "$work/uninstall.exe" /S "_?=$(cygpath -w "$dir")" && result=0 || result=$?
  kill %% 2>/dev/null; wait 2>/dev/null || true
  [ "$result" = 2 ] || fail "$name: uninstall did not refuse"
  [ "$(snapshot "$dir")" = "$before" ] || fail "$name: the install changed"
  mustnt "$dir/.ely-old" "$name"
}
dir=$work/held-list
[ "$(install 1 "$dir")" = 0 ] || fail "held list setup"
cp "$dir/uninstall.exe" "$work/uninstall.exe"
unheld "list held for reading" "$dir" Read
unheld "list held against deletion" "$dir" ReadWrite

# Uninstalling removes only what was installed.
dir=$work/stranger
cp "$dir/uninstall.exe" "$work/uninstall.exe"
"$work/uninstall.exe" /S "_?=$(cygpath -w "$dir")" || fail "uninstall"
for f in $own; do must "$dir/$f" uninstall; done
for f in ely-probe.exe ely-probe-worker.exe extra.txt licenses/LICENSE data bin zz uninstall.exe .ely-installed; do
  mustnt "$dir/$f" uninstall
done
# An in-app update runs the next setup over it (#10).
dir="$work/Ely Office/ElyProbe"
[ "$(install 1 "$dir")" = 0 ] || fail "update: setup 1"
ELY_TEST_SETUP=$(cygpath -w "$work/setup-2.exe") ELY_TEST_DIR=$(cygpath -w "$dir")   cargo test -q -p ely-platform --test install a_real_setup_installs_over_its_folder -- --exact >"$work/update.log" 2>&1 ||
  { cat "$work/update.log" >&2; fail "update: the after-quit step"; }
grep -q "1 passed" "$work/update.log" || fail "update: the test did not run"
must "$dir/extra.txt" "update"
must "$dir/zz/deep.txt" "update"
[ "$(cat "$dir/ely-probe-worker.exe")" = 2 ] || fail "update: version 1 stays"
mustnt "$work/Ely Office/ElyProbe/.ely-old" "update"
rm -rf "$work"
echo "PASS: windows installer"
