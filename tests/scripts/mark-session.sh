#!/usr/bin/env bash
# QA-21: a save keeps bytes no edit touched.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP: macOS only"; exit 0; }
[ -d "${ELY_CEF:-$root/engines/cef}/Chromium Embedded Framework.framework" ] || {
  echo "SKIP: no CEF; run scripts/fetch-cef.sh"
  exit 0
}
command -v npm >/dev/null || { echo "SKIP: no npm"; exit 0; }
work=$(mktemp -d)
app=
trap 'if [ -n "$app" ]; then kill "$app" 2>/dev/null || true; wait "$app" 2>/dev/null || true; fi; chmod -R u+rw "$work"; rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

"$root/scripts/build-mark-content.sh" >"$work/build" 2>&1 || fail "build: $(tail -n 5 "$work/build")"
cargo build -q -p ely-mark --manifest-path "$root/Cargo.toml"
swiftc -O -o "$work/press" "$root/tests/scripts/ax-press.swift"
waits() {
  for _ in $(seq 200); do
    grep -qF "$1" "$work/err" && return 0
    kill -0 "$app" 2>/dev/null || break
    sleep 0.25
  done
  fail "never saw '$1': $(grep -E 'mark|office' "$work/err" | tail -n 5)"
}
# Starts ElyMark on `file`; it must show visually.
launch() {
  : >"$work/err"
  ELY_LOG=info ELY_LOCALE=en ELY_NO_DIALOG=1 HOME="$work" XDG_DATA_HOME="$work/data" \
    "$root/target/debug/ely-mark" "$1" >"$work/out" 2>"$work/err" &
  app=$!
}
shown() {
  waits "mark: shown as"
  grep -qF 'mark: shown as "visual", fallback false' "$work/err" ||
    fail "not shown visually: $(grep 'mark: shown' "$work/err")"
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" \
    -e 'delay 0.5' >/dev/null
}
type() { osascript -e "tell application \"System Events\" to $1" -e 'delay 1' >/dev/null; }
save_quit() {
  type 'keystroke "s" using command down'
  waits "office: saved"
  osascript -e 'tell application "System Events" to keystroke "q" using command down' >/dev/null
  for _ in $(seq 40); do
    kill -0 "$app" 2>/dev/null || break
    sleep 0.25
  done
  ! kill -0 "$app" 2>/dev/null || fail "ElyMark did not quit"
  app=
  ! grep -q "panicked" "$work/err" || fail "panics: $(grep -m3 panicked "$work/err")"
}
# Saves `name` after `keys`; it must equal `want`.
check() {
  local name=$1 text=$2 keys=$3 want=$4
  printf '%s' "$text" >"$work/$name"
  launch "$work/$name"
  shown
  [ -z "$keys" ] || type "$keys"
  save_quit
  [ "$(cat "$work/$name"; echo .)" = "$want." ] || fail "$name saved as: $(cat "$work/$name")"
}

# BOM, CRLF, front matter and syntax Milkdown rewrites.
python3 - "$work/keep.md" <<'PY'
import sys
text = """---
title: Keep
tags: [a, b]
---
First paragraph.

Setext
======

1) paren list
2) second

* star
* list

<div align="center">
  <b>html</b>
</div>

|a|b|
|-|-|
|1|2|

See [[Wiki Link]], ~sub~ and [a ref][r].  
next line

[r]: https://example.com "T"

Last paragraph.
"""
open(sys.argv[1], "wb").write(b"\xef\xbb\xbf" + text.replace("\n", "\r\n").encode())
PY
cp "$work/keep.md" "$work/plain.md"
launch "$work/plain.md"
shown
save_quit
cmp -s "$work/keep.md" "$work/plain.md" || fail "an unedited save changed bytes"
cp "$work/keep.md" "$work/edited.md"
launch "$work/edited.md"
shown
type 'keystroke "x"'
save_quit
python3 - "$work/keep.md" "$work/edited.md" <<'PY'
import sys
before, after = (open(path, "rb").read() for path in sys.argv[1:])
want = before.replace(b"First paragraph.", b"xFirst paragraph.", 1)
if after != want:
    sys.exit(f"FAIL: the edit rewrote more than its block:\n{after!r}")
