#!/usr/bin/env bash
# Linux builds carry Wayland and X11; ElyTree shows on each.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

features=$(cargo tree --locked --manifest-path "$root/Cargo.toml" -p ely-tree -e features \
  -i gpui-pre-linux --target x86_64-unknown-linux-gnu)
for backend in wayland x11; do
  grep -q "gpui-pre-linux feature \"$backend\"" <<<"$features" || fail "gpui is built without $backend"
done
[ "$(uname)" = Linux ] || {
  echo "SKIP: windows show on Linux only"
  exit 0
}
for tool in hyprctl grim magick tesseract jq git; do
  command -v "$tool" >/dev/null || {
    echo "SKIP: no $tool; the window check reads Hyprland"
    exit 0
  }
done
# The binary Cargo reports, wherever it builds.
app=$(cargo build -q --locked --message-format=json --manifest-path "$root/Cargo.toml" -p ely-tree --bin ely-tree |
  jq -r 'select(.reason == "compiler-artifact" and .target.name == "ely-tree") | .executable // empty')
[ -x "$app" ] || fail "Cargo reported no ElyTree binary"
work=$(mktemp -d)
pid=
trap '[ -z "$pid" ] || kill "$pid" 2>/dev/null; rm -rf "$work"' EXIT

show() { # backend, env args
  local backend=$1 geometry= shot=$work/$1.png
  shift
  env "$@" ELY_GIT="$(command -v git)" ELY_SMOKE=hold ELY_APPEARANCE=dark ELY_OMARCHY_SYNC=0 ELY_LOCALE=en-US \
    HOME="$work" XDG_DATA_HOME="$work/data" "$app" 2>"$work/$backend.err" &
  pid=$!
  for _ in $(seq 120); do
    geometry=$(hyprctl clients -j | jq -r --argjson pid "$pid" \
      '.[] | select(.pid == $pid and .mapped) | "\(.at[0]),\(.at[1]) \(.size[0])x\(.size[1])"')
    [ -n "$geometry" ] && break
    kill -0 "$pid" 2>/dev/null || fail "ElyTree quit on $backend: $(tail -n 5 "$work/$backend.err")"
    sleep 0.5
  done
  [ -n "$geometry" ] || fail "Hyprland never mapped ElyTree on $backend"
  sleep 2
  grim -g "$geometry" "$shot"
  kill "$pid"
  wait "$pid" 2>/dev/null || true
  pid=
  # An even #252525 page and the start page's words.
  local page words
  page=$(magick "$shot" -gravity center -crop 60%x30%+0+25% +repage -format \
    '%[fx:max(abs(mean.r-0x25/255),max(abs(mean.g-0x25/255),abs(mean.b-0x25/255)))*255] %[fx:standard_deviation*255]' info:)
  awk -v off="${page% *}" -v spread="${page#* }" 'BEGIN { exit !(off < 16 && spread < 6) }' ||
    fail "ElyTree on $backend shows no even page (off by, spread: $page)"
  words=$(tesseract "$shot" - 2>"$work/ocr.err") || fail "tesseract failed: $(cat "$work/ocr.err")"
  grep -q 'Open a repository' <<<"$words" || fail "ElyTree on $backend shows no 'Open a repository': $words"
  echo "linux: ElyTree shows on $backend"
}
ran=
if [ -n "${WAYLAND_DISPLAY:-}" ]; then
  show wayland
  ran=1
fi
if [ -n "${DISPLAY:-}" ]; then
  show x11 -u WAYLAND_DISPLAY
  ran=1
fi
[ -n "$ran" ] || echo "SKIP: no WAYLAND_DISPLAY or DISPLAY"
