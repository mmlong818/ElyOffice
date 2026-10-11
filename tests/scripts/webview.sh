#!/usr/bin/env bash
# A live WebView: input, fence, failures, release, paint.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP: macOS only"; exit 0; }
[ -d "${ELY_CEF:-$root/engines/cef}/Chromium Embedded Framework.framework" ] || {
  echo "SKIP: no CEF; run scripts/fetch-cef.sh"
  exit 0
}
work=$(mktemp -d)
probe=
trap '[ -n "$probe" ] && kill "$probe" 2>/dev/null; rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

cargo build -q -p ely-web-content --example webprobe --manifest-path "$root/Cargo.toml"
swiftc -O -o "$work/share" "$root/tests/scripts/window-share.swift"
swiftc -O -o "$work/press" "$root/tests/scripts/ax-press.swift"
ELY_LOG=info ELY_LOCALE=en ELY_SMOKE=hold HOME="$work" XDG_DATA_HOME="$work/data" \
  perl -e 'alarm 150; exec @ARGV' "$root/target/debug/examples/webprobe" >"$work/out" 2>"$work/err" &
probe=$!
wait_for() {
  for _ in $(seq 400); do
    grep -qxF "$1" "$work/out" && return 0
    kill -0 "$probe" 2>/dev/null || break
    sleep 0.25
  done
  fail "never saw '$1': $(cat "$work/out") $(tail -n 5 "$work/err")"
}
wait_for "probe: hold"
[ "$("$work/press" $$ none)" != untrusted ] || fail "needs Accessibility permission"
# The page's magenta marker: share of the window.
marker() {
  local share
  share=$("$work/share" "$probe" 255,0,170)
  case $share in
    denied) fail "needs Screen Recording permission" ;;
    none) fail "the probe has no window" ;;
  esac
  echo "$share"
}
share=$(marker)
awk -v s="$share" 'BEGIN { exit !(s > 0.01) }' || fail "the page is not painted (marker share $share)"
# A killed renderer: the fault shows, its button reloads.
pkill -P "$probe" -f -- "--type=renderer" || fail "no renderer to stop"
wait_for "probe: main failed"
sleep 1
gone=$(marker)
awk -v s="$gone" 'BEGIN { exit !(s < 0.001) }' || fail "the dead page still shows (marker share $gone)"
pressed=
for _ in $(seq 20); do
  [ "$("$work/press" "$probe" Reload)" = pressed ] && pressed=1 && break
  sleep 0.25
done
[ -n "$pressed" ] || fail "no Reload button in the fault view"
wait_for "probe: recovered"
sleep 1.5
back=$(marker)
awk -v s="$back" 'BEGIN { exit !(s > 0.01) }' || fail "the page did not come back (marker share $back)"
wait "$probe" || fail "the probe did not exit cleanly: $(tail -n 5 "$work/err")"
probe=
out=$(cat "$work/out")
for want in "probe: value=你好a" "probe: focused=text" "probe: zoom=1.25" "probe: stayed" \
  "probe: outside=false" "probe: network=refused" "probe: udp=silent" "probe: rtc=undefined" "probe: main failed" "probe: hash loaded" "probe: missing failed" \
  "probe: closed 20" "probe: dropped 5 unmade, 10 early" "probe: inert loaded" "probe: scripted ran" "probe: inert checked"; do
  grep -qxF "$want" <<<"$out" || fail "expected '$want', got: $out"
done
! grep -q "probe: inert ran" <<<"$out" || fail "an inert page ran a script: $out"
grep -qE '^probe: fg=rgba\([0-9]+, [0-9]+, [0-9]+, ' <<<"$out" || fail "no theme token: $out"
# Freed before the hold: 20 closed, 5 unmade, 10 early.
freed=$(awk "/probe: hold/ { exit } /a page's memory is freed/ { n++ } END { print n + 0 }" "$work/err")
[ "$freed" -ge 35 ] || fail "only $freed of 35 closed pages freed their memory"
! grep -q "panicked" "$work/err" || fail "panics: $(grep -m3 panicked "$work/err")"
echo "ok: webview (marker share $share, $freed pages freed)"