PY

# Blocks Milkdown would rewrite are edited as raw text.
check wiki.md $'Alpha with [[Wiki Link]].\n' 'keystroke "x"' $'xAlpha with [[Wiki Link]].\n'
quote=$'> First.\n>\n> Setext\n> ======\n>\n> Last with [[Wiki]].\n'
check quote.md "$quote" 'keystroke "x"' "x$quote"
check defs.md $'[r]: https://example.com\n' 'keystroke "x"' $'x[r]: https://example.com\n'
# A compact table stays a table; edits keep its style.
check table.md $'|a|b|\n|-|-|\n|1|2|\n' 'keystroke "x"' $'|xa|b|\n|-|-|\n|1|2|\n'
check dashes.md $'| a | b |\n|---|---|\n| 1 | 2 |\n' 'keystroke "x"' $'| xa | b |\n| - | - |\n| 1 | 2 |\n'
check quoted.md $'> |alpha|beta|\n> |---|---|\n> |1|2|\n' 'keystroke "x"' $'> |xalpha|beta|\n> |-|-|\n> |1|2|\n'
check listed.md $'- item\n\n  |alpha|beta|\n  |---|---|\n  |1|2|\n' 'keystroke "x"' $'- xitem\n\n  |alpha|beta|\n  |-|-|\n  |1|2|\n'
# Items open with a paragraph; a leading table stays raw.
check leading.md $'- |alpha|beta|\n  |---|---|\n' 'keystroke "x"' $'x- |alpha|beta|\n  |---|---|\n'
check short.md $'|a|b|c|\n|-|-|-|\n|1|2|\n' 'keystroke "x"' $'|xa|b|c|\n|-|-|-|\n|1|2||\n'
# Cells Milkdown would rewrite keep the table raw.
check wikicell.md $'|a|[[Wiki Link]]|\n|-|-|\n|1|2|\n' 'keystroke "x"' $'x|a|[[Wiki Link]]|\n|-|-|\n|1|2|\n'
# Deleting forward at a hidden definition moves nothing.
gap=$'Alpha.\n\n[r]: /path " T "   \n\nBeta.\n\nTail.\n'
check gap.md "$gap" 'key code 119' "$gap"
printf '%s' "$gap" >"$work/gap2.md"
launch "$work/gap2.md"
shown
type 'key code 119'
type 'key code 117'
save_quit
[ "$(cat "$work/gap2.md"; echo .)" = "$gap." ] || fail "the definition moved: $(cat "$work/gap2.md")"

# A reloaded page's edits still save.
printf 'Alpha.\n' >"$work/crash.md"
launch "$work/crash.md"
shown
# Two versions first: the new page counts from one.
type 'keystroke "x"'
type 'keystroke "z"'
pkill -P "$app" -f -- "--type=renderer" || fail "no renderer to stop"
pressed=
for _ in $(seq 40); do
  [ "$("$work/press" "$app" Reload)" = pressed ] && pressed=1 && break
  sleep 0.25
done
[ -n "$pressed" ] || fail "no Reload button"
for _ in $(seq 80); do
  [ "$(grep -c 'mark: shown as' "$work/err")" -ge 2 ] && break
  sleep 0.25
done
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" >/dev/null
type 'keystroke "y"'
save_quit
grep -q "y" "$work/crash.md" || fail "an edit after reload was lost: $(cat "$work/crash.md")"

# An unreadable file says so; no password is asked.
printf 'secret\n' >"$work/denied.md"
chmod 000 "$work/denied.md"
launch "$work/denied.md"
waits "did not open"
grep -q "Permission denied" "$work/err" || fail "the reason is missing: $(grep 'did not open' "$work/err")"
! grep -q "locked" "$work/err" || fail "taken for a locked file"
kill "$app"
app=
echo "ok: mark session"
