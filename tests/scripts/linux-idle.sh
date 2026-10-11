#!/usr/bin/env bash
# Idle on Linux: ElyTree rereads neither theme, settings nor repo.
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
for tool in inotifywait jq git; do
  command -v "$tool" >/dev/null || {
    echo "SKIP: no $tool"
    exit 0
  }
done
app=$(cargo build -q --locked --message-format=json --manifest-path "$root/Cargo.toml" -p ely-tree --bin ely-tree |
  jq -r 'select(.reason == "compiler-artifact" and .target.name == "ely-tree") | .executable // empty')
[ -x "$app" ] || fail "Cargo reported no ElyTree binary"

work=$(mktemp -d)
pid=
spy=
trap '[ -z "$pid" ] || kill "$pid" 2>/dev/null; [ -z "$spy" ] || kill "$spy" 2>/dev/null; rm -rf "$work"' EXIT
current=$work/omarchy/current
mkdir -p "$current/theme" "$work/data/ElyOffice" "$work/bin"
cp "$root/crates/ely-ui/tests/fixtures/omarchy/catppuccin.toml" "$current/theme/colors.toml"
echo catppuccin >"$current/theme.name"
echo '{"version": 1}' >"$work/data/ElyOffice/settings.json"
repo=$work/repo
git init -q "$repo"
echo one >"$repo/a.txt"
git -C "$repo" add a.txt
git -C "$repo" -c user.name=t -c user.email=t@t commit -q -m one
real=$(command -v git)
printf '#!/bin/sh\necho "$*" >>"%s"\nexec "%s" "$@"\n' "$work/git.log" "$real" >"$work/bin/git"
chmod +x "$work/bin/git"

ELY_GIT="$work/bin/git" ELY_OMARCHY_DIR="$current" ELY_SMOKE=hold ELY_LOG=info HOME="$work" \
  XDG_DATA_HOME="$work/data" "$app" "$repo" 2>"$work/app.err" &
pid=$!
statuses() { if [ -f "$work/git.log" ]; then grep -c '^status' "$work/git.log" || true; else echo 0; fi; }
for _ in $(seq 120); do
  [ "$(statuses)" -gt 0 ] && break
  kill -0 "$pid" 2>/dev/null || fail "ElyTree quit: $(tail -n 5 "$work/app.err")"
  sleep 0.5
done
[ "$(statuses)" -gt 0 ] || fail "ElyTree never read the repo"
grep -q 'omarchy catppuccin applied' "$work/app.err" || fail "the Omarchy theme was not applied"
sleep 3

# Four quiet seconds: no reads, no git status.
idle() { # when
  inotifywait -q -m -e open --format '%w%f' "$current" "$current/theme" "$work/data/ElyOffice" >"$work/opens" 2>/dev/null &
  spy=$!
  sleep 1
  local before opens
  before=$(statuses)
  sleep 4
  kill "$spy"
  spy=
  opens=$(grep -c -E '(colors\.toml|theme\.name|settings\.json)$' "$work/opens" || true)
  [ "$opens" -eq 0 ] || fail "ElyTree $1 read the theme or settings $opens times: $(sort "$work/opens" | uniq -c)"
  [ "$(statuses)" -eq "$before" ] || fail "ElyTree $1 ran git status $(($(statuses) - before)) more times"
  echo "linux: ElyTree $1 reads nothing"
}
seen() { grep -c "$1" "$work/app.err" || true; }
logged() { # pattern, what; once more than before
  local before=${3:-0}
  for _ in $(seq 40); do
    [ "$(seen "$1")" -gt "$before" ] && return
    sleep 0.25
  done
  fail "ElyTree missed $2"
}
grown() { # what
  for _ in $(seq 40); do
    [ "$(statuses)" -gt "$before" ] && return
    sleep 0.25
  done
  fail "ElyTree missed $1"
}
# Writes through a mapping: CLOSE_WRITE, no MODIFY.
mapped() { # file, old, new
  python3 -I -c 'import mmap, sys
with open(sys.argv[1], "r+b") as f, mmap.mmap(f.fileno(), 0) as m:
    at = m.find(sys.argv[2].encode())
    assert at >= 0, sys.argv[2]
    m[at:at + len(sys.argv[3])] = sys.argv[3].encode()
    m.flush()' "$@"
}
idle "idle after opening"

# Omarchy's swap, a settings write, an edit; then quiet again.
mkdir "$current/next-theme"
cp "$root/crates/ely-ui/tests/fixtures/omarchy/gruvbox.toml" "$current/next-theme/colors.toml"
rm -rf "$current/theme"
mv "$current/next-theme" "$current/theme"
echo gruvbox >"$current/theme.name"
logged 'omarchy gruvbox applied' "Omarchy's theme swap"
echo '{"version": 1, "author": "Tester"}' >"$work/data/ElyOffice/settings.json"
logged 'settings: changed on disk' "a settings write"
before=$(statuses)
echo two >>"$repo/a.txt"
grown "an edit in the repo"
sleep 2
idle "idle after changes"

mapped "$current/theme.name" gruvbox Gruvbox
logged 'omarchy Gruvbox applied' "a mapped theme name"
changed=$(seen 'settings: changed on disk')
mapped "$work/data/ElyOffice/settings.json" Tester Testor
logged 'settings: changed on disk' "a mapped settings write" "$changed"
before=$(statuses)
mapped "$repo/a.txt" two Two
grown "a mapped edit in the repo"
echo "linux: ElyTree follows mapped writes"
