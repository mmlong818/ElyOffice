#!/usr/bin/env bash
# Idle on Linux: ElyMark rereads no theme, settings or file.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
[ "$(uname)" = Linux ] || {
  echo "SKIP: inotify is Linux"
  exit 0
}
[ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ] || {
  echo "SKIP: no display"
  exit 0
}
for tool in inotifywait jq hyprctl grim magick tesseract; do
  command -v "$tool" >/dev/null || {
    echo "SKIP: no $tool"
    exit 0
  }
done
[ -f "$root/engines/cef/libcef.so" ] || fail "no engines/cef; run scripts/fetch-cef.sh"
[ -f "$root/engines/content/ely-mark/index.html" ] || fail "no ElyMark page; run scripts/build-mark-content.sh"
app=$(cargo build -q --locked --message-format=json --manifest-path "$root/Cargo.toml" -p ely-mark --bin ely-mark |
  jq -r 'select(.reason == "compiler-artifact" and .target.name == "ely-mark") | .executable // empty')
[ -x "$app" ] || fail "Cargo reported no ElyMark binary"

work=$(mktemp -d)
pid=
spy=
trap '[ -z "$spy" ] || kill "$spy" 2>/dev/null; [ -z "$pid" ] || stop_app "$pid"; rm -rf "$work"' EXIT
# shellcheck source=tests/scripts/linux-common.sh
. "$root/tests/scripts/linux-common.sh"
current=$work/omarchy/current
mkdir -p "$current/theme" "$work/data/ElyOffice" "$work/notes"
cp "$root/crates/ely-ui/tests/fixtures/omarchy/catppuccin.toml" "$current/theme/colors.toml"
echo catppuccin >"$current/theme.name"
echo '{"version": 1}' >"$work/data/ElyOffice/settings.json"
note=$work/notes/note.md
printf '# Idle\n\nNothing moves.\n' >"$note"

LD_LIBRARY_PATH="$root/engines/cef" ELY_OMARCHY_DIR="$current" ELY_SMOKE=hold ELY_LOG=info HOME="$work" \
  XDG_DATA_HOME="$work/data" setsid "$app" "$note" 2>"$work/app.err" &
pid=$!
seen() { grep -c "$1" "$work/app.err" || true; }
logged() { # pattern, what, count before
  local before=${3:-0}
  for _ in $(seq 120); do
    [ "$(seen "$1")" -gt "$before" ] && return
    kill -0 "$pid" 2>/dev/null || fail "ElyMark quit: $(tail -n 5 "$work/app.err")"
    sleep 0.25
  done
  fail "ElyMark missed $2"
}
logged 'mark: watching 1 folders' "its file's folder"
logged 'omarchy catppuccin applied' "the Omarchy theme"
sleep 3

# Four quiet seconds: nothing opens the theme, settings or note.
idle() { # when
  inotifywait -q -m -e open --format '%w%f' "$current" "$current/theme" "$work/data/ElyOffice" "$work/notes" \
    >"$work/opens" 2>/dev/null &
  spy=$!
  for _ in 1 2 3 4 5; do
    sleep 1
    kill -0 "$pid" 2>/dev/null || fail "ElyMark quit while $1: $(tail -n 5 "$work/app.err")"
  done
  kill "$spy"
  spy=
  local opens
  opens=$(grep -c -E '(colors\.toml|theme\.name|settings\.json|note\.md)$' "$work/opens" || true)
  [ "$opens" -eq 0 ] || fail "ElyMark $1 opened files $opens times: $(sort "$work/opens" | uniq -c)"
  echo "linux: ElyMark $1 reads nothing"
}
idle "idle after start"

mkdir "$current/next-theme"
cp "$root/crates/ely-ui/tests/fixtures/omarchy/gruvbox.toml" "$current/next-theme/colors.toml"
rm -rf "$current/theme"
mv "$current/next-theme" "$current/theme"
echo gruvbox >"$current/theme.name"
logged 'omarchy gruvbox applied' "Omarchy's theme swap"
echo '{"version": 1, "author": "Tester"}' >"$work/data/ElyOffice/settings.json"
logged 'settings: changed on disk' "a settings write"
changed=$(seen 'changed outside')
printf '# Idle\n\nSomething moved.\n' >"$note"
logged 'changed outside' "an outside edit" "$changed"
# The window shows the new words.
shown=
for _ in $(seq 20); do
  kill -0 "$pid" 2>/dev/null || fail "ElyMark quit: $(tail -n 5 "$work/app.err")"
  id=$(hyprctl clients -j | jq -r --argjson pid "$pid" '[.[] | select(.pid == $pid and .mapped)][0].stableId // empty')
  if [ -n "$id" ]; then
    grim -T "$id" "$work/shot.png" 2>"$work/grim.err" || fail "grim failed: $(cat "$work/grim.err")"
    magick "$work/shot.png" -colorspace Gray -negate "$work/ocr.png"
    text=$(tesseract "$work/ocr.png" - 2>"$work/ocr.err") || fail "tesseract failed: $(cat "$work/ocr.err")"
    grep -q 'moved' <<<"$text" && shown=1 && break
  fi
  sleep 0.5
done
[ -n "$shown" ] || fail "ElyMark does not show the outside edit"
sleep 2
idle "idle after changes"
